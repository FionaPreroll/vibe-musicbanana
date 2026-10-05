-- Undoing merges. Every merge is an entry in merge_op, and while it runs, the
-- triggers below write each row it inserts, changes or deletes in the catalog
-- and the listens to merge_change: the row's key and, for a change, the old and
-- new values of the columns that changed. Undoing a merge plays those back from
-- the last to the first, see undo_merge().
CREATE TABLE merge_op (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    kind       text        NOT NULL,              -- artist, release, recording
    from_id    bigint      NOT NULL,
    into_id    bigint      NOT NULL,
    from_name  text        NOT NULL,
    into_name  text        NOT NULL,
    merged_at  timestamptz NOT NULL DEFAULT now(),
    undone_at  timestamptz
);

CREATE TABLE merge_change (
    op_id  bigint NOT NULL REFERENCES merge_op(id) ON DELETE CASCADE,
    seq    bigint GENERATED ALWAYS AS IDENTITY,
    tbl    text   NOT NULL,
    action text   NOT NULL,                       -- INSERT, UPDATE, DELETE
    key    jsonb  NOT NULL,                       -- the primary key of the row (after the change)
    old    jsonb,                                 -- UPDATE: changed columns before; DELETE: the row
    new    jsonb,                                 -- UPDATE: changed columns after; INSERT: the row
    PRIMARY KEY (op_id, seq)
);
CREATE INDEX merge_change_row ON merge_change (tbl, key);

-- Writes a change to the merge named by the setting musicbanana.merge_op. The
-- trigger arguments are the columns of the table's primary key.
CREATE FUNCTION journal_merge_change() RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    op  bigint := current_setting('musicbanana.merge_op')::bigint;
    o   jsonb  := CASE WHEN TG_OP <> 'INSERT' THEN to_jsonb(OLD) END;
    n   jsonb  := CASE WHEN TG_OP <> 'DELETE' THEN to_jsonb(NEW) END;
    k   jsonb;
BEGIN
    SELECT jsonb_object_agg(c, coalesce(n, o) -> c) INTO k FROM unnest(TG_ARGV) c;
    IF TG_OP = 'UPDATE' THEN
        SELECT jsonb_object_agg(e.key, e.value) INTO o
          FROM jsonb_each(to_jsonb(OLD)) e WHERE e.value IS DISTINCT FROM n -> e.key;
        SELECT jsonb_object_agg(e.key, e.value) INTO n
          FROM jsonb_each(to_jsonb(NEW)) e WHERE e.value IS DISTINCT FROM to_jsonb(OLD) -> e.key;
        IF o IS NULL THEN
            RETURN NULL;
        END IF;
    END IF;
    INSERT INTO merge_change (op_id, tbl, action, key, old, new)
    VALUES (op, TG_TABLE_NAME, TG_OP, k, o, n);
    RETURN NULL;
END
$$;

-- Only while a merge runs; otherwise the trigger does not even start.
CREATE TRIGGER journal AFTER INSERT OR UPDATE OR DELETE ON listen FOR EACH ROW
    WHEN (coalesce(current_setting('musicbanana.merge_op', true), '') <> '')
    EXECUTE FUNCTION journal_merge_change('id');
CREATE TRIGGER journal AFTER INSERT OR UPDATE OR DELETE ON artist FOR EACH ROW
    WHEN (coalesce(current_setting('musicbanana.merge_op', true), '') <> '')
    EXECUTE FUNCTION journal_merge_change('id');
CREATE TRIGGER journal AFTER INSERT OR UPDATE OR DELETE ON release FOR EACH ROW
    WHEN (coalesce(current_setting('musicbanana.merge_op', true), '') <> '')
    EXECUTE FUNCTION journal_merge_change('id');
CREATE TRIGGER journal AFTER INSERT OR UPDATE OR DELETE ON recording FOR EACH ROW
    WHEN (coalesce(current_setting('musicbanana.merge_op', true), '') <> '')
    EXECUTE FUNCTION journal_merge_change('id');
CREATE TRIGGER journal AFTER INSERT OR UPDATE OR DELETE ON artist_alias FOR EACH ROW
    WHEN (coalesce(current_setting('musicbanana.merge_op', true), '') <> '')
    EXECUTE FUNCTION journal_merge_change('name_key');
CREATE TRIGGER journal AFTER INSERT OR UPDATE OR DELETE ON release_alias FOR EACH ROW
    WHEN (coalesce(current_setting('musicbanana.merge_op', true), '') <> '')
    EXECUTE FUNCTION journal_merge_change('artist_id', 'title_key');
