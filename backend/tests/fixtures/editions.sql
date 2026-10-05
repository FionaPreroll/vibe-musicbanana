-- On top of profiles.sql, as an import before edition notes were left out (such
-- as Spotify's) left them: Geräusch and Unrockbar under a second, noted title;
-- Deine Schuld noted, but with another MusicBrainz ID than the plain one;
-- Hyperballad only under two noted titles; and a live version, which stays.
INSERT INTO release (id, title, artist_id) OVERRIDING SYSTEM VALUE VALUES
    (10, 'Geräusch (Deluxe Edition)', 1);

INSERT INTO recording (id, title, artist_id) OVERRIDING SYSTEM VALUE VALUES
    (10, 'Unrockbar - Remastered 2011', 1),
    (11, 'Deine Schuld - 2012 Remaster', 1),
    (12, 'Human Behaviour (Live)', 2),
    (13, 'Hyperballad - Remastered', 2),
    (14, 'Hyperballad (2015 Remaster)', 2);

INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, album_raw, artist_id, recording_id, release_id) VALUES
    (1, '2020-01-01 12:00:00+00', 'Die Ärzte', 'Unrockbar - Remastered 2011', 'Geräusch (Deluxe Edition)', 1, 10, 10),
    (1, '2020-01-02 12:00:00+00', 'Die Ärzte', 'Deine Schuld - 2012 Remaster', 'Geräusch (Deluxe Edition)', 1, 11, 10),
    (1, '2020-01-03 12:00:00+00', 'Björk', 'Human Behaviour (Live)', NULL, 2, 12, NULL),
    (1, '2020-01-04 12:00:00+00', 'Björk', 'Hyperballad - Remastered', NULL, 2, 13, NULL),
    (1, '2020-01-05 12:00:00+00', 'Björk', 'Hyperballad (2015 Remaster)', NULL, 2, 14, NULL);

INSERT INTO release_alias (artist_id, title_key, release_id) VALUES
    (1, 'geräusch (deluxe edition)', 10);

INSERT INTO recording_alias (artist_id, title_key, recording_id) VALUES
    (1, 'unrockbar - remastered 2011', 10),
    (1, 'deine schuld - 2012 remaster', 11),
    (2, 'human behaviour (live)', 12),
    (2, 'hyperballad - remastered', 13),
    (2, 'hyperballad (2015 remaster)', 14);

INSERT INTO recording_mbid (artist_id, mbid, recording_id) VALUES
    (1, '00000000-0000-0000-0000-000000000002', 2),
    (1, '00000000-0000-0000-0000-000000000011', 11);

SELECT setval(pg_get_serial_sequence('release', 'id'), 10);
SELECT setval(pg_get_serial_sequence('recording', 'id'), 14);
