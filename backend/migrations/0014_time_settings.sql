-- The time zone and first day of the week an account sees dates in, on its own
-- profiles and on everybody else's. No time zone means the browser's; the week
-- starts on Monday unless the account picks another day (ISO numbers, 1 Monday
-- to 7 Sunday).
ALTER TABLE account
    ADD COLUMN time_zone  text,
    ADD COLUMN week_start smallint NOT NULL DEFAULT 1 CHECK (week_start BETWEEN 1 AND 7);