CREATE TRIGGER journal AFTER INSERT OR UPDATE OR DELETE ON recording_alias FOR EACH ROW
    WHEN (coalesce(current_setting('musicbanana.merge_op', true), '') <> '')
    EXECUTE FUNCTION journal_merge_change('artist_id', 'title_key');
CREATE TRIGGER journal AFTER INSERT OR UPDATE OR DELETE ON artist_mbid FOR EACH ROW
    WHEN (coalesce(current_setting('musicbanana.merge_op', true), '') <> '')
    EXECUTE FUNCTION journal_merge_change('mbid');
CREATE TRIGGER journal AFTER INSERT OR UPDATE OR DELETE ON release_mbid FOR EACH ROW
    WHEN (coalesce(current_setting('musicbanana.merge_op', true), '') <> '')
    EXECUTE FUNCTION journal_merge_change('artist_id', 'mbid');
CREATE TRIGGER journal AFTER INSERT OR UPDATE OR DELETE ON recording_mbid FOR EACH ROW
    WHEN (coalesce(current_setting('musicbanana.merge_op', true), '') <> '')
    EXECUTE FUNCTION journal_merge_change('artist_id', 'mbid');

-- Plays the changes of merge `op` back, the last first. A row that has changed
-- again since (a rename, a new spelling taken by another entry) keeps its
-- current values; the function returns how many changes were left so. Merges
-- after `op` that changed the same rows have to be undone first.
CREATE FUNCTION undo_merge(op bigint) RETURNS bigint
LANGUAGE plpgsql
AS $$
DECLARE
    c       record;
    later   bigint;
    keys    text;
    cols    text;
    applied bigint;
    kept    bigint := 0;
BEGIN
    PERFORM FROM merge_op WHERE id = op AND undone_at IS NULL FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'there is no merge % that is not undone', op;
    END IF;
    SELECT min(l.op_id) INTO later
      FROM merge_change m
      JOIN merge_change l ON l.tbl = m.tbl AND l.key = m.key AND l.op_id > m.op_id
      JOIN merge_op o ON o.id = l.op_id AND o.undone_at IS NULL
     WHERE m.op_id = op;
    IF later IS NOT NULL THEN
        RAISE EXCEPTION 'merge % changed the same entries later; undo that one first', later;
    END IF;

    FOR c IN SELECT * FROM merge_change WHERE op_id = op ORDER BY seq DESC LOOP
        -- The row by its key, given as $1 (an index lookup).
        SELECT string_agg(format('t.%1$I = (jsonb_populate_record(NULL::%2$I, $1)).%1$I', k, c.tbl), ' AND ')
          INTO keys
          FROM jsonb_object_keys(c.key) k;
        -- The columns to write back, without generated ones.
        SELECT string_agg(quote_ident(a.attname), ', ') INTO cols
          FROM pg_attribute a
         WHERE a.attrelid = c.tbl::regclass AND a.attnum > 0 AND NOT a.attisdropped
           AND a.attgenerated = ''
           AND (c.action <> 'UPDATE' OR c.old ? a.attname);
        IF c.action = 'INSERT' THEN
            EXECUTE format('DELETE FROM %I t WHERE %s', c.tbl, keys) USING c.key;
            GET DIAGNOSTICS applied = ROW_COUNT;
        ELSIF c.action = 'DELETE' THEN
            EXECUTE format(
                'INSERT INTO %I (%s) SELECT %s FROM jsonb_populate_record(NULL::%I, $1) ON CONFLICT DO NOTHING',
                c.tbl, cols, cols, c.tbl) USING c.old;
            GET DIAGNOSTICS applied = ROW_COUNT;
        ELSIF cols IS NULL THEN
            applied := 1;  -- only generated columns changed
        ELSE
            -- Only where the row still has the values the merge gave it.
            EXECUTE format(
                'UPDATE %I t SET (%s) = (SELECT %s FROM jsonb_populate_record(NULL::%I, $2))
                  WHERE %s
                    AND (SELECT jsonb_object_agg(k, to_jsonb(t) -> k) FROM jsonb_object_keys($3) k) = $3',
                c.tbl, cols, cols, c.tbl, keys)
                USING c.key, c.old, c.new;
            GET DIAGNOSTICS applied = ROW_COUNT;
        END IF;
        IF applied = 0 THEN
            kept := kept + 1;
        END IF;
    END LOOP;

    UPDATE merge_op SET undone_at = now() WHERE id = op;
    RETURN kept;
END
$$;
