-- Following a profile for followers needs its owner's yes: until then the
-- follow is a request (accepted_at NULL) and shows the follower nothing. The
-- follows from the old musicbanana ("friends") count as accepted.
ALTER TABLE follow ADD COLUMN accepted_at timestamptz;
UPDATE follow SET accepted_at = created_at;

CREATE INDEX follow_profile_idx ON follow (profile_id);
