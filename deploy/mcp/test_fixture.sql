-- Use only in a disposable, empty database. The password is for local tests.
\set ON_ERROR_STOP on
CREATE ROLE pathnote_mcp LOGIN PASSWORD 'test';
CREATE TABLE users(id bigint PRIMARY KEY, google_id text);
CREATE TABLE places(id bigint PRIMARY KEY, user_id bigint, deleted_at timestamptz);
CREATE TABLE courses(id bigint PRIMARY KEY, place_id bigint, deleted_at timestamptz);
CREATE TABLE sessions(id bigint PRIMARY KEY, place_id bigint, course_id bigint, deleted_at timestamptz);
CREATE TABLE vocabulary_cards(session_id bigint, word_key text, active boolean);
INSERT INTO users VALUES (1, 'owner-google'), (2, 'other-google');
INSERT INTO places VALUES (1, 1, NULL), (2, 2, NULL);
INSERT INTO courses VALUES (1, 1, now());
INSERT INTO sessions VALUES (1, 1, NULL, NULL), (2, 2, NULL, NULL),
                            (3, 1, NULL, now()), (4, 1, 1, NULL);
INSERT INTO vocabulary_cards VALUES
    (1, encode(sha256('honest'::bytea), 'hex'), true),
    (1, encode(sha256('find out'::bytea), 'hex'), true),
    (2, encode(sha256('other'::bytea), 'hex'), true),
    (3, encode(sha256('deleted'::bytea), 'hex'), true),
    (1, encode(sha256('inactive'::bytea), 'hex'), false),
    (4, encode(sha256('closed'::bytea), 'hex'), true);
