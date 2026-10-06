-- Artists and tracks the owner of a profile took out of its "Lost & found",
-- because they don't want them back, until shown again. A dismissed artist
-- takes its tracks along. After a merge the dismissal counts for the entry the
-- dismissed one was merged into (merged_into), so it stays out.
CREATE TABLE lost_found_dismissal (
    profile_id    bigint      NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    kind          text        NOT NULL CHECK (kind IN ('artist', 'recording')),
    entity_id     bigint      NOT NULL,
    dismissed_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (profile_id, kind, entity_id)
);
