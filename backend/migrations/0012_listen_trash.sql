-- Listens taken out of a profile's history by its owner, on the page "Edit
-- listening history". They stay here until restored (back to listen, with the
-- same id) or until the owner empties the trash. Charts and every other page
-- read listen only, so they leave the trash out without knowing about it.
CREATE TABLE listen_trash (
    id                bigint      PRIMARY KEY,
    profile_id        bigint      NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    listened_at       timestamptz NOT NULL,
    artist_raw        text        NOT NULL,
    track_raw         text        NOT NULL,
    album_raw         text,
    album_artist_raw  text,
    track_number      smallint,
    duration_ms       integer,
    client            text,
    extra             jsonb,
    -- As in the listen when it was trashed; a merge since then is followed
    -- (merged_into) when it comes back.
    artist_id         bigint      NOT NULL REFERENCES artist(id),
    recording_id      bigint      NOT NULL REFERENCES recording(id),
    release_id        bigint      REFERENCES release(id),
    submitted_at      timestamptz NOT NULL,
    trashed_at        timestamptz NOT NULL DEFAULT now()
);

-- Also keeps a scrobbler or an import from sending a trashed listen again.
CREATE INDEX listen_trash_profile_time ON listen_trash (profile_id, listened_at);
