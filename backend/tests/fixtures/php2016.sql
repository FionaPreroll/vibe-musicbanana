-- Small musicbanana-php database with the quirks of the real 2016 dump:
-- double-encoded names next to correct ones, merges via link_to_*_id,
-- album_id = 0 for "no album", per-user listen tables (one empty, one orphaned).

CREATE TABLE mb_user (
  id int(10) unsigned NOT NULL AUTO_INCREMENT,
  user varchar(32) NOT NULL,
  md5_password varchar(32) NOT NULL,
  email varchar(256) NOT NULL,
  registration timestamp NOT NULL DEFAULT current_timestamp(),
  activation varchar(32) NOT NULL DEFAULT 'false',
  avatar varchar(36) NOT NULL,
  realname varchar(128) NOT NULL,
  gender enum('unspecified','male','female') NOT NULL,
  age int(10) unsigned NOT NULL,
  location varchar(128) NOT NULL,
  fruit varchar(128) NOT NULL,
  letter varchar(128) NOT NULL,
  psyc varchar(128) NOT NULL,
  jabber varchar(128) NOT NULL,
  friends varchar(1024) NOT NULL,
  notify enum('none','badtag') NOT NULL DEFAULT 'none',
  PRIMARY KEY (id)
) ENGINE=MyISAM DEFAULT CHARSET=utf8mb3;

CREATE TABLE mb_artists (
  id bigint(20) unsigned NOT NULL AUTO_INCREMENT,
  link_to_artist_id bigint(20) unsigned NOT NULL,
  name varchar(1024) NOT NULL,
  times_played bigint(20) unsigned NOT NULL DEFAULT 0,
  PRIMARY KEY (id)
) ENGINE=MyISAM DEFAULT CHARSET=utf8mb3;

CREATE TABLE mb_albums (
  id bigint(20) unsigned NOT NULL AUTO_INCREMENT,
  link_to_album_id bigint(20) unsigned NOT NULL,
  artist_id bigint(20) unsigned NOT NULL,
  title varchar(1024) NOT NULL,
  times_played bigint(20) unsigned NOT NULL DEFAULT 0,
  PRIMARY KEY (id)
) ENGINE=MyISAM DEFAULT CHARSET=utf8mb3;

CREATE TABLE mb_tracks (
  id bigint(20) unsigned NOT NULL AUTO_INCREMENT,
  link_to_track_id bigint(20) unsigned NOT NULL,
  artist_id bigint(20) unsigned NOT NULL,
  title varchar(1024) NOT NULL,
  album_id bigint(20) unsigned NOT NULL,
  length int(10) unsigned NOT NULL,
  times_played bigint(20) unsigned NOT NULL DEFAULT 0,
  PRIMARY KEY (id)
) ENGINE=MyISAM DEFAULT CHARSET=utf8mb3;

CREATE TABLE mb_usertracks_1 (timestamp bigint(20) unsigned NOT NULL, track_id bigint(20) unsigned NOT NULL, artist_id bigint(20) unsigned NOT NULL, PRIMARY KEY (timestamp)) ENGINE=MyISAM DEFAULT CHARSET=utf8mb3;
CREATE TABLE mb_usertracks_2 (timestamp bigint(20) unsigned NOT NULL, track_id bigint(20) unsigned NOT NULL, artist_id bigint(20) unsigned NOT NULL, PRIMARY KEY (timestamp)) ENGINE=MyISAM DEFAULT CHARSET=utf8mb3;
CREATE TABLE mb_usertracks_3 (timestamp bigint(20) unsigned NOT NULL, track_id bigint(20) unsigned NOT NULL, artist_id bigint(20) unsigned NOT NULL, PRIMARY KEY (timestamp)) ENGINE=MyISAM DEFAULT CHARSET=utf8mb3;
CREATE TABLE mb_usertracks_9 (timestamp bigint(20) unsigned NOT NULL, track_id bigint(20) unsigned NOT NULL, artist_id bigint(20) unsigned NOT NULL, PRIMARY KEY (timestamp)) ENGINE=MyISAM DEFAULT CHARSET=utf8mb3;

-- md5('banana'), md5('apple'), md5('cherry')
INSERT INTO mb_user (id, user, md5_password, email, registration, avatar, realname, gender, age, location, fruit, letter, psyc, jabber, friends) VALUES
  (1, 'fiona', '72b302bf297a228a75730123efef7c41', 'Fiona@Example.org', '2007-08-01 10:00:00', '', '', 'unspecified', 0, '', 'Banane', 'b', '', '', ';2;3;7'),
  (2, 'alex',  '1f3870be274f6c49b3e31a0c6728957f', 'alex@example.org',  '2008-01-01 10:00:00', '', '', 'unspecified', 0, '', '', '', '', '', ';1'),
  (3, 'sam',   'c7a4476fc64b75ead800da9ea2b7d072', '',                  '2009-01-01 10:00:00', '', '', 'unspecified', 0, '', '', '', '', '', ';1');

INSERT INTO mb_artists (id, link_to_artist_id, name, times_played) VALUES
  (1, 0, 'Die Ärzte', 10),
  (2, 0, 'Die Ã„rzte', 3),
  (3, 0, 'die ärzte', 5),
  (4, 0, 'Tiësto', 4),
  (5, 4, 'DJ Tiësto', 1),
  (6, 0, 'Björk', 2),
  (7, 0, 'björk', 1);

INSERT INTO mb_albums (id, link_to_album_id, artist_id, title, times_played) VALUES
  (1, 0, 1, 'Geräusch', 5),
  (2, 0, 2, 'GerÃ¤usch', 1),
  (3, 0, 3, 'Jazz ist anders', 3),
  (4, 3, 3, 'Jazz ist anders (Bonus)', 1);

INSERT INTO mb_tracks (id, link_to_track_id, artist_id, title, album_id, length, times_played) VALUES
  (1, 0, 1, 'Unrockbar', 1, 240, 5),
  (2, 0, 2, 'Unrockbar', 2, 241, 1),
  (3, 0, 3, 'Junge', 3, 200, 2),
  (4, 0, 3, 'Junge', 0, 200, 1),
  (5, 0, 5, 'Adagio for Strings', 0, 400, 1),
  (6, 0, 6, 'Jóga', 0, 300, 2),
  (7, 6, 7, 'Joga', 0, 0, 1),
  (8, 0, 3, 'Westerland', 4, 230, 1);

INSERT INTO mb_usertracks_1 (timestamp, track_id, artist_id) VALUES
  (1187094470, 1, 1), (1187094800, 2, 2), (1187095000, 3, 3), (1187095300, 4, 3),
  (1187095600, 5, 5), (1187096000, 6, 6), (1187096300, 7, 7), (1187096600, 8, 3),
  (1187096900, 99, 1);
INSERT INTO mb_usertracks_2 (timestamp, track_id, artist_id) VALUES
  (1464629818, 1, 1), (1464630100, 6, 6);
INSERT INTO mb_usertracks_9 (timestamp, track_id, artist_id) VALUES
  (1300000000, 1, 1);
