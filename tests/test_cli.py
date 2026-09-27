"""Offline acceptance tests: build with cargo build, run python3 -m unittest discover -s tests."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

BIN = str(Path(__file__).resolve().parents[1] / 'target/debug/jev-grep')


class CliTests(unittest.TestCase):
    def setUp(self):
        self.requests = []
        self.response = None
        self.responses = []
        self.status = 200
        self.response_gate = None
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                owner.requests.append((self.path, dict(self.headers), body))
                gate = owner.response_gate
                if gate and len(owner.requests) == gate[0]:
                    gate[1].set()
                    gate[2].wait(timeout=10)
                response = owner.responses.pop(0) if owner.responses else owner.response
                if response is None:
                    state = body['state']
                    if isinstance(state, dict):
                        nxt = state['next_record']
                        same = 0.9 if 'join' in nxt else 0.1
                        response = {'answers': {
                            'same_event': {'noul': same}, 'useful_context': {'noul': 0.1},
                            'match_next': {'noul': 0.9 if 'PKIX' in nxt else 0.1},
                            'match_extended': {'noul': 0.9 if 'PKIX' in state['current_fragment'] or 'PKIX' in nxt else 0.1}}}
                    else:
                        response = {'answers': {'match': {'noul': 0.9 if 'PKIX' in state else 0.1}}}
                data = json.dumps(response).encode()
                self.send_response(owner.status)
                self.send_header('Content-Length', str(len(data)))
                self.end_headers()
                self.wfile.write(data)

            def log_message(self, *_):
                pass

        self.server = ThreadingHTTPServer(('localhost', 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.addCleanup(self.cleanup_server)
        self.url = f'http://localhost:{self.server.server_port}/v1/systemone'

    def cleanup_server(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()

    def test_help_version_and_default_configuration(self):
        help_result = subprocess.run([BIN, '--help'], text=True, capture_output=True, timeout=5)
        self.assertEqual(help_result.returncode, 0)
        self.assertIn('jev-grep', help_result.stdout)
        self.assertIn('https://opencode.ai/zen/v1/systemone', help_result.stdout)
        self.assertIn('jev-1.13', help_result.stdout)
        self.assertNotIn('lg', help_result.stdout)
        version = subprocess.run([BIN, '--version'], text=True, capture_output=True, timeout=5)
        self.assertEqual(version.returncode, 0)
        self.assertTrue(version.stdout.startswith('jev-grep '), version.stdout)

    def test_new_environment_defaults_and_old_environment_is_ignored(self):
        result = self.run_lg('criterion', data='PKIX\n', use_default_url=False,
                             env_overrides={
                                 'JEV_GREP_API_URL': self.url,
                                 'LG_API_URL': 'http://127.0.0.1:1/old',
                                 'LG_MODEL': 'old-model',
                                 'LG_API_KEY': 'old-secret',
                             })
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.requests[0][2]['model'], 'jev-1.13')
        self.assertNotIn('authorization', {k.lower() for k in self.requests[0][1]})

    def test_environment_and_flag_precedence_and_required_model(self):
        result = self.run_lg('criterion', data='PKIX\n', use_default_url=False,
                             env_overrides={'JEV_GREP_API_URL': self.url,
                                            'JEV_GREP_MODEL': 'env-model',
                                            'JEV_GREP_API_KEY': 'env-token'})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.requests[-1][2]['model'], 'env-model')
        headers = {key.lower(): value.lower() for key, value in self.requests[-1][1].items()}
        self.assertEqual(headers.get('authorization'), 'bearer env-token')

        self.requests.clear()
        result = self.run_lg('--api-url', self.url, '--model', 'flag-model', '--api-key', 'flag-token',
                             'criterion', data='PKIX\n', use_default_url=False,
                             env_overrides={'JEV_GREP_API_URL': 'http://127.0.0.1:1/ignored',
                                            'JEV_GREP_MODEL': 'env-model',
                                            'JEV_GREP_API_KEY': 'env-token'})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.requests[0][2]['model'], 'flag-model')
        headers = {key.lower(): value.lower() for key, value in self.requests[0][1].items()}
        self.assertEqual(headers.get('authorization'), 'bearer flag-token')

        self.requests.clear()
        for invalid_model in ('', '   '):
            result = self.run_lg('--model', invalid_model, 'criterion', data='PKIX\n')
            self.assertEqual(result.returncode, 2, result.stderr)
            self.assertIn('model must not be empty', result.stderr)
        self.assertEqual(self.requests, [])

    def test_zen_gateway_http_errors_include_status_without_fallback(self):
        for status in (401, 429, 529):
            with self.subTest(status=status):
                self.requests.clear()
                self.status = status
                result = self.run_lg('--api-key', 'test-token', 'criterion', data='PKIX\n')
                self.assertEqual(result.returncode, 2)
                self.assertIn(f'HTTP {status}', result.stderr)
                self.assertEqual(len(self.requests), 1)
                headers = {key.lower(): value.lower() for key, value in self.requests[0][1].items()}
                self.assertEqual(headers.get('authorization'), 'bearer test-token')
        self.status = 200

    def run_lg(self, *args, data='', cwd=None, env_overrides=None, use_default_url=True):
        env = {k: v for k, v in os.environ.items()
               if not k.startswith(('LG_', 'JEV_GREP_'))}
        if env_overrides:
            env.update(env_overrides)
        command = [BIN]
        if use_default_url:
            command += ['--api-url', self.url]
        command += list(args)
        return subprocess.run(command, input=data, text=True, capture_output=True,
                              timeout=10, cwd=cwd, env=env)

    def test_fragment_join_and_located_json(self):
        result = self.run_lg('--format', 'json', 'errors', data='PKIX first\njoin continuation\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        row = json.loads(result.stdout)
        self.assertEqual(row['text'], 'PKIX first\njoin continuation\n')
        self.assertEqual(row['range'], {'start_line': 1, 'end_line': 2, 'start_byte': 0, 'end_byte': 29})
        self.assertEqual(row['source'], {'kind': 'stdin', 'path': None})
        self.assertEqual(len(self.requests), 2)
        self.assertEqual(set(self.requests[1][2]['questions']), {'same_event','useful_context','match_next','match_extended'})

    def test_non_join_uses_saved_score_and_streams_before_eof(self):
        result = self.run_lg('errors', data='PKIX first\nother\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('(standard input):1-1', result.stdout)
        self.assertIn('PKIX first', result.stdout)

    def test_useful_context_at_equality_and_extended_relevance_change(self):
        self.responses = [
            {'answers': {'match': {'noul': 0.9}}},
            {'answers': {'same_event': {'noul': 0.2}, 'useful_context': {'noul': 0.8},
                         'match_next': {'noul': 0.1}, 'match_extended': {'noul': 0.1}}},
        ]
        result = self.run_lg('--join-threshold', '0.8', 'errors', data='PKIX original\nresolves it\n')
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(result.stdout, '')
        self.assertEqual(len(self.requests), 2)

    def test_declined_extension_does_not_replace_pending_score(self):
        self.responses = [
            {'answers': {'match': {'noul': 0.9}}},
            {'answers': {'same_event': {'noul': 0.1}, 'useful_context': {'noul': 0.1},
                         'match_next': {'noul': 0.1}, 'match_extended': {'noul': 0.99}}},
            {'answers': {'match': {'noul': 0.1}}},
        ]
        result = self.run_lg('--format', 'json', 'errors', data='PKIX original\nother\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        row = json.loads(result.stdout)
        self.assertEqual(row['text'], 'PKIX original\n')
        self.assertEqual(row['probability'], 0.9)

    def test_human_and_json_select_the_same_fragment(self):
        def responses():
            return [
                {'answers': {'match': {'noul': 0.9}}},
                {'answers': {'same_event': {'noul': 0.9}, 'useful_context': {'noul': 0.1},
                             'match_next': {'noul': 0.1}, 'match_extended': {'noul': 0.9}}},
                {'answers': {'same_event': {'noul': 0.1}, 'useful_context': {'noul': 0.1},
                             'match_next': {'noul': 0.1}, 'match_extended': {'noul': 0.1}}},
            ]
        data = 'PKIX failure\njoin detail\nother\n'
        self.responses = responses()
        human = self.run_lg('errors', data=data)
        self.responses = responses()
        machine = self.run_lg('--format', 'json', 'errors', data=data)
        self.assertEqual(human.returncode, machine.returncode)
        row = json.loads(machine.stdout)
        self.assertIn('PKIX failure\njoin detail\n', human.stdout)
        self.assertEqual(row['text'], 'PKIX failure\njoin detail\n')
        self.assertEqual(row['range']['start_line'], 1)
        self.assertEqual(row['range']['end_line'], 2)

    def test_decoded_jsonl(self):
        result = self.run_lg('--input-format', 'log-jsonl', '--format', 'json', 'errors', data='{"text":"PKIX\\ntrace"}\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        row = json.loads(result.stdout)
        self.assertEqual(row['text'], 'PKIX\ntrace')
        self.assertEqual(row['text_basis'], 'decoded-records')
        self.assertEqual(row['range']['end_line'], 1)

    def test_raw_crlf_unicode_coordinates_and_text(self):
        result = self.run_lg('--format', 'json', 'errors', data='PKIX ошибка\r\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        row = json.loads(result.stdout)
        self.assertEqual(row['text'], 'PKIX ошибка\r\n')
        self.assertEqual(row['range'], {'start_line': 1, 'end_line': 1, 'start_byte': 0,
                                        'end_byte': len('PKIX ошибка\r\n'.encode())})

    def test_fragment_split_and_cli_conflicts(self):
        result = self.run_lg('--max-fragment-bytes', '6', '--format', 'json', 'errors', data='PKIX!\nPKIX\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        rows = [json.loads(x) for x in result.stdout.splitlines()]
        self.assertTrue(rows[0]['split_after'])
        self.assertTrue(rows[1]['split_before'])
        self.assertEqual([r['text'] for r in rows], ['PKIX!\n', 'PKIX\n'])
        result = self.run_lg('--json', '--format', 'human', 'errors', data='')
        self.assertEqual(result.returncode, 2)

    def test_count_list_quiet_invert_and_validation(self):
        result = self.run_lg('-c', 'errors', data='PKIX\nother\n')
        self.assertEqual((result.returncode, result.stdout), (0, '(standard input):1\n'))
        result = self.run_lg('-l', 'errors', data='PKIX\n')
        self.assertEqual((result.returncode, result.stdout), (0, '(standard input)\n'))
        result = self.run_lg('-q', 'errors', data='PKIX\n')
        self.assertEqual((result.returncode, result.stdout), (0, ''))
        result = self.run_lg('-v', 'errors', data='PKIX\nother\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('other', result.stdout)
        self.requests.clear()
        result = self.run_lg('-m', '1', 'errors', data='PKIX\nother\nPKIX later\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(self.requests), 2, 'stop at first completed selected fragment')
        result = self.run_lg('--no-filename', 'errors', data='')
        self.assertEqual(result.returncode, 2)
        result = self.run_lg('--join-threshold', 'NaN', 'errors', data='')
        self.assertEqual(result.returncode, 2)

    def test_multiple_file_listing_and_walk_filters(self):
        with tempfile.TemporaryDirectory(dir=Path.cwd()) as folder:
            root = Path(folder)
            (root / 'one.log').write_text('PKIX one\n')
            (root / 'two.log').write_text('PKIX two\n')
            (root / 'skip.txt').write_text('PKIX ignored by glob\n')
            (root / '.hidden.log').write_text('PKIX hidden\n')
            (root / '.ignore').write_text('ignored.log\n')
            (root / 'ignored.log').write_text('PKIX ignored by rule\n')

            result = self.run_lg('-l', '--no-ignore', 'errors', str(root / 'one.log'), str(root / 'two.log'))
            self.assertEqual(result.returncode, 0, (result.stdout, result.stderr, self.requests))
            self.assertEqual(set(result.stdout.splitlines()), {str(root / 'one.log'), str(root / 'two.log')})

            result = self.run_lg('-l', '--hidden', '--no-ignore', '-g', '*.log', 'errors', str(root))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(set(result.stdout.splitlines()), {
                str(root / 'one.log'), str(root / 'two.log'), str(root / '.hidden.log'),
                str(root / 'ignored.log')})
    def test_empty_input_and_broken_or_invalid_records(self):
        self.assertEqual(self.run_lg('errors', data='').returncode, 1)
        result = self.run_lg('--input-format', 'log-jsonl', 'errors', data='[]\n')
        self.assertEqual(result.returncode, 2)

    def test_broken_pipe_exits_cleanly(self):
        entered, release = threading.Event(), threading.Event()
        self.response_gate = (3, entered, release)
        process = subprocess.Popen([BIN, '--api-url', self.url, 'errors'],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE)
        try:
            process.stdin.write(b'PKIX first\nPKIX second\nother\n')
            process.stdin.close()
            first_output = process.stdout.readline() + process.stdout.readline()
            self.assertIn(b'PKIX first', first_output)
            # Hold the third inference response until the first result has
            # been read and the downstream end closed, forcing the next write.
            self.assertTrue(entered.wait(timeout=5), 'second fragment response was not reached')
            process.stdout.close()
            release.set()
            process.wait(timeout=10)
            stderr = process.stderr.read()
            self.assertEqual(process.returncode, 0, stderr.decode(errors='replace'))
            self.assertEqual(stderr, b'')
        finally:
            release.set()
            if process.poll() is None:
                process.kill()
                process.wait()
            process.stderr.close()

    def test_ast_grep_fixture_preserves_upstream_candidate(self):
        data = (Path(__file__).parent / 'fixtures/ast-grep-native.jsonl').read_text()
        result = self.run_lg('--input-format', 'ast-grep', '--format', 'json', 'errors', data=data)
        self.assertEqual(result.returncode, 0, result.stderr)
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0]['text'], 'PKIX ошибка\r\n')
        self.assertEqual(rows[0]['source'], {'kind': 'file', 'path': 'src/main.rs'})
        self.assertEqual(rows[0]['range'], {'start_line': 3, 'end_line': 3, 'start_byte': 10, 'end_byte': 29})
        self.assertEqual(rows[0]['language'], 'Rust')
        self.assertNotIn('lines', rows[0]['text'])
        self.assertEqual(rows[1]['range']['start_byte'], 30)
        self.assertEqual([r[2]['state'] for r in self.requests], ['PKIX ошибка\r\n', 'PKIX node'])

    def test_ast_grep_stdin_and_streams_before_jsonl_eof(self):
        candidate = {'text':'PKIX ready', 'file':'STDIN', 'range':{'byteOffset':{'start':0,'end':10}, 'start':{'line':0,'column':0}, 'end':{'line':0,'column':10}}, 'lines':'wrong'}
        data = (json.dumps(candidate) + '\n').encode()
        process = subprocess.Popen([BIN, '--api-url', self.url, '--input-format', 'ast-grep', '--format', 'json', 'errors'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            process.stdin.write(data); process.stdin.flush()
            import select
            ready, _, _ = select.select([process.stdout], [], [], 5)
            self.assertTrue(ready, 'complete candidate should be emitted before EOF')
            row = json.loads(process.stdout.readline())
            self.assertEqual(row['source'], {'kind':'stdin','path':None})
            self.assertEqual(row['text'], 'PKIX ready')
            process.stdin.close()
            self.assertEqual(process.wait(timeout=5), 0)
        finally:
            if process.poll() is None: process.kill(); process.wait()
            for stream in [process.stdin, process.stdout, process.stderr]: stream.close()

    def test_ast_grep_rejects_bad_schema_ranges_and_oversized_candidates(self):
        malformed = ['[]', '{"text":"x"}',
            json.dumps({'text':'λ','file':'x','range':{'byteOffset':{'start':0,'end':1},'start':{'line':0,'column':0},'end':{'line':0,'column':1}}}),
            json.dumps({'text':'x','file':'x','range':{'byteOffset':{'start':2,'end':1},'start':{'line':0,'column':0},'end':{'line':0,'column':1}}})]
        for row in malformed:
            result = self.run_lg('--input-format','ast-grep','errors',data=row+'\n')
            self.assertEqual(result.returncode, 2, row)
            self.assertEqual(self.requests, [])
        candidate = {'text':'PKIX long','file':'x','range':{'byteOffset':{'start':0,'end':9},'start':{'line':0,'column':0},'end':{'line':0,'column':9}}}
        result = self.run_lg('--input-format','ast-grep','--max-fragment-bytes','4','errors',data=json.dumps(candidate)+'\n')
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertEqual(self.requests, [])

    def test_ast_grep_rejects_reversed_or_inconsistent_columns_before_http(self):
        def candidate(text, start_column, end_column, start_line=0, end_line=0):
            return {'text': text, 'file': 'x', 'range': {
                'byteOffset': {'start': 10, 'end': 10 + len(text.encode())},
                'start': {'line': start_line, 'column': start_column},
                'end': {'line': end_line, 'column': end_column}}}
        # ast-grep columns count Unicode characters, while byteOffset is UTF-8.
        for row in [candidate('λx', 4, 3), candidate('λx', 4, 7),
                    candidate('first\nxy', 4, 3, 0, 1),
                    candidate('first\nxy\n', 4, 2, 0, 2)]:
            result = self.run_lg('--input-format', 'ast-grep', 'errors', data=json.dumps(row) + '\n')
            self.assertEqual(result.returncode, 2, result.stderr)
            self.assertIn('JSONL candidate', result.stderr)
            self.assertEqual(self.requests, [], 'coordinate validation must precede inference')

    def test_ast_grep_upstream_source_accounting_and_limit_validation(self):
        def candidate(file, text):
            return {'text': text, 'file': file, 'range': {
                'byteOffset': {'start': 0, 'end': len(text.encode())},
                'start': {'line': 0, 'column': 0},
                'end': {'line': 0, 'column': len(text)}}}

        data = ''.join(json.dumps(row) + '\n' for row in [
            candidate('A', 'PKIX first'), candidate('B', 'other'), candidate('A', 'PKIX second')])
        result = self.run_lg('--input-format', 'ast-grep', '--format', 'json', '-m', '1',
                             'errors', data=data)
        self.assertEqual(result.returncode, 0, result.stderr)
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual([row['source']['path'] for row in rows], ['A'])
        self.assertEqual([body['state'] for _, _, body in self.requests], ['PKIX first', 'other'])

        # A is capped, but its next physical record still has to be validated.
        self.requests.clear()
        malformed = candidate('A', 'PKIX second')
        malformed['range']['byteOffset']['end'] -= 1
        result = self.run_lg('--input-format', 'ast-grep', '-m', '1', 'errors',
                             data=json.dumps(candidate('A', 'PKIX first')) + '\n'
                                  + json.dumps(malformed) + '\n')
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertEqual(len(self.requests), 1, 'capped candidate is validated without inference')

    def test_ast_grep_count_first_seen_sources_and_empty_stream(self):
        def candidate(file, text):
            return {'text': text, 'file': file, 'range': {
                'byteOffset': {'start': 0, 'end': len(text.encode())},
                'start': {'line': 0, 'column': 0},
                'end': {'line': 0, 'column': len(text)}}}

        data = ''.join(json.dumps(row) + '\n' for row in [
            candidate('A', 'PKIX'), candidate('B', 'other'), candidate('A', 'other')])
        result = self.run_lg('--input-format', 'ast-grep', '-c', 'errors', data=data)
        self.assertEqual((result.returncode, result.stdout), (0, 'A:1\nB:0\n'))
        self.assertEqual(len(self.requests), 3)
        result = self.run_lg('--input-format', 'ast-grep', '-c', 'errors', data='')
        self.assertEqual((result.returncode, result.stdout), (1, ''))

    def test_ast_grep_files_with_matches_is_once_per_upstream_source(self):
        def candidate(text):
            return {'text': text, 'file': 'same.rs', 'range': {
                'byteOffset': {'start': 0, 'end': len(text)},
                'start': {'line': 0, 'column': 0}, 'end': {'line': 0, 'column': len(text)}}}
        data = json.dumps(candidate('PKIX one')) + '\n' + json.dumps(candidate('PKIX two')) + '\n'
        result = self.run_lg('--input-format', 'ast-grep', '-l', 'errors', data=data)
        self.assertEqual((result.returncode, result.stdout), (0, 'same.rs\n'))
        self.assertEqual(len(self.requests), 2)

    def test_ast_grep_source_accounting_is_shared_across_input_files(self):
        def candidate(file, text):
            return {'text': text, 'file': file, 'range': {
                'byteOffset': {'start': 0, 'end': len(text)},
                'start': {'line': 0, 'column': 0}, 'end': {'line': 0, 'column': len(text)}}}

        with tempfile.TemporaryDirectory() as folder:
            first = Path(folder, 'first.jsonl')
            second = Path(folder, 'second.jsonl')
            first.write_text(json.dumps(candidate('src/A.rs', 'PKIX first')) + '\n')
            second.write_text('\n'.join(json.dumps(row) for row in [
                candidate('src/A.rs', 'PKIX second'), candidate('src/B.rs', 'other')]) + '\n')

            result = self.run_lg('--input-format', 'ast-grep', '-m', '1', '-c', 'errors',
                                 str(first), str(second))
            self.assertEqual((result.returncode, result.stdout), (0, 'src/A.rs:1\nsrc/B.rs:0\n'))
            self.assertEqual([body['state'] for _, _, body in self.requests],
                             ['PKIX first', 'other'], 'second A is capped but B is still evaluated')

            self.requests.clear()
            result = self.run_lg('--input-format', 'ast-grep', '-l', 'errors',
                                 str(first), str(second))
            self.assertEqual((result.returncode, result.stdout), (0, 'src/A.rs\n'))
            self.assertEqual(len(self.requests), 3)

    def test_stream_boundary_emits_before_eof(self):
        process = subprocess.Popen([BIN, '--api-url', self.url, 'errors'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            process.stdin.write(b'PKIX\n')
            process.stdin.flush()
            import select
            ready, _, _ = select.select([process.stdout], [], [], 0.3)
            self.assertFalse(ready, 'an idle pending fragment must not flush')
            process.stdin.write(b'other\n')
            process.stdin.flush()
            ready, _, _ = select.select([process.stdout], [], [], 5)
            self.assertTrue(ready)
            self.assertIn(b'PKIX', process.stdout.readline() + process.stdout.readline())
            process.stdin.close()
            self.assertEqual(process.wait(timeout=5), 0)
        finally:
            if process.poll() is None: process.kill(); process.wait()
            for stream in [process.stdin, process.stdout, process.stderr]:
                stream.close()


if __name__ == '__main__':
    unittest.main()
