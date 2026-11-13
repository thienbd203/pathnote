\set ON_ERROR_STOP on
BEGIN;
CREATE SCHEMA IF NOT EXISTS pathnote_mcp;
REVOKE ALL ON SCHEMA pathnote_mcp FROM PUBLIC;

-- Keep an identity row even when the owner has no vocabulary yet.
CREATE OR REPLACE VIEW pathnote_mcp.vocabulary AS
SELECT u.google_id, owned.word_key
FROM public.users u
LEFT JOIN (
    SELECT p.user_id, v.word_key
    FROM public.vocabulary_cards v
    JOIN public.sessions s ON s.id = v.session_id
    JOIN public.places p ON p.id = s.place_id
    LEFT JOIN public.courses c ON c.id = s.course_id AND c.place_id = s.place_id
    WHERE v.active AND s.deleted_at IS NULL AND p.deleted_at IS NULL
      AND (s.course_id IS NULL OR c.deleted_at IS NULL)
) owned ON owned.user_id = u.id
WHERE u.id = :'owner_id'::bigint AND u.google_id IS NOT NULL;

REVOKE ALL ON pathnote_mcp.vocabulary FROM PUBLIC;
GRANT USAGE ON SCHEMA pathnote_mcp TO pathnote_mcp;
GRANT SELECT ON pathnote_mcp.vocabulary TO pathnote_mcp;
COMMIT;
