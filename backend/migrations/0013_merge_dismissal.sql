-- Merge suggestions an admin hid on the page "Merges", as two entries that only
-- look alike (a live version and the original, two bands of the same name).
-- `merge suggest` and the page leave such a pair out, in either direction,
-- until it is shown again. The names are as they were when it was hidden.
CREATE TABLE merge_dismissal (
    kind       text        NOT NULL,              -- artist, release, recording
    from_id    bigint      NOT NULL,
    into_id    bigint      NOT NULL,
    from_name  text        NOT NULL,
    into_name  text        NOT NULL,
    hidden_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (kind, from_id, into_id)
);
