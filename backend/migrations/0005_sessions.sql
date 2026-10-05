-- Browser logins. The cookie holds a random token; only its SHA-256 is stored,
-- as for API tokens, so the table alone gives no working session.
CREATE TABLE session (
    token_hash bytea       PRIMARY KEY,
    account_id bigint      NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL
);

CREATE INDEX session_account ON session (account_id);
