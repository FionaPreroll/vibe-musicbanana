-- Fetching a connection's whole history again, e.g. after YourSpotify has
-- imported an older Spotify export (src/connections.rs). Plays the profile has
-- already are skipped as usual.
ALTER TABLE yourspotify_connection
    -- Asked for and not done yet; the server starts within a minute.
    ADD COLUMN refetch_requested_at timestamptz,
    -- How far it got: the plays before this time are done. An import that
    -- stops goes on from here the next time.
    ADD COLUMN refetch_at           timestamptz,
    -- What it found so far, or the last time.
    ADD COLUMN refetch_plays        bigint NOT NULL DEFAULT 0,
    ADD COLUMN refetch_new          bigint NOT NULL DEFAULT 0,
    ADD COLUMN refetch_left_out     bigint NOT NULL DEFAULT 0,  -- YourSpotify knows no artist
    ADD COLUMN refetched_at         timestamptz;                -- when the last one finished
