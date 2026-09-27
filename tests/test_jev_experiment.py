"""Offline runner checks against loopback only; no credentials or live Zen calls."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "experiments/jev/run.py"
BIN = ROOT / "target/debug/jev-grep"
KEY = "offline-test-secret-never-evidence"


class RunnerTests(unittest.TestCase):
    def setUp(self):
        self.requests = []
        self.mode = "complete"
        self.malformed_value = None
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                owner.requests.append((self.path, dict(self.headers), body))
                answers = {name: {"noul": 0.8} for name in body.get("questions", {})}
                response = {"model": body["model"], "usage": {"input_tokens": 7, "output_tokens": 2}, "answers": answers}
                status = 200
                if owner.mode == "incomplete":
                    response["answers"] = {}
                elif owner.mode == "malformed":
                    response["answers"] = {name: {"noul": owner.malformed_value} for name in body.get("questions", {})}
                elif owner.mode == "http":
                    status = 529
                    response = {"error": KEY}
                elif owner.mode == "corpus_http" and len(owner.requests) > 3:
                    status = 529
                    response = {"error": KEY}
                data = json.dumps(response).encode()
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

            def log_message(self, *_):
                pass

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.addCleanup(self.cleanup)
        self.url = f"http://127.0.0.1:{self.server.server_port}/v1/systemone"

    def cleanup(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()

    def run_runner(self, out):
        env = {k: v for k, v in os.environ.items() if k not in ("JEV_GREP_API_KEY", "JEV_GREP_API_URL", "JEV_GREP_MODEL")}
        env.update(JEV_GREP_API_KEY=KEY, JEV_GREP_API_URL=self.url, JEV_GREP_MODEL="mock-jev", PYTHONUNBUFFERED="1")
        return subprocess.run([sys.executable, str(RUNNER), "--bin", str(BIN), "--out", str(out)], cwd=ROOT, env=env, text=True, capture_output=True, timeout=45)

    def test_text_physical_record_count_includes_unterminated_last_record(self):
        sys.path.insert(0, str(ROOT / "experiments/jev"))
        try:
            from run import text_record_count
            self.assertEqual(text_record_count("one\nlast"), 2)
            self.assertEqual(text_record_count("one\n"), 1)
            self.assertEqual(text_record_count(""), 0)
        finally:
            sys.path.remove(str(ROOT / "experiments/jev"))

    def test_missing_key_reports_blocker_without_network(self):
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / "run"
            env = {k: v for k, v in os.environ.items() if k not in ("JEV_GREP_API_KEY", "JEV_GREP_API_URL", "JEV_GREP_MODEL")}
            result = subprocess.run([sys.executable, str(RUNNER), "--bin", str(BIN), "--out", str(out)], cwd=ROOT, env=env, text=True, capture_output=True)
            self.assertEqual(result.returncode, 2)
            self.assertIn("BLOCKED", result.stderr)
            self.assertEqual(self.requests, [])
            self.assertFalse(out.exists())

    def test_live_runner_loopback_probes_corpus_and_safe_evidence(self):
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / "fresh-run"
            result = self.run_runner(out)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertNotIn("/health", [r[0] for r in self.requests])
            case_count = len((ROOT / "experiments/jev/dataset.jsonl").read_text().splitlines())
            self.assertGreaterEqual(len(self.requests), 3 + case_count)
            self.assertGreaterEqual(len(self.requests[3:]), case_count)
            for path, headers, body in self.requests:
                auth = next((v for k, v in headers.items() if k.lower() == "authorization"), None)
                self.assertEqual(auth, "Bearer " + KEY, path)
                self.assertEqual(body["model"], "mock-jev")
            probes = self.requests[:3]
            for _, headers, _ in probes:
                user_agent = next((v for k, v in headers.items() if k.lower() == "user-agent"), None)
                self.assertEqual(user_agent, "jev-grep-experiment/0.1.0")
            self.assertIsInstance(probes[0][2]["state"], str)
            self.assertIsInstance(probes[1][2]["state"], dict)
            self.assertEqual(set(probes[1][2]["questions"]), {"same_event", "useful_context", "match_next", "match_extended"})
            self.assertNotIn("Authorization", (out / "run-metadata.json").read_text())
            evidence = "".join(p.read_text(errors="ignore") for p in out.iterdir() if p.is_file())
            self.assertNotIn(KEY, evidence)
            metadata = json.loads((out / "run-metadata.json").read_text())
            self.assertEqual(metadata["status"], "completed")
            self.assertEqual(len(metadata["cases"]), 10)
            self.assertEqual(metadata["probe_count"], 3)
            self.assertTrue(any(not row["matches_expected"] for row in metadata["cases"]), "quality mismatches remain completed observations")
            expected_counts = []
            for line in (ROOT / "experiments/jev/dataset.jsonl").read_text().splitlines():
                case = json.loads(line)
                text = case["text"] + case.get("repeat_text", "") * case.get("repeat_count", 0) + case.get("suffix", "")
                expected_counts.append(1 if case["kind"] == "ast-grep" else text.count("\n") + int(bool(text) and not text.endswith("\n")))
            self.assertEqual([row["client_request_count"] for row in metadata["cases"]], expected_counts)
            self.assertEqual(len(self.requests), metadata["probe_count"] + sum(expected_counts))
            self.assertTrue(metadata["long_input_claim"].startswith("observed input"))
            self.assertTrue((out / "ast-rust-candidate.stdout.jsonl").exists())

    def test_malformed_probe_probabilities_block_before_corpus(self):
        for value in (True, 1.01, -0.01, float("nan"), float("inf")):
            with self.subTest(value=value):
                self.requests.clear()
                self.mode = "malformed"
                self.malformed_value = value
                with tempfile.TemporaryDirectory() as folder:
                    out = Path(folder) / "run"
                    result = self.run_runner(out)
                    self.assertEqual(result.returncode, 2)
                    self.assertEqual(len(self.requests), 1)
                    blocker = json.loads((out / "blocker.json").read_text())
                    self.assertEqual(blocker["status"], "blocked")
                    self.assertIn("invalid numeric", blocker["reason"])

    def test_incomplete_probe_stops_with_sanitized_blocker(self):
        self.mode = "incomplete"
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / "run"
            result = self.run_runner(out)
            self.assertEqual(result.returncode, 2)
            self.assertIn("BLOCKED", result.stderr)
            self.assertEqual(len(self.requests), 1)
            evidence = "".join(p.read_text(errors="ignore") for p in out.iterdir())
            self.assertNotIn(KEY, evidence)
            self.assertIn("incomplete", evidence)

    def test_http_error_stops_with_status_and_no_secret(self):
        self.mode = "http"
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / "run"
            result = self.run_runner(out)
            self.assertEqual(result.returncode, 2)
            self.assertIn("HTTP 529", result.stderr)
            self.assertEqual(len(self.requests), 1)
            self.assertNotIn(KEY, (out / "blocker.json").read_text())

    def test_corpus_http_error_preserves_evidence_and_marks_run_incomplete(self):
        self.mode = "corpus_http"
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / "run"
            result = self.run_runner(out)
            self.assertEqual(result.returncode, 2)
            self.assertIn("INCOMPLETE", result.stderr)
            self.assertEqual(len(self.requests), 4, "three successful probes then first corpus request error")
            metadata = json.loads((out / "run-metadata.json").read_text())
            self.assertEqual(metadata["status"], "incomplete")
            self.assertEqual(metadata["cases"][0]["exit"], 2)
            self.assertIsNone(metadata["cases"][0]["client_request_count"])
            blocker = json.loads((out / "blocker.json").read_text())
            self.assertEqual(blocker["case_id"], metadata["cases"][0]["id"])
            evidence = "".join(p.read_text(errors="ignore") for p in out.iterdir() if p.is_file())
            self.assertNotIn(KEY, evidence)
            self.assertTrue((out / metadata["cases"][0]["stderr"]).exists())

    def test_existing_output_directory_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / "existing"
            out.mkdir()
            result = self.run_runner(out)
            self.assertEqual(result.returncode, 2)
            self.assertIn("fresh", result.stderr)
            self.assertEqual(self.requests, [])


if __name__ == "__main__":
    unittest.main()
