import unittest

from dedup import analyze, classify, lookup_keys


class DedupTests(unittest.TestCase):
    def test_exact_preserves_punctuation_and_normalizes_unicode(self):
        words = [analyze(word) for word in [" Café ", "find\u00a0  out", "don't", "dont"]]
        result = classify(words, [("Cafe\u0301", "vocabulary"), ("FIND OUT", "vocabulary")], "exact")
        self.assertEqual(result["known_words"], [" Café ", "find\u00a0  out"])
        self.assertEqual(result["new_words"], ["don't", "dont"])

    def test_inflections_and_ambiguity(self):
        for word, lemma in [
            ("worked", "work"), ("working", "work"), ("works", "work"),
            ("went", "go"), ("gone", "go"), ("going", "go"), ("children", "child"),
            ("better", "good"), ("best", "good"), ("studies", "study"),
            ("studied", "study"), ("studying", "study"), ("cats", "cat"),
        ]:
            with self.subTest(word=word):
                result = classify([analyze(word)], [(lemma, "vocabulary")], "lemma")
                self.assertEqual(result["matches"][0]["match_type"], "lemma")
                self.assertIn(word, lookup_keys([analyze(lemma)], "lemma"))
                self.assertIn(lemma, lookup_keys([analyze(word)], "lemma"))
        self.assertIsNone(analyze("axes").lemma)
        result = classify([analyze("axes"), analyze("saw")], [("axis", "vocabulary"), ("see", "vocabulary")], "lemma")
        self.assertEqual(result["new_words"], ["axes", "saw"])

    def test_family_is_opt_in_and_keeps_distinct_words(self):
        candidates = list(map(analyze, ["choice", "manage", "glad", "notebook", "explain", "daily", "preference"]))
        existing = [(word, "vocabulary") for word in ["choose", "manager", "happy", "book", "plane", "day", "prefer"]]
        self.assertEqual(classify(candidates, existing, "lemma")["known_words"], [])
        self.assertEqual(
            classify(candidates, existing, "word_family")["known_words"],
            ["choice", "manage", "daily", "preference"],
        )

    def test_priority_and_in_request_duplicates(self):
        words = list(map(analyze, ["choice", "raise", "RAISE", "raising", "worker"]))
        existing = [("choose", "vocabulary"), ("choice", "generated_history"), ("work", "vocabulary")]
        result = classify(words, existing, "word_family")
        self.assertEqual(result["new_words"], ["raise"])
        self.assertEqual(result["known_words"], ["choice", "raising", "worker"])
        self.assertEqual([m["match_type"] for m in result["matches"]], ["exact", "exact", "lemma", "word_family"])
        self.assertEqual(result["matches"][0]["source"], "generated_history")
        self.assertEqual(result["matches"][1]["source"], "candidate_batch")
