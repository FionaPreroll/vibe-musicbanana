-- A tiny, made-up musicbanana-symfony database (PostgreSQL 12), shaped like the
-- tables of the 2021-2022 version. No real data.

CREATE TABLE users (
    id         serial PRIMARY KEY,
    name       varchar(255) NOT NULL UNIQUE,
    email      varchar(255) NOT NULL UNIQUE,
    password   varchar(255) NOT NULL,
    md5hash    varchar(32) DEFAULT NULL,
    registered timestamp(0) with time zone NOT NULL,
    avatar     varchar(1024) DEFAULT NULL,
    is_admin   boolean NOT NULL
);

-- Each account's first profile is called "_".
CREATE TABLE profile (
    id        serial PRIMARY KEY,
    user_id   int REFERENCES users(id) ON DELETE CASCADE,
    name      varchar(255) NOT NULL,
    is_public boolean NOT NULL,
    created   timestamp(0) with time zone NOT NULL,
    UNIQUE (name, user_id)
);

CREATE TABLE artist (
    id   serial PRIMARY KEY,
    name varchar(255) NOT NULL
);

CREATE TABLE album (
    id        serial PRIMARY KEY,
    artist_id int REFERENCES artist(id) ON DELETE CASCADE,
    title     varchar(255) NOT NULL
);

CREATE TABLE track (
    id          serial PRIMARY KEY,
    artist_id   int REFERENCES artist(id) ON DELETE CASCADE,
    album_id    int REFERENCES album(id) ON DELETE CASCADE,
    title       varchar(255) NOT NULL,
    length      int NOT NULL,
    tracknumber int NOT NULL
);

-- length is 0 when unknown.
CREATE TABLE scrobble (
    id          serial PRIMARY KEY,
    profile_id  int REFERENCES profile(id) ON DELETE CASCADE,
    artist_id   int REFERENCES artist(id) ON DELETE CASCADE,
    album_id    int REFERENCES album(id) ON DELETE CASCADE,
    track_id    int REFERENCES track(id) ON DELETE CASCADE,
    date        timestamp(0) with time zone NOT NULL,
    "timestamp" int NOT NULL,
    artist_name varchar(1024) NOT NULL,
    album_title varchar(1024) DEFAULT NULL,
    track_title varchar(1024) NOT NULL,
    length      int NOT NULL,
    tracknumber int,
    CONSTRAINT profile_timestamp_unique UNIQUE ("timestamp", profile_id)
);

CREATE TABLE nowplaying (
    profile_id  int REFERENCES profile(id) ON DELETE CASCADE,
    date        timestamp(0) with time zone NOT NULL,
    artist_name varchar(1024) NOT NULL,
    track_title varchar(1024) NOT NULL,
    length      int NOT NULL
);

INSERT INTO users (id, name, email, password, registered, is_admin) VALUES
    (1, 'fiona', 'fiona@example.org', '$2y$13$abcdefghijklmnopqrstuuJ8z9y8x7w6v5u4t3s2r1q0p9o8n7m6l', '2007-08-12 14:28:00+00', true),
    (2, 'Emma', 'emma@example.org', '$2y$13$abcdefghijklmnopqrstuuJ8z9y8x7w6v5u4t3s2r1q0p9o8n7m6l', '2008-01-01 12:00:00+00', false),
    (3, 'ghost', 'ghost@example.org', '$2y$13$abcdefghijklmnopqrstuuJ8z9y8x7w6v5u4t3s2r1q0p9o8n7m6l', '2020-01-01 12:00:00+00', false);

INSERT INTO profile (id, user_id, name, is_public, created) VALUES
    (1, 1, '_', true, '2007-08-12 14:28:00+00'),
    (2, 1, 'Arbeit', false, '2021-02-01 08:00:00+00'),
    (3, 2, 'emma', true, '2008-01-01 12:00:00+00'),
    (4, 3, '_', true, '2020-01-01 12:00:00+00'),
    (5, 1, 'leer', true, '2021-03-01 08:00:00+00');

INSERT INTO artist (id, name) VALUES (1, 'Die Ärzte'), (2, 'Björk');
INSERT INTO album (id, artist_id, title) VALUES (1, 1, 'Die Bestie in Menschengestalt'), (2, 2, 'Post');
INSERT INTO track (id, artist_id, album_id, title, length, tracknumber) VALUES
    (1, 1, 1, 'Schrei nach Liebe', 250, 3),
    (2, 1, NULL, 'Zu spät', 150, 0),
    (3, 2, 2, 'Hyperballad', 321, 6),
    (4, 1, NULL, 'Westerland', 230, 0);

INSERT INTO scrobble (profile_id, date, "timestamp", artist_name, album_title, track_title,
                      length, tracknumber, artist_id, album_id, track_id) VALUES
    -- Also in the 2016 import, at the same second.
    (1, to_timestamp(1400000000), 1400000000, 'Die Ärzte', 'Die Bestie in Menschengestalt',
        'Schrei nach Liebe', 250, 3, 1, 1, 1),
    -- Double-encoded, as some rows came through the older imports.
    (1, to_timestamp(1400000300), 1400000300, 'Die Ã„rzte', NULL, 'Zu spÃ¤t', 150, NULL, 1, NULL, 2),
    -- Only here; the length in milliseconds.
    (1, to_timestamp(1600000000), 1600000000, 'Björk', 'Post', 'Hyperballad', 321000, 6, 2, 2, 3),
    -- Navidrome sent the same play to the new musicbanana half a minute later.
    (1, to_timestamp(1650000000), 1650000000, 'Die Ärzte', NULL, 'Westerland', 230, NULL, 1, NULL, 4),
    -- A second profile, which the new account does not have yet.
    (2, to_timestamp(1620000000), 1620000000, 'Björk', 'Post', 'Hyperballad', 321, 6, 2, 2, 3),
    (2, to_timestamp(1620000400), 1620000400, 'Björk', 'Post', 'Hyperballad', 321, 6, 2, 2, 3),
    -- Emma's only profile goes to her default one. No length.
    (3, to_timestamp(1500000000), 1500000000, 'Die Ärzte', NULL, 'Westerland', 0, NULL, 1, NULL, 4),
    -- An hour off from her 2016 listen at 1300003600.
    (3, to_timestamp(1300000000), 1300000000, 'Björk', NULL, 'Hyperballad', 321, NULL, 2, NULL, 3),
    -- Nobody to import this for.
    (4, to_timestamp(1610000000), 1610000000, 'Björk', NULL, 'Hyperballad', 321, NULL, 2, NULL, 3),
    -- Belongs to no profile.
    (NULL, to_timestamp(1610000100), 1610000100, 'Björk', NULL, 'Hyperballad', 321, NULL, 2, NULL, 3);

SELECT setval('users_id_seq', 3), setval('profile_id_seq', 5), setval('artist_id_seq', 2),
       setval('album_id_seq', 2), setval('track_id_seq', 4);
