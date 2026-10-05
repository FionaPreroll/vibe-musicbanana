-- Where a listen came from: its client without the version, so that
-- "Navidrome 0.64.2 (abc)" and "Navidrome 0.65.0" are both "Navidrome", while
-- "Spotify via YourSpotify" and "import:php-2016" stay as they are. '' for
-- listens that named no client.
CREATE FUNCTION listen_source(client text) RETURNS text
LANGUAGE sql IMMUTABLE PARALLEL SAFE
AS $$
    SELECT coalesce(regexp_replace(client, '\s+v?[0-9].*$', ''), '')
$$;
