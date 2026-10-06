-- Whether an account sees streaks (days in a row with listens). Turned off,
-- it sees them on no profile, and nobody sees the streaks of its profiles.
ALTER TABLE account
    ADD COLUMN show_streaks boolean NOT NULL DEFAULT true;
