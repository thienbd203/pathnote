"""Deterministic matching over English dictionary forms and curated families."""

import json
import re
import unicodedata
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Literal

from lemminflect import getAllInflections, getAllLemmas

MatchMode = Literal["exact", "lemma", "word_family"]
Source = Literal["vocabulary", "generated_history", "candidate_batch"]

# Unicode White_Space, shared with the PostgreSQL expression index.
WHITESPACE = re.compile(r"[\t-\r \x85\xa0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+")
SINGLE_WORD = re.compile(r"[a-z]+")
FAMILIES = json.loads(Path(__file__).with_name("word_families.json").read_text(encoding="utf-8"))
FAMILY_BY_MEMBER = {member: family for family, members in FAMILIES.items() for member in members}
if len(FAMILY_BY_MEMBER) != sum(map(len, FAMILIES.values())):
    raise ValueError("A word belongs to multiple curated families.")

# For daily headword selection, treat these frequent forms as inflections.
# Other ambiguous forms (e.g. saw, leaves, axes) stay exact-only for lemma matching.
LEMMA_OVERRIDES = {
    "working": "work", "going": "go", "better": "good", "best": "good",
    "studied": "study",
}


def normalize(word: str) -> str:
    return unicodedata.normalize("NFC", WHITESPACE.sub(" ", word).strip(" ").lower())


@dataclass(frozen=True)
class Word:
    original: str
    exact: str
    lemma: str | None
    family: str | None


@lru_cache(maxsize=4096)
def canonical(exact: str) -> tuple[str | None, str | None]:
    if not SINGLE_WORD.fullmatch(exact):
        return exact, None
    lemmas = {lemma for forms in getAllLemmas(exact).values() for lemma in forms}
    lemma = LEMMA_OVERRIDES.get(exact)
    if lemma is None:
        lemma = next(iter(lemmas)) if len(lemmas) == 1 else exact if not lemmas else None
    family = FAMILY_BY_MEMBER.get(exact) or FAMILY_BY_MEMBER.get(lemma)
    return lemma, family


def analyze(word: str) -> Word:
    exact = normalize(word)
    return Word(word, exact, *canonical(exact))


@lru_cache(maxsize=4096)
def inflections(lemma: str) -> frozenset[str]:
    if not SINGLE_WORD.fullmatch(lemma):
        return frozenset({lemma})
    forms = {form for values in getAllInflections(lemma).values() for form in values}
    forms.update(word for word, base in LEMMA_OVERRIDES.items() if base == lemma)
    return frozenset({lemma, *forms})


def lookup_keys(words: list[Word], mode: MatchMode) -> list[str]:
    keys = set()
    for word in words:
        keys.add(word.exact)
        if mode != "exact" and word.lemma:
            keys.update(inflections(word.lemma))
        if mode == "word_family" and word.family:
            for member in FAMILIES[word.family]:
                keys.update(inflections(member))
                lemma, _ = canonical(member)
                if lemma and lemma != member:
                    keys.update(inflections(lemma))
    return sorted(keys)


def match_type(a: Word, b: Word, mode: MatchMode) -> MatchMode | None:
    if a.exact == b.exact:
        return "exact"
    if mode != "exact" and a.lemma is not None and a.lemma == b.lemma:
        return "lemma"
    if mode == "word_family" and a.family is not None and a.family == b.family:
        return "word_family"
    return None


def classify(words: list[Word], existing: list[tuple[str, Source]], mode: MatchMode) -> dict:
    pool = [(analyze(word), source) for word, source in existing]
    result = {"new_words": [], "known_words": [], "matches": []}
    seen = set()
    for word in words:
        ranked = []
        for other, source in pool:
            kind = match_type(word, other, mode)
            if kind:
                ranked.append((
                    ("exact", "lemma", "word_family").index(kind),
                    ("vocabulary", "generated_history", "candidate_batch").index(source),
                    other.original, kind, source,
                ))
        if ranked:
            _, _, matched, kind, source = min(ranked)
            result["matches"].append({
                "word": word.original, "normalized_word": word.exact,
                "lemma": word.lemma, "word_family": word.family,
                "matched_word": matched, "match_type": kind, "source": source,
            })
            if word.exact not in seen:
                result["known_words"].append(word.original)
        else:
            result["new_words"].append(word.original)
            pool.append((word, "candidate_batch"))
        seen.add(word.exact)
    return result
