-- Names an account had before it was renamed. Links with an old name lead to the
-- new one, and nobody else can take an old name, so such a link never shows
-- somebody else's listens. Renaming back to an old name takes it out again.
CREATE TABLE former_username (
    username    citext      PRIMARY KEY,
    account_id  bigint      NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    renamed_at  timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX former_username_account ON former_username (account_id);
