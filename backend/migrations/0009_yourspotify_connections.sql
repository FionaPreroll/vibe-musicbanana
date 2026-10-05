-- YourSpotify accounts connected to profiles: the server imports their new
-- Spotify plays every 15 minutes (src/connections.rs). One per profile, as an
-- import goes on from the profile's latest Spotify play.
CREATE TABLE yourspotify_connection (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    profile_id  bigint      NOT NULL UNIQUE REFERENCES profile(id) ON DELETE CASCADE,
    api_url     text        NOT NULL,             -- YourSpotify's API_ENDPOINT
    token       text        NOT NULL,             -- its public token, needed as is to ask it
    created_at  timestamptz NOT NULL DEFAULT now(),
    started_at  timestamptz,                      -- of the latest import
    finished_at timestamptz,
    imported    bigint      NOT NULL DEFAULT 0,   -- listens it brought so far
    error       text                              -- why the latest import failed
);
