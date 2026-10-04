-- The name an artist had when an ID came to it. An artist ID only counts under
-- a name the artist is known by (see src/catalog.rs), and this one stays such a
-- name when the artist is renamed: the second of two artists called Nirvana has
-- no spelling of its own, as the name leads to the first one.
ALTER TABLE artist_mbid ADD COLUMN name_key text;

-- No artist was renamed before, so that is the name it has now, folded like
-- catalog::name_key as far as SQL goes.
UPDATE artist_mbid m
   SET name_key = btrim(regexp_replace(lower(normalize(a.name, NFKC)), '\s+', ' ', 'g'))
  FROM artist a
 WHERE a.id = m.artist_id;

ALTER TABLE artist_mbid ALTER COLUMN name_key SET NOT NULL;
