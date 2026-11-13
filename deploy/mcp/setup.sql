\set ON_ERROR_STOP on
BEGIN;
CREATE SCHEMA IF NOT EXISTS pathnote_mcp;
REVOKE ALL ON SCHEMA pathnote_mcp FROM PUBLIC;

-- A NULL word_key row identifies a linked account with no matching words.
CREATE OR REPLACE FUNCTION pathnote_mcp.check_vocabulary(
    subject text, candidate_keys text[]
)
RETURNS TABLE (word_key text)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = pg_catalog, pg_temp
AS $$
    SELECT owned.word_key
    FROM public.users u
    LEFT JOIN LATERAL (
        SELECT DISTINCT v.word_key
        FROM public.places p
        JOIN public.sessions s ON s.place_id = p.id
        JOIN public.vocabulary_cards v ON v.session_id = s.id
        LEFT JOIN public.courses c ON c.id = s.course_id AND c.place_id = s.place_id
        WHERE p.user_id = u.id
          AND p.deleted_at IS NULL
          AND s.deleted_at IS NULL
          AND (s.course_id IS NULL OR c.deleted_at IS NULL)
          AND v.active
          AND v.word_key = ANY(candidate_keys)
    ) owned ON true
    WHERE u.google_id = subject;
$$;

REVOKE ALL ON FUNCTION pathnote_mcp.check_vocabulary(text, text[]) FROM PUBLIC;
DROP VIEW IF EXISTS pathnote_mcp.vocabulary;
GRANT USAGE ON SCHEMA pathnote_mcp TO pathnote_mcp;
GRANT EXECUTE ON FUNCTION pathnote_mcp.check_vocabulary(text, text[]) TO pathnote_mcp;

-- Index the existing projection without rewriting its words, hashes or FSRS data.
CREATE OR REPLACE FUNCTION pathnote_mcp.normalize_word(value text)
RETURNS text
LANGUAGE sql
IMMUTABLE STRICT PARALLEL SAFE
SET search_path = pg_catalog, pg_temp
RETURN normalize(lower(btrim(regexp_replace(
    value, U&'[\0009-\000D \0085\00A0\1680\2000-\200A\2028\2029\202F\205F\3000]+', ' ', 'g'
)) COLLATE pg_catalog."und-x-icu"));

CREATE INDEX IF NOT EXISTS vocabulary_cards_mcp_normalized_active_idx
    ON public.vocabulary_cards (md5(pathnote_mcp.normalize_word(word)), session_id)
    WHERE active;

CREATE TABLE IF NOT EXISTS pathnote_mcp.generated_batches (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id bigint NOT NULL REFERENCES public.users(id) ON DELETE CASCADE,
    source text NOT NULL CHECK (source = 'chatgpt_daily'),
    batch_date date NOT NULL,
    idempotency_key text NOT NULL CHECK (char_length(idempotency_key) BETWEEN 1 AND 128),
    match_mode text NOT NULL CHECK (match_mode IN ('exact', 'lemma', 'word_family')),
    words text[] NOT NULL CHECK (cardinality(words) = 20),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (user_id, source, batch_date),
    UNIQUE (user_id, idempotency_key),
    UNIQUE (id, user_id)
);

