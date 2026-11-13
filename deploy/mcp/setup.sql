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
COMMIT;
