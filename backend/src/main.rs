use std::{
    env,
    io::{self, IsTerminal},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, bail};
use clap::{Args, Parser, Subcommand, ValueEnum, builder::NonEmptyStringValueParser};
use musicbanana::{
    AppState, account, auth, connections, delete, edit, import_php, live,
    merge::{self, Kind, Suggestion},
    router, tokens, yourspotify,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use time::OffsetDateTime;
use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
};
use tracing_subscriber::EnvFilter;

/// musicbanana server. Configured through DATABASE_URL, LISTEN_ADDR and STATIC_DIR.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the web server (default).
    Serve,
    /// Import a musicbanana-php database into an empty musicbanana database.
    ImportPhp {
        /// MySQL/MariaDB URL of the old database, e.g. mysql://root@127.0.0.1:3306/musicbanana
        #[arg(long)]
        from: String,
    },
    /// Import Spotify plays from YourSpotify: the whole history the first time,
    /// then only the plays since.
    ImportYourspotify(YourSpotifyArgs),
    /// Connect YourSpotify accounts to profiles; the server then imports their
    /// Spotify plays by itself, the new ones every 15 minutes.
    #[command(subcommand)]
    Yourspotify(ConnectionCommand),
    /// Create accounts and set their passwords.
    #[command(subcommand)]
    Account(AccountCommand),
    /// Create profiles, e.g. one per player or per Spotify account.
    #[command(subcommand)]
    Profile(ProfileCommand),
    /// Manage the tokens scrobble clients use (ListenBrainz API).
    #[command(subcommand)]
    Token(TokenCommand),
    /// Merge duplicates in the catalog, e.g. two spellings of an artist.
    #[command(subcommand)]
    Merge(MergeCommand),
    /// Rename an artist, release (album) or recording (track). Its old spellings
    /// keep leading to it, so later listens under the old name count for it too.
    Rename {
        #[arg(value_enum)]
        kind: Entry,
        /// Id of the entry, as in the address of its page (…/artist/12) or shown
        /// by `merge suggest`.
        id: i64,
        /// The new name.
        name: String,
    },
    /// Give an artist, release (album) or recording (track) a MusicBrainz ID, or
    /// take one away.
    #[command(subcommand)]
    Mbid(MbidCommand),
}

#[derive(Args)]
struct YourSpotifyArgs {
    /// Address of YourSpotify's API (its API_ENDPOINT, not the web interface),
    /// e.g. http://yourspotify-server:8080
    #[arg(long, env = "YOURSPOTIFY_URL", value_parser = NonEmptyStringValueParser::new())]
    from: String,
    /// The public token from YourSpotify's settings. Better set as
    /// YOURSPOTIFY_TOKEN, so that it stays out of the shell history and the
    /// process list.
    #[arg(
        long,
        env = "YOURSPOTIFY_TOKEN",
        hide_env_values = true,
        value_parser = NonEmptyStringValueParser::new()
    )]
    token: String,
    /// Account the plays go to.
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    user: String,
    /// Profile the plays go to.
    #[arg(long, default_value = "default")]
    profile: String,
    /// Fetch the whole history again, e.g. after YourSpotify has imported an
    /// older Spotify export. Plays already in the profile are skipped.
    #[arg(long)]
    all: bool,
    /// Keep running and import the new plays every so often, e.g. 15m or 1h.
    #[arg(long, value_parser = parse_interval)]
    every: Option<Duration>,
}

#[derive(Subcommand)]
enum ConnectionCommand {
    /// Connect a YourSpotify account to a profile, once YourSpotify has taken the
    /// token. The running server imports the whole history within a minute.
    Add {
        /// Address of YourSpotify's API (its API_ENDPOINT, not the web interface),
        /// as the musicbanana server reaches it, e.g. http://yourspotify-server:8080
        #[arg(long, value_parser = NonEmptyStringValueParser::new())]
        from: String,
        /// The public token from YourSpotify's settings. Better set as
        /// YOURSPOTIFY_TOKEN, so that it stays out of the shell history.
        #[arg(
            long,
            env = "YOURSPOTIFY_TOKEN",
            hide_env_values = true,
            value_parser = NonEmptyStringValueParser::new()
        )]
        token: String,
        /// Account the plays go to.
        #[arg(long, value_parser = NonEmptyStringValueParser::new())]
        user: String,
        /// Profile the plays go to.
        #[arg(long, default_value = "default")]
        profile: String,
    },
    /// List the connections and how their latest import went.
    List,
    /// Remove a connection; the listens it brought stay.
    Remove { id: i64 },
}

