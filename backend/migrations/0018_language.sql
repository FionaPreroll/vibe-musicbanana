-- The language an account sees the pages in, as a language code like "de" or
-- "en". None means the browser's.
ALTER TABLE account
    ADD COLUMN language text CHECK (language ~ '^[a-z]{2,3}$');
