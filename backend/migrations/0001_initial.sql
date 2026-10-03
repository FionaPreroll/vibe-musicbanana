-- musicbanana: Schema-Entwurf v1 (PostgreSQL 16)
-- Grundidee: Scrobbles werden roh und unveraendert gespeichert (listen.*_raw),
-- der Katalog (artist/release/recording) ist davon getrennt und darf
-- umbenannt/zusammengefuehrt werden, ohne dass Rohdaten verloren gehen.

CREATE EXTENSION IF NOT EXISTS citext;

-- ---------------------------------------------------------------- Konten

CREATE TABLE account (
    id            bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    username      citext      NOT NULL UNIQUE,
    email         citext      NOT NULL UNIQUE,
    password_hash text        NOT NULL,           -- argon2id
    is_admin      boolean     NOT NULL DEFAULT false,
    avatar_path   text,
    created_at    timestamptz NOT NULL DEFAULT now()
);

CREATE TYPE visibility AS ENUM ('public', 'followers', 'private');

-- Ein Konto kann mehrere Profile haben (wie in musicbanana-symfony),
-- z.B. "zuhause" und "arbeit". Scrobbles haengen am Profil.
CREATE TABLE profile (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account_id  bigint      NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    slug        citext      NOT NULL,             -- URL: /u/<username>/<slug>
    name        text        NOT NULL,
    visibility  visibility  NOT NULL DEFAULT 'public',
    created_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (account_id, slug)
);

-- Token fuer Scrobble-Clients (ListenBrainz-/Audioscrobbler-kompatible API).
CREATE TABLE api_token (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    profile_id   bigint      NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    token_hash   bytea       NOT NULL UNIQUE,     -- sha256 des Tokens
    label        text        NOT NULL,            -- "Rhythmbox Laptop"
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz,
    revoked_at   timestamptz
);

-- Ersetzt "friends" aus musicbanana2: gerichtetes Folgen.
CREATE TABLE follow (
    follower_id bigint      NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    profile_id  bigint      NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    created_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (follower_id, profile_id)
);

-- ---------------------------------------------------------------- Katalog

-- Kein UNIQUE auf name: es gibt verschiedene Artists mit gleichem Namen.
-- Eindeutig ist nur die MusicBrainz-ID (falls bekannt).
CREATE TABLE artist (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name        text NOT NULL,
    sort_name   text,
    mbid        uuid UNIQUE,
    merged_into bigint REFERENCES artist(id),     -- statt "obsolete"-Flag
    created_at  timestamptz NOT NULL DEFAULT now()
);

-- Album. Heisst wie bei MusicBrainz "release".
CREATE TABLE release (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    title       text   NOT NULL,
    artist_id   bigint NOT NULL REFERENCES artist(id),  -- Album-Artist
    year        smallint,
    mbid        uuid UNIQUE,
    merged_into bigint REFERENCES release(id),
    created_at  timestamptz NOT NULL DEFAULT now()
);

-- Ein Stueck unabhaengig vom Album (wie last.fm-Track-Charts zaehlen):
-- dasselbe Lied auf Album und Best-of ist EIN recording.
CREATE TABLE recording (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    title       text   NOT NULL,
    artist_id   bigint NOT NULL REFERENCES artist(id),
    length_ms   integer,
    mbid        uuid UNIQUE,
    merged_into bigint REFERENCES recording(id),
    created_at  timestamptz NOT NULL DEFAULT now()
);

-- Aufloesung Rohstring -> Katalog-Eintrag. Neue Scrobbles werden ueber
-- den normalisierten Schluessel (lower, trim, Unicode NFKC) zugeordnet.
-- Ein Merge/Rename aendert nur diese Zeilen + listen-FKs, nie die Rohdaten.
CREATE TABLE artist_alias (
    name_key  text   PRIMARY KEY,
    artist_id bigint NOT NULL REFERENCES artist(id)
);

CREATE TABLE release_alias (
    artist_id bigint NOT NULL REFERENCES artist(id),
    title_key text   NOT NULL,
    release_id bigint NOT NULL REFERENCES release(id),
    PRIMARY KEY (artist_id, title_key)
);

CREATE TABLE recording_alias (
    artist_id    bigint NOT NULL REFERENCES artist(id),
    title_key    text   NOT NULL,
    recording_id bigint NOT NULL REFERENCES recording(id),
    PRIMARY KEY (artist_id, title_key)
);

-- ---------------------------------------------------------------- Scrobbles

CREATE TABLE listen (
    id                bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    profile_id        bigint      NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    listened_at       timestamptz NOT NULL,
    -- Rohdaten exakt wie vom Client geschickt (nie veraendert)
    artist_raw        text        NOT NULL,
    track_raw         text        NOT NULL,
    album_raw         text,
    album_artist_raw  text,
    track_number      smallint,
    duration_ms       integer,
    client            text,                      -- "rhythmbox 3.4", "import:php"
    extra             jsonb,                     -- mbids etc. aus dem Client
    -- Zuordnung zum Katalog (darf sich durch Merges aendern)
    artist_id         bigint      NOT NULL REFERENCES artist(id),
    recording_id      bigint      NOT NULL REFERENCES recording(id),
    release_id        bigint      REFERENCES release(id),
    submitted_at      timestamptz NOT NULL DEFAULT now(),
    UNIQUE (profile_id, listened_at)             -- Duplikat-Schutz bei Resubmits
);

-- Charts und "zuletzt gehoert" laufen alle ueber Profil + Zeitraum.
CREATE INDEX listen_profile_time ON listen (profile_id, listened_at DESC)
    INCLUDE (artist_id, recording_id, release_id);
CREATE INDEX listen_recording ON listen (recording_id);
CREATE INDEX listen_release   ON listen (release_id) WHERE release_id IS NOT NULL;
CREATE INDEX listen_artist    ON listen (artist_id);

-- Hoechstens ein "laeuft gerade" pro Profil, laeuft von selbst ab.
CREATE TABLE now_playing (
    profile_id  bigint      PRIMARY KEY REFERENCES profile(id) ON DELETE CASCADE,
    started_at  timestamptz NOT NULL,
    expires_at  timestamptz NOT NULL,
    artist_raw  text        NOT NULL,
    track_raw   text        NOT NULL,
    album_raw   text,
    duration_ms integer
);

-- Beispiel: Top-Artists eines Profils im letzten Monat (kein Vorberechnen noetig)
-- SELECT a.name, count(*) AS plays
--   FROM listen l JOIN artist a ON a.id = l.artist_id
--  WHERE l.profile_id = $1 AND l.listened_at >= now() - interval '30 days'
--  GROUP BY a.id, a.name ORDER BY plays DESC LIMIT 50;
