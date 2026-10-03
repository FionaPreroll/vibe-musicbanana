-- Two accounts, one of them with a second, private profile.
INSERT INTO account (id, username, email, password_hash) OVERRIDING SYSTEM VALUE VALUES
    (1, 'Fiona', 'fiona@example.org', 'unused'),
    (2, 'alex', 'alex@example.org', 'unused');

INSERT INTO profile (id, account_id, slug, name, visibility) OVERRIDING SYSTEM VALUE VALUES
    (1, 1, 'default', 'Default', 'public'),
    (2, 1, 'arbeit', 'Arbeit', 'private'),
    (3, 2, 'default', 'Default', 'public');

INSERT INTO artist (id, name) OVERRIDING SYSTEM VALUE VALUES
    (1, 'Die Ärzte'),
    (2, 'Björk'),
    (3, 'Tiësto');

INSERT INTO release (id, title, artist_id) OVERRIDING SYSTEM VALUE VALUES
    (1, 'Geräusch', 1),
    (2, 'Debut', 2);

INSERT INTO recording (id, title, artist_id) OVERRIDING SYSTEM VALUE VALUES
    (1, 'Unrockbar', 1),
    (2, 'Deine Schuld', 1),
    (3, 'Human Behaviour', 2),
    (4, 'Jóga', 2),
    (5, 'Adagio for Strings', 3);

INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, album_raw, artist_id, recording_id, release_id) VALUES
    (1, '2015-06-01 12:00:00+00', 'Die Ärzte', 'Unrockbar', 'Geräusch', 1, 1, 1),
    (1, '2015-06-02 12:00:00+00', 'Die Ärzte', 'Deine Schuld', 'Geräusch', 1, 2, 1),
    (1, '2015-06-03 12:00:00+00', 'Björk', 'Human Behaviour', 'Debut', 2, 3, 2),
    -- Still 2015 in UTC, already 2016 in Berlin.
    (1, '2015-12-31 23:30:00+00', 'Tiësto', 'Adagio for Strings', NULL, 3, 5, NULL),
    (1, '2016-03-01 12:00:00+00', 'Björk', 'Human Behaviour', 'Debut', 2, 3, 2),
    (1, '2016-03-02 12:00:00+00', 'Björk', 'Jóga', NULL, 2, 4, NULL),
    (1, '2016-03-03 12:00:00+00', 'Die Ärzte', 'Unrockbar', 'Geräusch', 1, 1, 1),
    (1, '2016-03-04 12:00:00+00', 'Die Ärzte', 'Unrockbar', NULL, 1, 1, NULL),
    (2, '2016-04-01 12:00:00+00', 'Tiësto', 'Adagio for Strings', NULL, 3, 5, NULL),
    (3, '2016-04-02 12:00:00+00', 'Die Ärzte', 'Deine Schuld', NULL, 1, 2, NULL);