#[derive(Subcommand)]
enum ProfileCommand {
    /// Create a profile of an account, at /u/<user>/<slug>.
    Create {
        #[arg(long, value_parser = NonEmptyStringValueParser::new())]
        user: String,
        /// Its address: lower-case letters, digits and dashes.
        slug: String,
        /// Its name, the slug by default.
        #[arg(long)]
        name: Option<String>,
        /// Who sees it.
        #[arg(long, value_enum, default_value_t = Visibility::Public)]
        visibility: Visibility,
    },
    /// Delete a profile with its listens, scrobble tokens, YourSpotify
    /// connection and followers. Without --yes it only says what would go.
    Delete {
        #[arg(long, value_parser = NonEmptyStringValueParser::new())]
        user: String,
        slug: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Visibility {
    Public,
    Followers,
    Private,
}

/// "90s", "15m" or "2h".
fn parse_interval(text: &str) -> Result<Duration, String> {
    let (number, unit) = if let Some(number) = text.strip_suffix('s') {
        (number, 1)
    } else if let Some(number) = text.strip_suffix('m') {
        (number, 60)
    } else if let Some(number) = text.strip_suffix('h') {
        (number, 3600)
    } else {
        (text, 0)
    };
    match number.parse::<u64>().ok().and_then(|n| n.checked_mul(unit)) {
        Some(seconds) if seconds > 0 => Ok(Duration::from_secs(seconds)),
        _ => Err(format!("\"{text}\" is not a time like 15m or 1h")),
    }
}

#[derive(Subcommand)]
enum AccountCommand {
    /// Create an account with a default profile. Asks for the password, or
    /// reads it from standard input when that is not a terminal.
    Create {
        username: String,
        #[arg(long)]
        email: String,
    },
    /// Set the password of an account, e.g. when it is forgotten. Asks for it,
    /// or reads it from standard input when that is not a terminal.
    Password { username: String },
    /// Give an account another user name. Links with the old one lead to the
    /// new one, and nobody else can take the old one.
    Rename { username: String, new: String },
    /// Delete an account with all its profiles, listens, tokens, connections,
    /// follows and logins; its user name is free again. Without --yes it only
    /// says what would go.
    Delete {
        username: String,
        #[arg(long)]
        yes: bool,
    },
    /// Give an account another email address, which logs in too.
    Email { username: String, address: String },
    /// Let an account see the status page at /status, or with --off no more.
    Admin {
        username: String,
        #[arg(long)]
        off: bool,
    },
}

#[derive(Subcommand)]
enum TokenCommand {
    /// Create a token; it is printed once and cannot be shown again.
    Create {
        /// Account the token is for.
        #[arg(long)]
        user: String,
        /// Profile the listens go to.
        #[arg(long, default_value = "default")]
        profile: String,
        /// What the token is used by, e.g. "Navidrome".
        #[arg(long)]
        label: String,
    },
    /// List tokens, of all accounts or of one.
    List {
        #[arg(long)]
        user: Option<String>,
    },
    /// Revoke a token; clients using it are refused from then on.
    Revoke {
        /// Token id, as shown by `token list`.
        id: i64,
    },
}

#[derive(Subcommand)]
enum MergeCommand {
    /// List artists, releases and recordings that look like another one, each
    /// with the command that merges them.
    Suggest {
        /// Only these. Artists are best merged first: that also merges their
        /// releases and recordings with the same title.
        #[arg(value_enum)]
        kind: Option<Kinds>,
        /// Suggestions shown per kind.
        #[arg(long, default_value_t = 30)]
        limit: usize,
    },
    /// Take edition notes such as "- Remastered 2011" or "(Deluxe Edition)" off
    /// the titles of albums and tracks heard so far: each one is merged into the
    /// album or track of its plain title, or renamed to it. New listens leave them
    /// out anyway.
    Editions {
        /// Only show what would change.
        #[arg(long)]
        dry_run: bool,
    },
    /// List the latest merges, newest first, with the number that undoes each.
    Log {
        /// Merges shown.
        #[arg(long, default_value_t = 30)]
        limit: i64,
    },
    /// Take a merge back: the merged entry stands on its own again, with its
    /// listens, spellings and MusicBrainz IDs.
    Undo {
        /// Its number, as `merge log` and the merge itself show it.
        op: i64,
        /// Only show what would change.
        #[arg(long)]
        dry_run: bool,
    },
    /// Merge an artist into another one, with its releases and recordings.
    Artist(MergeArgs),
    /// Merge a release (album) into another one.
    Release(MergeArgs),
    /// Merge a recording (track) into another one.
    Recording(MergeArgs),
}

#[derive(Args)]
struct MergeArgs {
    /// Id of the entry that goes away, as shown by `merge suggest`.
    from: i64,
    /// Id of the entry that stays.
    into: i64,
    /// Only show what would change.
    #[arg(long)]
    dry_run: bool,
    /// Merge even though their MusicBrainz IDs tell the two apart, such as an
    /// artist's other name into the main one, or a recording MusicBrainz lists twice.
    #[arg(long)]
    force: bool,
}

#[derive(Subcommand)]
enum MbidCommand {
    /// Give an entry an ID, so that listens with it count for the entry from
    /// then on. The listens so far stay where they are.
    Add(MbidArgs),
    /// Take an ID away from an entry, such as one that came to the wrong one.
    /// The next listen with it is matched as one with a new ID.
    Remove(MbidArgs),
}

#[derive(Args)]
struct MbidArgs {
    #[arg(value_enum)]
    kind: Entry,
    /// Id of the entry, as in the address of its page (…/artist/12) or shown by
    /// `merge suggest`.
    id: i64,
    /// The MusicBrainz ID or the address of its page, for a release (album) that
    /// of the release group: https://musicbrainz.org/release-group/…
    mbid: String,
}

#[derive(Clone, Copy, ValueEnum)]
enum Entry {
    Artist,
    Release,
    Recording,
}

impl From<Entry> for Kind {
    fn from(entry: Entry) -> Self {
        match entry {
            Entry::Artist => Kind::Artist,
            Entry::Release => Kind::Release,
            Entry::Recording => Kind::Recording,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Kinds {
    Artists,
    Releases,
    Recordings,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "musicbanana=info,tower_http=info".into()),
        )
        // No color codes in `docker compose logs` and other redirected output.
        .with_ansi(io::stdout().is_terminal())
        .init();
    let cli = Cli::parse();

    let database_url = env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .context("connecting to PostgreSQL")?;
    sqlx::migrate!()
        .run(&db)
        .await
        .context("running migrations")?;

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(db).await,
        Command::ImportPhp { from } => {
            let data = import_php::read_mysql(&from).await?;
            let report = import_php::write(&db, &data).await?;
            println!("{report}");
            Ok(())
        }
        Command::ImportYourspotify(args) => import_yourspotify(&db, args).await,
        Command::Yourspotify(command) => connection_command(&db, command).await,
        Command::Account(command) => account_command(&db, command).await,
        Command::Profile(command) => profile_command(&db, command).await,
        Command::Token(command) => token(&db, command).await,
        Command::Merge(command) => merge_command(&db, command).await,
        Command::Rename { kind, id, name } => {
            let renamed = edit::rename(&db, kind.into(), id, &name).await?;
            println!("{renamed}");
            Ok(())
        }
        Command::Mbid(command) => mbid_command(&db, command).await,
    }
}

async fn connection_command(db: &PgPool, command: ConnectionCommand) -> anyhow::Result<()> {
    match command {
        ConnectionCommand::Add {
            from,
            token,
            user,
            profile,
        } => {
            let profile_id = yourspotify::profile_id(db, &user, &profile).await?;
            let id = connections::add(db, profile_id, &from, &token).await?;
            println!(
                "Connected YourSpotify at {from} to {user}/{profile} (connection {id}). The \
                 running server imports its plays within a minute, the whole history first."
            );
        }
        ConnectionCommand::List => {
            let all = connections::list(db, None).await?;
            if all.is_empty() {
                println!("No YourSpotify connections.");
            }
            for c in all {
                let state = match (&c.error, c.started_at, c.finished_at) {
                    (_, None, _) => "not imported yet".to_owned(),
                    (_, Some(started), finished) if finished.is_none_or(|f| f < started) => {
                        format!("importing since {}", minutes(started))
                    }
                    (Some(error), _, _) => format!("failed: {error}"),
                    (None, _, Some(finished)) => format!("imported at {}", minutes(finished)),
                    (None, _, None) => unreachable!(),
                };
                println!(
                    "{:>4}  {}/{}  {}  {} listens so far, {state}",
                    c.id, c.username, c.profile, c.url, c.imported
                );
            }
        }
        ConnectionCommand::Remove { id } => {
            if !connections::remove(db, id, None).await? {
                bail!("there is no connection {id}");
            }
            println!("Removed connection {id}; the listens it brought stay.");
        }
    }
    Ok(())
}

fn report_removal(what: &str, removal: &delete::Removal, done: bool) {
    if done {
        println!("Deleted {what}. {removal}.");
    } else {
        println!("Would delete {what}. {removal}.\nNothing is deleted yet: add --yes to do it.");
    }
}

async fn profile_command(db: &PgPool, command: ProfileCommand) -> anyhow::Result<()> {
    let (user, slug, name, visibility) = match command {
        ProfileCommand::Create {
            user,
            slug,
            name,
            visibility,
        } => (user, slug, name, visibility),
        ProfileCommand::Delete { user, slug, yes } => {
            let removal = delete::profile(db, &user, slug.trim(), yes).await?;
            report_removal(
                &format!("the profile {user}/{}", slug.trim()),
                &removal,
                yes,
            );
            return Ok(());
        }
    };
    let slug = slug.trim();
    account::check_slug(slug).map_err(anyhow::Error::msg)?;
    let name = name.as_deref().map_or(slug, str::trim);
    let visibility = match visibility {
        Visibility::Public => "public",
        Visibility::Followers => "followers",
        Visibility::Private => "private",
    };
    let created = sqlx::query!(
        "INSERT INTO profile (account_id, slug, name, visibility)
         SELECT id, $2, $3, $4::text::visibility FROM account WHERE username = $1::text::citext
         ON CONFLICT (account_id, slug) DO NOTHING",
        user,
        slug,
        name,
        visibility,
    )
    .execute(db)
    .await?;
    if created.rows_affected() == 0 {
        bail!("there is no user {user}, or they have a profile {slug} already");
    }
    println!("Created the profile {user}/{slug} (\"{name}\", {visibility}).");
    Ok(())
}

async fn import_yourspotify(db: &PgPool, args: YourSpotifyArgs) -> anyhow::Result<()> {
    let source = yourspotify::Source::new(&args.from, &args.token)?;
    let profile = yourspotify::profile_id(db, &args.user, &args.profile).await?;
    let Some(every) = args.every else {
        let report = yourspotify::import(db, &source, profile, args.all).await?;
        println!("{report}");
        return Ok(());
    };
    let mut all = args.all;
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        match yourspotify::import(db, &source, profile, all).await {
            Ok(report) if report.plays == 0 => tracing::debug!("{report}"),
            Ok(report) => tracing::info!("{report}"),
            // Perhaps YourSpotify is down for the moment; try again next time.
            Err(error) => tracing::warn!("{error:#}"),
        }
        all = false;
        tokio::select! {
            () = tokio::time::sleep(every) => {}
            () = &mut shutdown => return Ok(()),
        }
    }
}

async fn mbid_command(db: &PgPool, command: MbidCommand) -> anyhow::Result<()> {
    match command {
        MbidCommand::Add(args) => {
            let kind = args.kind.into();
            let mbid = edit::parse_mbid(kind, &args.mbid)?;
            let (entry, new) = edit::add_mbid(db, kind, args.id, mbid).await?;
            if new {
                println!("Gave {entry} the MusicBrainz ID {mbid}.");
            } else {
                println!("Nothing to do: {entry} has the MusicBrainz ID {mbid} already.");
            }
        }
        MbidCommand::Remove(args) => {
            let kind = args.kind.into();
            let mbid = edit::parse_mbid(kind, &args.mbid)?;
            let entry = edit::remove_mbid(db, kind, args.id, mbid).await?;
            println!("Took the MusicBrainz ID {mbid} from {entry}.");
        }
    }
    Ok(())
}

async fn account_command(db: &PgPool, command: AccountCommand) -> anyhow::Result<()> {
    match command {
        AccountCommand::Create { username, email } => {
            account::check_username(username.trim()).map_err(anyhow::Error::msg)?;
            account::check_email(email.trim()).map_err(anyhow::Error::msg)?;
            let hash = auth::hash_password(&read_password()?)?;
            let mut tx = db.begin().await?;
            let id = sqlx::query_scalar!(
                "INSERT INTO account (username, email, password_hash)
                 SELECT $1::text::citext, $2::text::citext, $3
                  WHERE NOT EXISTS (SELECT FROM former_username
                                     WHERE username = $1::text::citext)
                 ON CONFLICT DO NOTHING
                 RETURNING id",
                username.trim(),
                email.trim(),
                hash,
            )
            .fetch_optional(&mut *tx)
            .await?
            .context("there is an account with this user name or email address already")?;
            sqlx::query!(
                "INSERT INTO profile (account_id, slug, name) VALUES ($1, 'default', 'Default')",
                id
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            eprintln!(
                "Created the account {} with a default profile.",
                username.trim()
            );
        }
        AccountCommand::Password { username } => {
            let hash = auth::hash_password(&read_password()?)?;
            let changed = sqlx::query!(
                "UPDATE account SET password_hash = $2, password_legacy_md5 = false
                  WHERE username = $1::text::citext",
                username,
                hash,
            )
            .execute(db)
            .await?;
            if changed.rows_affected() == 0 {
                bail!("there is no account {username}");
            }
            eprintln!("Set the password of {username}.");
        }
        AccountCommand::Rename { username, new } => {
            let id = sqlx::query_scalar!(
                "SELECT id FROM account WHERE username = $1::text::citext",
                username
            )
            .fetch_optional(db)
            .await?
            .with_context(|| format!("there is no account {username}"))?;
            let (old, new) = account::rename(db, id, &new)
                .await?
                .map_err(anyhow::Error::msg)?;
            eprintln!("Renamed {old} to {new}; links with {old} lead to {new}.");
        }
        AccountCommand::Delete { username, yes } => {
            let removal = delete::account(db, &username, yes).await?;
            report_removal(&format!("the account {username}"), &removal, yes);
        }
        AccountCommand::Email { username, address } => {
            let id = sqlx::query_scalar!(
                "SELECT id FROM account WHERE username = $1::text::citext",
                username
            )
            .fetch_optional(db)
            .await?
            .with_context(|| format!("there is no account {username}"))?;
            let (old, new) = account::change_email(db, id, &address)
                .await?
                .map_err(anyhow::Error::msg)?;
            eprintln!("Changed the email address of {username} from {old} to {new}.");
        }
        AccountCommand::Admin { username, off } => {
            let changed = sqlx::query!(
                "UPDATE account SET is_admin = $2 WHERE username = $1::text::citext",
                username,
                !off,
            )
            .execute(db)
            .await?;
            if changed.rows_affected() == 0 {
                bail!("there is no account {username}");
            }
            eprintln!(
                "{username} {} the status page now.",
                if off { "no longer sees" } else { "sees" }
            );
        }
    }
    Ok(())
}

/// Asks twice on a terminal; otherwise takes the first line of standard input.
fn read_password() -> anyhow::Result<String> {
    let password = if io::stdin().is_terminal() {
        let password = rpassword::prompt_password("Password: ")?;
        if rpassword::prompt_password("Again: ")? != password {
            bail!("the two passwords differ");
        }
        password
    } else {
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        line.trim_end_matches(['\r', '\n']).to_owned()
    };
    if password.chars().count() < account::MIN_PASSWORD {
        bail!(
            "the password needs at least {} characters",
            account::MIN_PASSWORD
        );
    }
    Ok(password)
}

async fn token(db: &PgPool, command: TokenCommand) -> anyhow::Result<()> {
    match command {
        TokenCommand::Create {
            user,
            profile,
            label,
        } => {
            let new = tokens::create(db, &user, &profile, &label).await?;
            // Only the token goes to stdout, so it can be piped somewhere.
            eprintln!(
                "Token {} for {user}/{profile} ({label}), shown only this once:",
                new.id
            );
            println!("{}", new.token);
        }
        TokenCommand::List { user } => {
            let list = tokens::list(db, user.as_deref()).await?;
            if list.is_empty() {
                eprintln!("No tokens.");
            }
            for t in list {
                let used = t.last_used_at.map_or("never used".into(), |at| {
                    format!("last used {}", minutes(at))
                });
                let revoked = t
                    .revoked_at
                    .map_or(String::new(), |at| format!(", revoked {}", minutes(at)));
                println!(
                    "{:>4}  {}/{}  {}  created {}, {used}{revoked}",
                    t.id,
                    t.username,
                    t.slug,
                    t.label,
                    minutes(t.created_at)
                );
            }
        }
        TokenCommand::Revoke { id } => {
            if !tokens::revoke(db, id).await? {
                bail!("there is no active token {id}");
            }
            eprintln!("Token {id} revoked.");
        }
    }
    Ok(())
}

async fn merge_command(db: &PgPool, command: MergeCommand) -> anyhow::Result<()> {
    let (kind, args) = match command {
        MergeCommand::Suggest { kind, limit } => {
            let kinds = match kind {
                None => vec![Kind::Artist, Kind::Release, Kind::Recording],
                Some(Kinds::Artists) => vec![Kind::Artist],
                Some(Kinds::Releases) => vec![Kind::Release],
                Some(Kinds::Recordings) => vec![Kind::Recording],
            };
            for kind in kinds {
                let found = merge::suggest(db, kind).await?;
                print_suggestions(kind, &found, limit);
            }
            return Ok(());
        }
        MergeCommand::Editions { dry_run } => {
            let done = merge::editions(db, dry_run).await?;
            for edition in &done {
                println!("{edition}");
            }
            let count = |f: fn(&merge::Edition) -> bool| done.iter().filter(|e| f(e)).count();
            let merged = count(|e| matches!(e, merge::Edition::Merged(_)));
            let renamed = count(|e| matches!(e, merge::Edition::Renamed { .. }));
            let kept = count(|e| matches!(e, merge::Edition::ToldApart { .. }));
            println!(
                "{} {merged} merged, {renamed} renamed, {kept} kept apart.",
                if dry_run { "Would have:" } else { "Done:" }
            );
            if dry_run {
                println!("Dry run, nothing was changed.");
            }
            return Ok(());
        }
        MergeCommand::Log { limit } => {
            let ops = merge::log(db, limit.max(1)).await?;
            if ops.is_empty() {
                println!("No merges yet.");
            }
            for op in ops {
                println!("{op}");
            }
            return Ok(());
        }
        MergeCommand::Undo { op, dry_run } => {
            println!("{}", merge::undo(db, op, dry_run).await?);
            return Ok(());
        }
        MergeCommand::Artist(args) => (Kind::Artist, args),
        MergeCommand::Release(args) => (Kind::Release, args),
        MergeCommand::Recording(args) => (Kind::Recording, args),
    };
    let options = merge::Options {
        dry_run: args.dry_run,
        force: args.force,
    };
    let merged = merge::merge(db, kind, args.from, args.into, options).await?;
    println!("{merged}");
    Ok(())
}

/// One line per suggestion: the command, then why as a shell comment, so the
/// whole line can be pasted.
fn print_suggestions(kind: Kind, found: &[Suggestion], limit: usize) {
    let heading = match kind {
        Kind::Artist => "Artists",
        Kind::Release => "Releases",
        Kind::Recording => "Recordings",
    };
    if found.is_empty() {
        println!("{heading} that look alike: none\n");
        return;
    }
    println!("{heading} that look alike, most certain first:");
    let shown = &found[..found.len().min(limit)];
    let commands: Vec<String> = shown
        .iter()
        .map(|s| format!("musicbanana merge {kind} {} {}", s.from.id, s.into.id))
        .collect();
    let width = commands.iter().map(String::len).max().unwrap_or(0);
    for (s, command) in shown.iter().zip(&commands) {
        let by = s
            .into
            .artist
            .as_ref()
            .map_or(String::new(), |artist| format!(", by {artist}"));
        println!(
            "{command:width$}  # {}: \"{}\" ({}) into \"{}\" ({}){by}",
            s.likeness,
            s.from.name,
            listens(s.from.listens),
            s.into.name,
            listens(s.into.listens),
        );
    }
    if found.len() > shown.len() {
        println!(
            "… and {} more, see --limit {}",
            found.len() - shown.len(),
            found.len()
        );
    }
    println!();
}

fn listens(n: i64) -> String {
    if n == 1 {
        "1 listen".to_owned()
    } else {
        format!("{n} listens")
    }
}

/// "2026-10-03 14:30 UTC"
fn minutes(at: OffsetDateTime) -> String {
    let at = at.to_offset(time::UtcOffset::UTC);
    format!("{} {:02}:{:02} UTC", at.date(), at.hour(), at.minute())
}

async fn serve(db: PgPool) -> anyhow::Result<()> {
    let listen_addr = env::var("LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let static_dir =
        PathBuf::from(env::var("STATIC_DIR").unwrap_or_else(|_| "../frontend/build".into()));

    let access_log = match env::var("ACCESS_LOG").as_deref() {
        Err(_) | Ok("" | "on" | "true" | "1") => true,
        Ok("off" | "false" | "0") => false,
        Ok(other) => bail!("ACCESS_LOG is on or off, not {other}"),
    };
    let allowed =
        yourspotify::Allowlist::parse(&env::var("YOURSPOTIFY_ALLOWED_URLS").unwrap_or_default())
            .context("YOURSPOTIFY_ALLOWED_URLS")?;
    if allowed.is_empty() {
        tracing::info!(
            "YourSpotify connections only from the command line (YOURSPOTIFY_ALLOWED_URLS is empty)"
        );
    } else {
        tracing::info!(
            "YourSpotify connections in the settings allowed for {}",
            allowed.addresses().join(", ")
        );
    }

    musicbanana::status::started();
    connections::spawn(db.clone());
    let live = live::Hub::default();
    live.spawn_listener(db.clone());
    let app = router(
        AppState {
            db,
            yourspotify_allowed: Arc::new(allowed),
            access_log,
            live: live.clone(),
        },
        &static_dir,
    );
    let listener = TcpListener::bind(&listen_addr).await?;
    tracing::info!(
        "listening on http://{listen_addr}, access log {}",
        if access_log { "on" } else { "off" }
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown_signal().await;
        // Open profile pages hold a connection each; end those too.
        live.close();
    })
    .await?;
    Ok(())
}

/// Ctrl+C, or SIGTERM from `docker stop`. In a container the server runs as
/// PID 1, which gets no default handler for SIGTERM and would otherwise only
/// stop when Docker kills it after the grace period.
async fn shutdown_signal() {
    let terminate = async {
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(_) => std::future::pending().await,
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        () = terminate => {}
    }
    tracing::info!("shutting down");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals() {
        assert_eq!(parse_interval("90s"), Ok(Duration::from_secs(90)));
        assert_eq!(parse_interval("15m"), Ok(Duration::from_secs(900)));
        assert_eq!(parse_interval("2h"), Ok(Duration::from_secs(7200)));
        for wrong in ["15", "0m", "m", "-1h", "1.5h", "1d", ""] {
            assert!(parse_interval(wrong).is_err(), "{wrong}");
        }
    }
}
