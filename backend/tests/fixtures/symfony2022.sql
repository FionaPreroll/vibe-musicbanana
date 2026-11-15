-- A tiny, made-up musicbanana-symfony database (PostgreSQL), shaped like the
-- Doctrine entities of the 2021-2022 version. No real data.

CREATE TABLE usr (
    id         serial PRIMARY KEY,
    name       varchar(180) NOT NULL UNIQUE,
    email      varchar(180) NOT NULL UNIQUE,
    password   varchar(255) NOT NULL,
    registered timestamp(0) NOT NULL,
    avatar     varchar(255),
    roles      json NOT NULL
);

CREATE TABLE profile (
    id        serial PRIMARY KEY,
    usr       int NOT NULL REFERENCES usr(id),
    name      varchar(255) NOT NULL,
    is_public boolean NOT NULL,
    created   timestamp(0) NOT NULL,
    UNIQUE (name, usr)
);

CREATE TABLE artist (
    id   serial PRIMARY KEY,
    name varchar(255) NOT NULL UNIQUE
);

CREATE TABLE album (
    id        serial PRIMARY KEY,
    artist_id int NOT NULL REFERENCES artist(id),
    title     varchar(255) NOT NULL,
    UNIQUE (artist_id, title)
);

CREATE TABLE track (
    id          serial PRIMARY KEY,
    artist_id   int NOT NULL REFERENCES artist(id),
    album_id    int REFERENCES album(id),
    title       varchar(255) NOT NULL,
    length      int,
    tracknumber int
);

CREATE TABLE listen (
    id                serial PRIMARY KEY,
    profile_id        int NOT NULL REFERENCES profile(id),
    date              timestamp(0) with time zone NOT NULL,
    "timestamp"       int NOT NULL,
    artist_name       varchar(255) NOT NULL,
    album_artist_name varchar(255),
    album_title       varchar(255),
    track_title       varchar(255) NOT NULL,
    length            int,
    tracknumber       int,
    artist_id         int REFERENCES artist(id),
    album_id          int REFERENCES album(id),
    track_id          int REFERENCES track(id),
    UNIQUE (profile_id, "timestamp")
);

INSERT INTO usr (id, name, email, password, registered, roles) VALUES
    (1, 'fiona', 'fiona@example.org', '$2y$13$abcdefghijklmnopqrstuuJ8z9y8x7w6v5u4t3s2r1q0p9o8n7m6l', '2007-08-12 14:28:00', '["ROLE_ADMIN"]'),
    (2, 'Emma', 'emma@example.org', '$2y$13$abcdefghijklmnopqrstuuJ8z9y8x7w6v5u4t3s2r1q0p9o8n7m6l', '2008-01-01 12:00:00', '[]'),
    (3, 'ghost', 'ghost@example.org', '$2y$13$abcdefghijklmnopqrstuuJ8z9y8x7w6v5u4t3s2r1q0p9o8n7m6l', '2020-01-01 12:00:00', '[]');

INSERT INTO profile (id, usr, name, is_public, created) VALUES
    (1, 1, 'Default', true, '2007-08-12 14:28:00'),
    (2, 1, 'Arbeit', false, '2021-02-01 08:00:00'),
    (3, 2, 'emma', true, '2008-01-01 12:00:00'),
    (4, 3, 'ghost', true, '2020-01-01 12:00:00');

INSERT INTO artist (id, name) VALUES (1, 'Die Ärzte'), (2, 'Björk');
INSERT INTO album (id, artist_id, title) VALUES (1, 1, 'Die Bestie in Menschengestalt'), (2, 2, 'Post');
INSERT INTO track (id, artist_id, album_id, title, length, tracknumber) VALUES
    (1, 1, 1, 'Schrei nach Liebe', 250, 3),
    (2, 1, NULL, 'Zu spät', 150, NULL),
    (3, 2, 2, 'Hyperballad', 321, 6),
    (4, 1, NULL, 'Westerland', 230, NULL);

INSERT INTO listen (profile_id, date, "timestamp", artist_name, album_artist_name, album_title,
                    track_title, length, tracknumber, artist_id, album_id, track_id) VALUES
    -- Also in the 2016 import, at the same second.
    (1, to_timestamp(1400000000), 1400000000, 'Die Ärzte', 'Die Ärzte', 'Die Bestie in Menschengestalt',
        'Schrei nach Liebe', 250, 3, 1, 1, 1),
    -- Double-encoded, as some rows came through the older imports.
    (1, to_timestamp(1400000300), 1400000300, 'Die Ã„rzte', NULL, NULL, 'Zu spÃ¤t', 150, NULL, 1, NULL, 2),
    -- Only here; the length in milliseconds.
    (1, to_timestamp(1600000000), 1600000000, 'Björk', 'Björk', 'Post', 'Hyperballad', 321000, 6, 2, 2, 3),
    -- Navidrome sent the same play to the new musicbanana half a minute later.
    (1, to_timestamp(1650000000), 1650000000, 'Die Ärzte', NULL, NULL, 'Westerland', 230, NULL, 1, NULL, 4),
    -- A second profile, which the new account does not have yet.
    (2, to_timestamp(1620000000), 1620000000, 'Björk', NULL, 'Post', 'Hyperballad', 321, 6, 2, 2, 3),
    (2, to_timestamp(1620000400), 1620000400, 'Björk', NULL, 'Post', 'Hyperballad', 321, 6, 2, 2, 3),
    -- Emma's only profile goes to her default one.
    (3, to_timestamp(1500000000), 1500000000, 'Die Ärzte', NULL, NULL, 'Westerland', NULL, NULL, 1, NULL, 4),
    -- An hour off from her 2016 listen at 1300003600.
    (3, to_timestamp(1300000000), 1300000000, 'Björk', NULL, NULL, 'Hyperballad', 321, NULL, 2, NULL, 3),
    -- Nobody to import this for.
    (4, to_timestamp(1610000000), 1610000000, 'Björk', NULL, NULL, 'Hyperballad', 321, NULL, 2, NULL, 3);

SELECT setval('usr_id_seq', 3), setval('profile_id_seq', 4), setval('artist_id_seq', 2),
       setval('album_id_seq', 2), setval('track_id_seq', 4);
