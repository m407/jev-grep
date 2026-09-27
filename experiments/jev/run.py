#!/usr/bin/env python3
"""Run the synthetic Jev corpus against Zen; never writes credentials to evidence."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import time
from urllib.parse import urlsplit
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
DATASET = Path(__file__).with_name("dataset.jsonl")
DEFAULT_URL = "https://opencode.ai/zen/v1/systemone"
DEFAULT_MODEL = "jev-1.13"
QUESTIONS = {
    "match": "Evaluate whether this synthetic text matches the user criterion.",
    "same_event": "Does next_record continue the same concrete event as current_fragment? Shared topic alone is insufficient.",
    "useful_context": "Does next_record materially explain current_fragment for the user criterion?",
    "match_next": "Evaluate only next_record against the user criterion.",
    "match_extended": "Evaluate current_fragment followed by next_record against the user criterion.",
}


class RunError(Exception):
    pass


def validate_answers(payload, question_ids):
    if not isinstance(payload, dict) or not isinstance(payload.get("answers"), dict):
        raise RunError("probe returned an incomplete answers object")
    answers = payload["answers"]
    for question_id in question_ids:
        answer = answers.get(question_id)
        value = answer.get("noul") if isinstance(answer, dict) else None
        if (isinstance(value, bool) or not isinstance(value, (int, float))
                or not 0 <= value <= 1 or not math.isfinite(value)):
            raise RunError("probe returned incomplete or invalid numeric noul answers")


def request(url, model, key, state, questions):
    body = json.dumps({"model": model, "state": state, "questions": questions}).encode()
    headers = {
        "Content-Type": "application/json",
        "Authorization": "Bearer " + key,
        "User-Agent": "jev-grep-experiment/0.1.0",
    }
    req = urllib.request.Request(url, data=body, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=60) as response:
            payload = json.loads(response.read(65537))
            validate_answers(payload, questions)
            return {"http_status": response.status, "model": payload.get("model"), "usage": payload.get("usage")}
    except urllib.error.HTTPError as exc:
        raise RunError("probe returned HTTP " + str(exc.code)) from None
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError, OSError) as exc:
        raise RunError("probe failed: " + type(exc).__name__) from None


def load_cases():
    rows = [json.loads(line) for line in DATASET.read_text(encoding="utf-8").splitlines() if line]
    for row in rows:
        if "repeat_count" in row:
            row["text"] += row.pop("repeat_text") * row.pop("repeat_count") + row.pop("suffix")
        if row["kind"] == "ast-grep":
            row["range"]["byteOffset"]["end"] = len(row["text"].encode())
            row["range"]["end"]["column"] = len(row["text"].splitlines()[-1])
    return rows


def text_record_count(text):
    """Count LF-delimited physical records, including a final unterminated one."""
    if not text:
        return 0
    return text.count("\n") + (0 if text.endswith("\n") else 1)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path, help="must not already exist")
    args = parser.parse_args(argv)
    endpoint = os.environ.get("JEV_GREP_API_URL", DEFAULT_URL)
    model = os.environ.get("JEV_GREP_MODEL", DEFAULT_MODEL)
    key = os.environ.get("JEV_GREP_API_KEY")
    if not key:
        print("BLOCKED: JEV_GREP_API_KEY is not set; use fnox exec. No inference performed.", file=sys.stderr)
        return 2
    parsed_endpoint = urlsplit(endpoint)
    if (parsed_endpoint.scheme not in ("http", "https") or not parsed_endpoint.hostname
            or parsed_endpoint.username is not None or parsed_endpoint.password is not None
            or parsed_endpoint.query or parsed_endpoint.fragment
            or not parsed_endpoint.path.endswith("/v1/systemone") or not model.strip()):
        print("BLOCKED: endpoint must be an http(s) /v1/systemone URL without embedded credentials/query, and model must be non-empty.", file=sys.stderr)
        return 2
    if not args.bin.is_file() or not os.access(args.bin, os.X_OK):
        print("BLOCKED: executable jev-grep binary not found; build it separately.", file=sys.stderr)
        return 2
    if args.out.exists():
        print("BLOCKED: --out must name a fresh, nonexistent directory.", file=sys.stderr)
        return 2
    args.out.mkdir(parents=True)
    started_utc = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    try:
        single = request(endpoint, model, key, "synthetic authorized metadata probe", {"match": {"type": "noul", "instructions": QUESTIONS["match"]}})
        grouped = request(endpoint, model, key, {"current_fragment": "synthetic current", "next_record": "synthetic next"}, {q: {"type": "noul", "instructions": QUESTIONS[q]} for q in ("same_event", "useful_context", "match_next", "match_extended")})
        long_case = next(c for c in load_cases() if c["id"] == "long-input")
        long_probe = request(endpoint, model, key, long_case["text"], {"match": {"type": "noul", "instructions": long_case["query"]}})
    except RunError as exc:
        (args.out / "blocker.json").write_text(json.dumps({"status": "blocked", "reason": str(exc)}, indent=2) + "\n")
        print("BLOCKED: " + str(exc) + "; see blocker.json (no credentials recorded).", file=sys.stderr)
        return 2
    cases = load_cases()
    rows = []
    incomplete = None
    for case in cases:
        inp = json.dumps({k: case[k] for k in ("text", "file", "range")}, ensure_ascii=False) + "\n" if case["kind"] == "ast-grep" else case["text"]
        options = [str(args.bin), "--api-url", endpoint, "--model", model, "--format", "json"]
        if case["kind"] == "ast-grep":
            options += ["--input-format", "ast-grep"]
        limit = case.get("max_fragment_bytes", 16384)
        options += ["--threshold", "0.5", "--join-threshold", "0.5", "--max-fragment-bytes", str(limit), "--max-line-bytes", "65536", case["query"], "-"]
        env = {k: v for k, v in os.environ.items() if k != "JEV_GREP_API_KEY"}
        env["JEV_GREP_API_KEY"] = key
        started = time.monotonic()
        result = subprocess.run(options, input=inp, text=True, capture_output=True, env=env)
        elapsed = round((time.monotonic() - started) * 1000, 3)
        raw_name = case["id"] + ".stdout.jsonl"
        err_name = case["id"] + ".stderr"
        (args.out / raw_name).write_text(result.stdout, encoding="utf-8")
        # The client is designed not to echo credentials; redact defensively.
        safe_stderr = result.stderr.replace(key, "[REDACTED]")
        (args.out / err_name).write_text(safe_stderr, encoding="utf-8")
        selected = []
        for line in result.stdout.splitlines():
            try:
                row = json.loads(line)
                selected.append({"start_line": row["range"]["start_line"], "end_line": row["range"]["end_line"]})
            except (ValueError, KeyError, TypeError):
                continue
        client_request_count = None
        if result.returncode in (0, 1):
            client_request_count = 1 if case["kind"] == "ast-grep" else text_record_count(case["text"])
        rows.append({"id": case["id"], "exit": result.returncode, "elapsed_ms": elapsed, "expected": case["expected"], "selected": selected, "matches_expected": selected == case["expected"] and result.returncode in (0, 1), "stdout": raw_name, "stderr": err_name, "query": case["query"], "threshold": 0.5, "join_threshold": 0.5, "max_fragment_bytes": limit, "max_line_bytes": 65536, "client_request_count": client_request_count, "client_request_count_basis": "flow-derived from successful client control flow: one request per text physical record or AST candidate; not server-instrumented" if client_request_count is not None else "unknown because client invocation failed"})
        if result.returncode not in (0, 1):
            incomplete = {"case_id": case["id"], "reason": "jev-grep invocation failed with exit status " + str(result.returncode)}
            break
    status = "incomplete" if incomplete else "completed"
    metadata = {"status": status, "started_utc": started_utc, "completed_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "endpoint": endpoint, "requested_model": model, "client_version": subprocess.run([str(args.bin), "--version"], capture_output=True, text=True).stdout.strip(), "dataset_sha256": hashlib.sha256(DATASET.read_bytes()).hexdigest(), "probe_count": 3, "probes": {"single": single, "grouped": grouped, "long_input": long_probe}, "cases": rows, "credentials_recorded": False, "long_input_claim": "observed input/usage only; not a context-limit test"}
    (args.out / "run-metadata.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if incomplete:
        blocker = {"status": "incomplete", **incomplete}
        (args.out / "blocker.json").write_text(json.dumps(blocker, indent=2) + "\n", encoding="utf-8")
        print("INCOMPLETE: " + incomplete["reason"] + " during " + incomplete["case_id"] + "; evidence preserved in output directory.", file=sys.stderr)
        return 2
    print("Completed live experiment; review per-case output and quality mismatches in run-metadata.json.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
