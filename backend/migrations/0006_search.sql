-- A name as the search compares it: lower case, without the accents of Latin
-- letters, so that "arzte" finds "Die Ärzte" and "bjork" finds "Björk".
CREATE FUNCTION search_fold(name text) RETURNS text
LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE
AS $$
    SELECT translate(
        replace(replace(replace(lower(normalize(name, NFC)), 'ß', 'ss'), 'æ', 'ae'), 'œ', 'oe'),
        'áàâäãåāăąçćčďđéèêëēėęěğíìîïīįıłľĺñńňóòôöõøōőŕřśšşșťţțúùûüūůűųýÿźżž',
        'aaaaaaaaacccddeeeeeeeegiiiiiiilllnnnoooooooorrsssstttuuuuuuuuyyzzz'
    )
$$;

-- Folded once per name rather than per search.
ALTER TABLE artist    ADD COLUMN search_name text NOT NULL GENERATED ALWAYS AS (search_fold(name)) STORED;
ALTER TABLE release   ADD COLUMN search_name text NOT NULL GENERATED ALWAYS AS (search_fold(title)) STORED;
ALTER TABLE recording ADD COLUMN search_name text NOT NULL GENERATED ALWAYS AS (search_fold(title)) STORED;
