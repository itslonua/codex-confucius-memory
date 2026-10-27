import unittest

from bench import answer_matches, extract_usage, parse_answer, parse_tasks, summarize


class ScoringTests(unittest.TestCase):
    def test_task_parser_accepts_valid_values_without_changing_them(self):
        task = {"id": "one", "class": "familiar", "query": "one", "note_path": "notes/one.md",
                "current_context": "current", "question": "question", "answer": "answer", "note": "note",
                "expected_terms": ["answer"]}
        self.assertEqual(parse_tasks([task]), [task])

    def test_task_parser_rejects_invalid_shape_duplicate_ids_and_path_traversal(self):
        base = {"id": "one", "class": "familiar", "query": "one", "note_path": "notes/one.md",
                "current_context": "current", "question": "question", "answer": "answer", "note": "note",
                "expected_terms": ["answer"]}
        with self.assertRaises(ValueError):
            parse_tasks({"tasks": [base]})
        with self.assertRaises(ValueError):
            parse_tasks([base, base])
        with self.assertRaises(ValueError):
            parse_tasks([{**base, "note_path": "../outside.md"}])
        with self.assertRaises(ValueError):
            parse_tasks([{**base, "expected_terms": "answer"}])

    def test_parses_json_from_plain_or_fenced_response(self):
        self.assertEqual(parse_answer('{"answer":"blue"}'), {"answer": "blue"})
        self.assertEqual(parse_answer('```json\n{"answer":"blue"}\n```'), {"answer": "blue"})

    def test_rejects_malformed_or_non_object_response(self):
        self.assertIsNone(parse_answer('not json'))
        self.assertIsNone(parse_answer('["blue"]'))

    def test_answer_matches_all_required_terms_case_insensitively(self):
        self.assertTrue(answer_matches({"answer": "Use stable keys"}, ["stable", "keys"]))
        self.assertFalse(answer_matches({"answer": "Use generated keys"}, ["stable", "keys"]))
        self.assertFalse(answer_matches({"answer": "12"}, ["2"]))

    def test_usage_is_read_only_and_absent_when_events_have_no_usage(self):
        self.assertEqual(extract_usage('{"type":"event","usage":{"input_tokens":10,"output_tokens":2}}'),
                         {"input_tokens": 10, "output_tokens": 2})
        self.assertIsNone(extract_usage('{"type":"turn.started"}'))

    def test_summary_uses_only_valid_trials_and_keeps_arm_separation(self):
        rows = [
            {"arm": "memory", "correct": True, "answer": {}, "exit_code": 0, "elapsed_seconds": 2.0},
            {"arm": "memory", "correct": False, "answer": {}, "exit_code": 0, "elapsed_seconds": 4.0},
            {"arm": "no_memory", "correct": False, "answer": {}, "exit_code": 0, "elapsed_seconds": 3.0},
            {"arm": "memory", "correct": False, "answer": None, "exit_code": 1, "elapsed_seconds": None},
        ]
        result = summarize(rows)
        self.assertEqual(result["memory"]["trials"], 3)
        self.assertEqual(result["memory"]["valid_answers"], 2)
        self.assertEqual(result["memory"]["failed_trials"], 1)
        self.assertEqual(result["memory"]["accuracy"], 1 / 3)
        self.assertEqual(result["no_memory"]["accuracy"], 0.0)
        self.assertEqual(result["no_memory"]["mean_elapsed_seconds"], 3.0)


if __name__ == "__main__":
    unittest.main()
