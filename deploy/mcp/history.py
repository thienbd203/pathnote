"""Persistence for proposal history, separate from notes and flashcards."""

from datetime import date

from psycopg import AsyncConnection

from dedup import MatchMode, Word, classify, lookup_keys


class UnlinkedAccount(Exception):
    pass


async def check(connection: AsyncConnection, subject: str, words: list[Word], mode: MatchMode) -> dict:
    cursor = await connection.execute(
        "SELECT word, source FROM pathnote_mcp.find_matches(%s, %s)",
        (subject, lookup_keys(words, mode)),
    )
    rows = await cursor.fetchall()
    if not rows:
        raise UnlinkedAccount
    return classify(words, [(word, source) for word, source in rows if word is not None], mode)


async def find_batch(connection: AsyncConnection, subject: str, day: date, key: str | None = None):
    cursor = await connection.execute(
        "SELECT * FROM pathnote_mcp.find_batch(%s, %s, %s)",
        (subject, day, key),
    )
    return await cursor.fetchone()


def receipt(batch_id: int, words: list[str], replay: bool) -> dict:
    return {
        "success": True, "batch_id": str(batch_id), "recorded_words": words,
        "recorded_count": len(words), "duplicates": [], "idempotent_replay": replay,
    }


def conflict(code: str, duplicates=()) -> dict:
    return {
        "success": False, "error_code": code, "recorded_words": [],
        "recorded_count": 0, "duplicates": list(duplicates),
    }


async def record(
    connection: AsyncConnection, subject: str, words: list[Word],
    day: date, key: str, mode: MatchMode,
) -> dict:
    # The connection context commits before the tool sends its response. Hold this lock across
    # lookup, conflict detection and insert so concurrent modes see prior writes.
    cursor = await connection.execute("SELECT pathnote_mcp.lock_account(%s)", (subject,))
    if (await cursor.fetchone())[0] is None:
        raise UnlinkedAccount
    entries = [word.original for word in words]
    prior = await find_batch(connection, subject, day, key)
    if prior:
        batch_id, saved_day, saved_key, saved_mode, saved_words = prior
        if key == saved_key:
            if (day, mode, entries) != (saved_day, saved_mode, saved_words):
                return conflict("IDEMPOTENCY_CONFLICT")
            return receipt(batch_id, saved_words, True)
        return {
            **conflict("DAILY_BATCH_EXISTS"),
            "daily_batch": receipt(batch_id, saved_words, True),
        }
    matches = (await check(connection, subject, words, mode))["matches"]
    if matches:
        return conflict("DUPLICATE_VOCABULARY", matches)
    cursor = await connection.execute(
        "SELECT pathnote_mcp.store_batch(%s, %s, %s, %s, %s)",
        (subject, day, key, mode, entries),
    )
    batch_id = (await cursor.fetchone())[0]
    return receipt(batch_id, entries, False)
