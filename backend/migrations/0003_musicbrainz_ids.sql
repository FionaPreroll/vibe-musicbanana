-- The MusicBrainz IDs a catalog entry is known under, like the spellings in the
-- *_alias tables. They replace the mbid columns, which nothing filled so far: an
-- entry has several IDs after a merge, and albums of different artists share one,
-- since a compilation is an album of each artist on it here. So albums and tracks
-- are looked up by artist and ID, as by artist and title.
--
-- Scrobbles that come with IDs keep apart what has the same name, such as two
-- artists called Nirvana or two albums called "Weezer"; see src/catalog.rs.

CREATE TABLE artist_mbid (
    mbid      uuid   PRIMARY KEY,
    artist_id bigint NOT NULL REFERENCES artist(id)
);
CREATE INDEX artist_mbid_artist ON artist_mbid (artist_id);

-- The ID of the release group (the album in all its editions), not of an edition.
CREATE TABLE release_mbid (
    artist_id  bigint NOT NULL REFERENCES artist(id),
    mbid       uuid   NOT NULL,
    release_id bigint NOT NULL REFERENCES release(id),
    PRIMARY KEY (artist_id, mbid)
);
CREATE INDEX release_mbid_release ON release_mbid (release_id);

CREATE TABLE recording_mbid (
    artist_id    bigint NOT NULL REFERENCES artist(id),
    mbid         uuid   NOT NULL,
    recording_id bigint NOT NULL REFERENCES recording(id),
    PRIMARY KEY (artist_id, mbid)
);
CREATE INDEX recording_mbid_recording ON recording_mbid (recording_id);

ALTER TABLE artist DROP COLUMN mbid;
ALTER TABLE release DROP COLUMN mbid;
ALTER TABLE recording DROP COLUMN mbid;
