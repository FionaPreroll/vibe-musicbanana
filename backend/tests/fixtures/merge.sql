-- On top of profiles.sql: "Die Aerzte", a second spelling of Die Ärzte, with an
-- album and a track that Die Ärzte has as well and one of each it does not;
-- "Bjork" with "Joga"; and a live version of Unrockbar.
INSERT INTO artist (id, name) OVERRIDING SYSTEM VALUE VALUES
    (4, 'Die Aerzte'),
    (5, 'Bjork');

INSERT INTO release (id, title, artist_id) OVERRIDING SYSTEM VALUE VALUES
    (3, 'Geräusch', 4),
    (4, 'Jazz ist anders', 4);

INSERT INTO recording (id, title, artist_id, length_ms) OVERRIDING SYSTEM VALUE VALUES
    (6, 'Unrockbar', 4, 222000),
    (7, 'Schrei nach Liebe', 4, NULL),
    (8, 'Unrockbar (Live)', 1, NULL),
    (9, 'Joga', 5, NULL);

INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, album_raw, artist_id, recording_id, release_id) VALUES
    (1, '2016-05-01 12:00:00+00', 'Die Aerzte', 'Unrockbar', 'Geräusch', 4, 6, 3),
    (1, '2016-05-02 12:00:00+00', 'Die Aerzte', 'Schrei nach Liebe', 'Jazz ist anders', 4, 7, 4),
    (3, '2016-05-03 12:00:00+00', 'Die Ärzte', 'Unrockbar (Live)', NULL, 1, 8, NULL),
    (1, '2016-05-04 12:00:00+00', 'Bjork', 'Joga', NULL, 5, 9, NULL);

INSERT INTO artist_alias (name_key, artist_id) VALUES
    ('die aerzte', 4),
    ('bjork', 5);

INSERT INTO release_alias (artist_id, title_key, release_id) VALUES
    (4, 'geräusch', 3),
    (4, 'jazz ist anders', 4);

INSERT INTO recording_alias (artist_id, title_key, recording_id) VALUES
    (4, 'unrockbar', 6),
    (4, 'schrei nach liebe', 7),
    (1, 'unrockbar (live)', 8),
    (5, 'joga', 9);

SELECT setval(pg_get_serial_sequence('artist', 'id'), 5);
SELECT setval(pg_get_serial_sequence('release', 'id'), 4);
SELECT setval(pg_get_serial_sequence('recording', 'id'), 9);