CREATE TABLE IF NOT EXISTS pathnote_mcp.generated_items (
    batch_id bigint NOT NULL,
    user_id bigint NOT NULL,
    word text NOT NULL,
    normalized_word text NOT NULL,
    exact_key text GENERATED ALWAYS AS (md5(normalized_word)) STORED,
    PRIMARY KEY (user_id, exact_key),
    FOREIGN KEY (batch_id, user_id)
        REFERENCES pathnote_mcp.generated_batches(id, user_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS generated_items_batch_idx ON pathnote_mcp.generated_items(batch_id);

CREATE OR REPLACE FUNCTION pathnote_mcp.find_matches(google_subject text, keys text[])
RETURNS TABLE (word text, source text)
LANGUAGE sql STABLE SECURITY DEFINER
SET search_path = pg_catalog, pg_temp
AS $$
    WITH account AS (
        SELECT u.id FROM public.users u WHERE u.google_id = google_subject
    )
    SELECT NULL::text, NULL::text FROM account
    UNION ALL
    SELECT DISTINCT v.word, 'vocabulary'::text
    FROM account a
    JOIN public.places p ON p.user_id = a.id AND p.deleted_at IS NULL
    JOIN public.sessions s ON s.place_id = p.id AND s.deleted_at IS NULL
    JOIN public.vocabulary_cards v ON v.session_id = s.id AND v.active
    LEFT JOIN public.courses c ON c.id = s.course_id AND c.place_id = s.place_id
    WHERE (s.course_id IS NULL OR c.deleted_at IS NULL)
      AND md5(pathnote_mcp.normalize_word(v.word)) = ANY(ARRAY(SELECT md5(key) FROM unnest(keys) key))
      AND pathnote_mcp.normalize_word(v.word) = ANY(keys)
    UNION ALL
    SELECT i.word, 'generated_history'::text
    FROM account a
    JOIN pathnote_mcp.generated_items i ON i.user_id = a.id
    WHERE i.exact_key = ANY(ARRAY(SELECT md5(key) FROM unnest(keys) key))
      AND i.normalized_word = ANY(keys);
$$;

CREATE OR REPLACE FUNCTION pathnote_mcp.lock_account(google_subject text)
RETURNS bigint
LANGUAGE sql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, pg_temp
AS $$
    SELECT id FROM public.users WHERE google_id = google_subject FOR NO KEY UPDATE;
$$;

CREATE OR REPLACE FUNCTION pathnote_mcp.find_batch(
    google_subject text, day date, request_key text
)
RETURNS TABLE (id bigint, batch_date date, idempotency_key text, match_mode text, words text[])
LANGUAGE sql STABLE SECURITY DEFINER
SET search_path = pg_catalog, pg_temp
AS $$
    SELECT b.id, b.batch_date, b.idempotency_key, b.match_mode, b.words
    FROM pathnote_mcp.generated_batches b
    JOIN public.users u ON u.id = b.user_id
    WHERE u.google_id = google_subject
      AND (b.idempotency_key = request_key OR (b.source = 'chatgpt_daily' AND b.batch_date = day))
    ORDER BY (b.idempotency_key = request_key) DESC NULLS LAST, b.id;
$$;

CREATE OR REPLACE FUNCTION pathnote_mcp.store_batch(
    google_subject text, day date, request_key text, mode text, entries text[]
)
RETURNS bigint
LANGUAGE sql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, pg_temp
AS $$
    WITH batch AS (
        INSERT INTO pathnote_mcp.generated_batches
            (user_id, source, batch_date, idempotency_key, match_mode, words)
        SELECT u.id, 'chatgpt_daily', day, request_key, mode, entries
        FROM public.users u
        WHERE u.google_id = google_subject
        RETURNING id, user_id
    ), items AS (
        INSERT INTO pathnote_mcp.generated_items (batch_id, user_id, word, normalized_word)
        SELECT b.id, b.user_id, item.word, pathnote_mcp.normalize_word(item.word)
        FROM batch b CROSS JOIN unnest(entries) AS item(word)
    )
    SELECT id FROM batch;
$$;

REVOKE ALL ON ALL FUNCTIONS IN SCHEMA pathnote_mcp FROM PUBLIC;
-- PostgreSQL evaluates the index expression with the role writing the card.
-- The pure normalizer reads no tables. The schema itself remains private.
GRANT EXECUTE ON FUNCTION pathnote_mcp.normalize_word(text) TO PUBLIC;
GRANT EXECUTE ON FUNCTION pathnote_mcp.find_matches(text, text[]),
    pathnote_mcp.lock_account(text), pathnote_mcp.find_batch(text, date, text),
    pathnote_mcp.store_batch(text, date, text, text, text[]) TO pathnote_mcp;
COMMIT;
