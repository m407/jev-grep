use anyhow::{Context, Result, ensure};
use rand::Rng;
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::io::Read;
use std::time::{Duration, Instant, SystemTime};

use crate::Args;
use crate::rate_limit::{Clock, RateLimiter, SystemClock, WINDOW, estimate_request_tokens};

pub struct Api {
    client: Client,
    limiter: RefCell<RateLimiter>,
    clock: Box<dyn Clock>,
}

const MAX_RESPONSE_BYTES: usize = 65536;
const MAX_ATTEMPTS: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupProbabilities {
    pub same_event: f64,
    pub useful_context: f64,
    pub match_next: f64,
    pub match_extended: f64,
}

impl Api {
    pub fn new(args: &Args) -> Result<Self> {
        let url = reqwest::Url::parse(&args.api_url).context("invalid API URL")?;
        ensure!(
            matches!(url.scheme(), "http" | "https"),
            "API URL must use http or https"
        );
        ensure!(
            url.username().is_empty() && url.password().is_none(),
            "use JEV_GREP_API_KEY instead of URL credentials"
        );
        Ok(Self {
            client: Client::builder()
                .retry(reqwest::retry::never())
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            limiter: RefCell::new(RateLimiter::new(args.tpm)),
            clock: Box::new(SystemClock),
        })
    }

    pub fn probability(&self, args: &Args, text: &str) -> Result<f64> {
        let body = request_body(
            args,
            json!(text),
            json!({
                "match": question(&args.query)
            }),
        );
        let answers = self.send(args, &body)?;
        probability(&answers, "match")
    }

    pub fn group_probabilities(
        &self,
        args: &Args,
        current_fragment: &str,
        next_record: &str,
    ) -> Result<GroupProbabilities> {
        let criterion = &args.query;
        let body = request_body(
            args,
            json!({
                "current_fragment": current_fragment,
                "next_record": next_record,
            }),
            json!({
                "same_event": question("Does next_record continue the same concrete event as current_fragment? Shared topic alone is insufficient."),
                "useful_context": question(&format!("Does next_record materially explain, qualify, resolve, or contradict current_fragment with respect to the user criterion: {criterion}? Merely matching that criterion independently is insufficient.")),
                "match_next": question(&format!("Evaluate only next_record, ignoring current_fragment. Criterion: {criterion}")),
                "match_extended": question(&format!("Evaluate current_fragment followed by next_record as one fragment. Criterion: {criterion}")),
            }),
        );
        let answers = self.send(args, &body)?;
        // Validate all answers before returning any of them, so callers cannot
        // mutate pending fragment state using a partial model response.
        Ok(GroupProbabilities {
            same_event: probability(&answers, "same_event")?,
            useful_context: probability(&answers, "useful_context")?,
            match_next: probability(&answers, "match_next")?,
            match_extended: probability(&answers, "match_extended")?,
        })
    }

    fn send(&self, args: &Args, body: &Value) -> Result<Value> {
        let body = serde_json::to_vec(body)?;
        let cost = estimate_request_tokens(&body, args.tpm_safety_factor)?;
        let budget = Duration::from_secs(args.timeout);
        let mut started = self.clock.now();
        let mut last_failure = String::from("deadline exhausted");
        for attempt in 1..=MAX_ATTEMPTS {
            let remaining = budget.saturating_sub(self.clock.now().duration_since(started));
            ensure!(
                !remaining.is_zero(),
                "API retry budget exhausted after attempt {}/{}: {last_failure}",
                attempt - 1,
                MAX_ATTEMPTS
            );
            let mut request = self
                .client
                .post(&args.api_url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.clone());
            if let Some(key) = &args.api_key {
                request = request.bearer_auth(key);
            }
            // Build/authenticate before reserving; every actual send, including a
            // retry, reserves the full body. Waiting does not spend API time.
            let waiting = self.clock.now();
            loop {
                let delay = self.limiter.borrow_mut().reserve(cost, self.clock.now())?;
                let Some(delay) = delay else { break };
                self.report_wait("local TPM budget", cost, delay);
                self.clock.sleep(delay);
            }
            started += self.clock.now().duration_since(waiting);
            let remaining = budget.saturating_sub(self.clock.now().duration_since(started));
            ensure!(
                !remaining.is_zero(),
                "API retry budget exhausted before attempt {attempt}: {last_failure}"
            );
            match request.timeout(remaining).send() {
                Ok(response) => {
                    let status = response.status();
                    if !status.is_success() {
                        let code = status.as_u16();
                        let retry_after = (code != 429)
                            .then(|| {
                                response
                                    .headers()
                                    .get(reqwest::header::RETRY_AFTER)
                                    .and_then(|h| h.to_str().ok())
                                    .map(str::to_owned)
                            })
                            .flatten();
                        drop(response);
                        if !retryable_status(code) {
                            anyhow::bail!("API returned HTTP {code}");
                        }
                        last_failure = format!("HTTP {code}");
                        if attempt == MAX_ATTEMPTS {
                            break;
                        }
                        if code == 429 {
                            self.report_wait("HTTP 429 cooldown", cost, WINDOW);
                            let waiting = self.clock.now();
                            self.clock.sleep(WINDOW);
                            started += self.clock.now().duration_since(waiting);
                            continue;
                        }
                        let delay = retry_delay(attempt, retry_after.as_deref(), SystemTime::now());
                        self.report_wait(&last_failure, cost, delay);
                        sleep_within_budget(
                            self.clock.as_ref(),
                            delay,
                            started,
                            budget,
                            attempt,
                            &last_failure,
                        )?;
                        continue;
                    }
                    let mut bytes = Vec::new();
                    match response
                        .take((MAX_RESPONSE_BYTES + 1) as u64)
                        .read_to_end(&mut bytes)
                    {
                        Ok(_) if bytes.len() > MAX_RESPONSE_BYTES => {
                            anyhow::bail!("API response exceeds {MAX_RESPONSE_BYTES} bytes")
                        }
                        Ok(_) => {
                            let response: Value = serde_json::from_slice(&bytes)
                                .context("invalid JSON API response")?;
                            let answers = response
                                .get("answers")
                                .and_then(Value::as_object)
                                .context("API response is missing an answers object")?;
                            return Ok(Value::Object(answers.clone()));
                        }
                        Err(error) if retryable_error(&error) => {
                            last_failure = "transient response transport failure".into();
                            if attempt == MAX_ATTEMPTS {
                                break;
                            }
                            let delay = retry_delay(attempt, None, SystemTime::now());
                            self.report_wait(&last_failure, cost, delay);
                            sleep_within_budget(
                                self.clock.as_ref(),
                                delay,
                                started,
                                budget,
                                attempt,
                                &last_failure,
                            )?;
                        }
                        Err(error) => return Err(error).context("reading API response"),
                    }
                }
                Err(error) if retryable_transport(&error) => {
                    last_failure = if error.is_timeout() {
                        "request timed out"
                    } else {
                        "transient connection failure"
                    }
                    .into();
                    if attempt == MAX_ATTEMPTS {
                        break;
                    }
                    let delay = retry_delay(attempt, None, SystemTime::now());
                    self.report_wait(&last_failure, cost, delay);
                    sleep_within_budget(
                        self.clock.as_ref(),
                        delay,
                        started,
                        budget,
                        attempt,
                        &last_failure,
                    )?;
                }
                Err(error) => return Err(error.without_url()).context("API request failed"),
            }
        }
        anyhow::bail!("API retry attempts exhausted ({MAX_ATTEMPTS}); last failure: {last_failure}")
    }

    fn report_wait(&self, reason: &str, cost: u64, delay: Duration) {
        let mut limiter = self.limiter.borrow_mut();
        limiter.expire(self.clock.now());
        let limit = limiter
            .limit
            .map_or_else(|| "disabled".into(), |n| n.to_string());
        eprintln!(
            "jev-grep: waiting: {reason}; request estimate={cost} tokens; occupied budget={}/{limit}; pause={:.3}s",
            limiter.used,
            delay.as_secs_f64()
        );
    }
}

fn retryable_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504 | 529)
}

fn retryable_transport(error: &reqwest::Error) -> bool {
    if error.is_timeout() {
        return true;
    }
    retryable_error(error)
}

fn retryable_error(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(current) = source {
        if let Some(io) = current.downcast_ref::<std::io::Error>()
            && retryable_io(io)
        {
            return true;
        }
        source = current.source();
    }
    false
}

fn retryable_io(error: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    matches!(
        error.kind(),
        ErrorKind::TimedOut
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionRefused
            | ErrorKind::ConnectionAborted
            | ErrorKind::BrokenPipe
            | ErrorKind::UnexpectedEof
    )
}

fn retry_delay(attempt: u32, retry_after: Option<&str>, now: SystemTime) -> Duration {
    let exponential = Duration::from_millis([200, 400, 800][(attempt - 1) as usize]);
    let jitter = Duration::from_millis(rand::thread_rng().gen_range(0..=100));
    let backoff = exponential + jitter;
    let server = retry_after.and_then(|value| {
        if value.bytes().all(|byte| byte.is_ascii_digit()) {
            Some(
                value
                    .parse::<u64>()
                    .map(Duration::from_secs)
                    .unwrap_or(Duration::MAX),
            )
        } else {
            httpdate::parse_http_date(value)
                .ok()
                .map(|date| date.duration_since(now).unwrap_or_default())
        }
    });
    server.map_or(backoff, |delay| backoff.max(delay))
}

fn sleep_within_budget(
    clock: &dyn Clock,
    delay: Duration,
    started: Instant,
    budget: Duration,
    attempt: u32,
    failure: &str,
) -> Result<()> {
    let remaining = budget.saturating_sub(clock.now().duration_since(started));
    ensure!(
        delay < remaining,
        "API retry budget exhausted after attempt {attempt}: {failure}"
    );
    clock.sleep(delay);
    ensure!(
        clock.now().duration_since(started) < budget,
        "API retry budget exhausted after attempt {attempt}: {failure}"
    );
    Ok(())
}

fn request_body(args: &Args, state: Value, questions: Value) -> Value {
    json!({"model": args.model, "state": state, "questions": questions})
}

fn question(instructions: &str) -> Value {
    json!({"type": "noul", "instructions": instructions})
}

fn probability(answers: &Value, id: &str) -> Result<f64> {
    let value = answers
        .get(id)
        .and_then(|answer| answer.get("noul"))
        .and_then(Value::as_f64)
        .with_context(|| format!("API response is missing numeric answers.{id}.noul"))?;
    ensure!(
        value.is_finite() && (0.0..=1.0).contains(&value),
        "answers.{id}.noul probability must be between 0 and 1"
    );
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        io::{Read, Write},
        net::TcpListener,
        rc::Rc,
        thread,
    };

    fn run_http(response: String, delay: Duration) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let count = stream.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
                let header_end = request.windows(4).position(|window| window == b"\r\n\r\n");
                if let Some(header_end) = header_end {
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|value| value.parse::<usize>().ok())
                        })
                        .unwrap();
                    if request.len() >= header_end + 4 + length {
                        break;
                    }
                }
            }
            thread::sleep(delay);
            let _ = stream.write_all(response.as_bytes());
            String::from_utf8(request).unwrap()
        });
        (format!("http://{address}/v1/systemone"), handle)
    }

    fn args(url: String, timeout: u64, api_key: Option<String>) -> Args {
        Args {
            query: "criterion".into(),
            paths: vec![],
            api_url: url,
            api_key,
            model: "checkpoint".into(),
            threshold: 0.5,
            join_threshold: 0.5,
            input_format: "text".into(),
            format: None,
            max_fragment_bytes: 16384,
            line_number: false,
            invert_match: false,
            count: false,
            files_with_matches: false,
            quiet: false,
            json: false,
            max_count: None,
            globs: vec![],
            hidden: false,
            no_ignore: false,
            with_filename: false,
            no_filename: false,
            max_line_bytes: 16384,
            timeout,
            tpm: None,
            tpm_safety_factor: 1.2,
        }
    }

    fn http_response(status: u16, body: &str) -> String {
        format!(
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn read_request(stream: &mut std::net::TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let count = stream.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..count]);
            if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|v| v.parse::<usize>().ok())
                    })
                    .unwrap();
                if request.len() >= header_end + 4 + length {
                    break;
                }
            }
        }
        String::from_utf8(request).unwrap()
    }

    #[test]
    fn partial_body_connection_reset_retries_same_request_once() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let body = json!({"answers":{"match":{"noul":0.9}}}).to_string();
        let response = http_response(200, &body);
        let server = thread::spawn(move || {
            let (mut first, _) = listener.accept().unwrap();
            let first_request = read_request(&mut first);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            first.write_all(header.as_bytes()).unwrap();
            first.write_all(&body.as_bytes()[..body.len() / 2]).unwrap();
            drop(first);
            let (mut second, _) = listener.accept().unwrap();
            let second_request = read_request(&mut second);
            second.write_all(response.as_bytes()).unwrap();
            (first_request, second_request)
        });
        let url = format!("http://{address}/v1/systemone");
        let mut args = args(url, 4, Some("same-token".into()));
        args.tpm = Some(100_000);
        let api = Api::new(&args).unwrap();
        assert_eq!(api.probability(&args, "A").unwrap(), 0.9);
        let (first, second) = server.join().unwrap();
        assert_eq!(first, second);
        let payload = first.split_once("\r\n\r\n").unwrap().1;
        assert_eq!(
            api.limiter.borrow().used,
            2 * estimate_request_tokens(payload.as_bytes(), 1.2).unwrap()
        );
        assert!(
            first
                .to_ascii_lowercase()
                .contains("authorization: bearer same-token")
        );
    }

    #[test]
    fn drip_fed_body_cannot_extend_evaluation_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{")
                .unwrap();
            for _ in 0..20 {
                thread::sleep(Duration::from_millis(100));
                if stream.write_all(b" ").is_err() {
                    break;
                }
            }
            request
        });
        let url = format!("http://{address}/v1/systemone");
        let args = args(url, 1, None);
        let started = Instant::now();
        assert!(Api::new(&args).unwrap().probability(&args, "A").is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        server.join().unwrap();
    }

    #[test]
    fn repeated_body_disconnects_stop_within_shared_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(2);
            let mut requests = Vec::new();
            while Instant::now() < until && requests.len() < 4 {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let request = read_request(&mut stream);
                        requests.push(request);
                        let _ = stream.write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{",
                        );
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) => panic!("accept failed: {error}"),
                }
            }
            requests
        });
        let url = format!("http://{address}/v1/systemone");
        let args = args(url, 1, None);
        let started = Instant::now();
        assert!(Api::new(&args).unwrap().probability(&args, "A").is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        let requests = server.join().unwrap();
        assert!(!requests.is_empty());
        assert!(requests.len() <= MAX_ATTEMPTS as usize);
    }

    #[test]
    fn group_call_sends_one_request_and_parses_independent_answers() {
        let body = json!({"answers": {
            "same_event": {"noul": 0.1}, "useful_context": {"noul": 0.9},
            "match_next": {"noul": 0.2}, "match_extended": {"noul": 0.8}
        }})
        .to_string();
        let (url, server) = run_http(http_response(200, &body), Duration::ZERO);
        let args = args(url, 2, Some("token".into()));
        let got = Api::new(&args)
            .unwrap()
            .group_probabilities(&args, "A", "B")
            .unwrap();
        assert_eq!(
            got,
            GroupProbabilities {
                same_event: 0.1,
                useful_context: 0.9,
                match_next: 0.2,
                match_extended: 0.8,
            }
        );
        let request = server.join().unwrap();
        let (headers, payload) = request.split_once("\r\n\r\n").unwrap();
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("authorization: bearer token")
        );
        let payload: Value = serde_json::from_str(payload).unwrap();
        assert_eq!(
            payload["state"],
            json!({"current_fragment":"A", "next_record":"B"})
        );
        assert_eq!(
            payload["questions"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            [
                "match_extended",
                "match_next",
                "same_event",
                "useful_context"
            ]
            .into_iter()
            .map(str::to_owned)
            .collect()
        );
        for id in [
            "same_event",
            "useful_context",
            "match_next",
            "match_extended",
        ] {
            assert_eq!(payload["questions"][id]["type"], "noul");
            assert!(payload["questions"][id]["instructions"].is_string());
        }
    }

    #[test]
    fn group_http_rejects_partial_or_invalid_answer_sets() {
        let complete = json!({"same_event":{"noul":0.5},"useful_context":{"noul":0.5},
            "match_next":{"noul":0.5},"match_extended":{"noul":0.5}});
        let cases = [
            json!({"same_event":{"noul":0.5},"useful_context":{"noul":0.5},"match_next":{"noul":0.5}}),
            json!({"same_event":{"noul":0.5},"useful_context":{"noul":"0.5"},"match_next":{"noul":0.5},"match_extended":{"noul":0.5}}),
            json!({"same_event":{"noul":0.5},"useful_context":{"noul":0.5},"match_next":{"noul":0.5},"match_extended":{"noul":1.1}}),
        ];
        for answers in cases {
            let response = json!({"answers": answers}).to_string();
            let (url, server) = run_http(http_response(200, &response), Duration::ZERO);
            let args = args(url, 2, None);
            assert!(
                Api::new(&args)
                    .unwrap()
                    .group_probabilities(&args, "A", "B")
                    .is_err()
            );
            server.join().unwrap();
        }
        assert_eq!(complete.as_object().unwrap().len(), 4);
    }

    #[test]
    fn transport_keeps_redirect_timeout_and_response_limit_rejected() {
        let (url, server) = run_http(http_response(302, ""), Duration::ZERO);
        let redirect_args = args(url, 2, None);
        assert!(
            Api::new(&redirect_args)
                .unwrap()
                .probability(&redirect_args, "text")
                .is_err()
        );
        server.join().unwrap();

        let (url, server) = run_http(http_response(200, "{}"), Duration::from_millis(1500));
        let timeout_args = args(url, 1, None);
        // A positive one-second deadline must expire before the delayed response.
        assert!(
            Api::new(&timeout_args)
                .unwrap()
                .probability(&timeout_args, "text")
                .is_err()
        );
        server.join().unwrap();

        let body = format!(
            "{{\"answers\":{{\"match\":{{\"noul\":0.5}}}},\"padding\":\"{}\"}}",
            "x".repeat(65537)
        );
        let (url, server) = run_http(http_response(200, &body), Duration::ZERO);
        let size_args = args(url, 2, None);
        assert!(
            Api::new(&size_args)
                .unwrap()
                .probability(&size_args, "text")
                .is_err()
        );
        server.join().unwrap();
    }

    #[test]
    fn grouped_answers_are_independent_and_all_required() {
        let answers = json!({
            "same_event": {"noul": 0.1},
            "useful_context": {"noul": 0.9},
            "match_next": {"noul": 0.2},
            "match_extended": {"noul": 0.8},
            "ignored": {"choice": "yes"}
        });
        assert_eq!(probability(&answers, "same_event").unwrap(), 0.1);
        assert_eq!(probability(&answers, "useful_context").unwrap(), 0.9);
        assert_eq!(probability(&answers, "match_next").unwrap(), 0.2);
        assert_eq!(probability(&answers, "match_extended").unwrap(), 0.8);
        for (id, value) in [
            ("missing", json!({})),
            ("string", json!({"noul": "0.5"})),
            ("bool", json!({"noul": true})),
            ("low", json!({"noul": -0.01})),
            ("high", json!({"noul": 1.01})),
        ] {
            let responses = json!({(id): value});
            assert!(probability(&responses, id).is_err(), "{id}");
        }
    }

    #[test]
    fn request_shapes_preserve_single_and_group_contracts() {
        let args = Args {
            query: "criterion".into(),
            paths: vec![],
            api_url: "http://localhost".into(),
            api_key: None,
            model: "checkpoint".into(),
            threshold: 0.5,
            join_threshold: 0.5,
            input_format: "text".into(),
            format: None,
            max_fragment_bytes: 16384,
            line_number: false,
            invert_match: false,
            count: false,
            files_with_matches: false,
            quiet: false,
            json: false,
            max_count: None,
            globs: vec![],
            hidden: false,
            no_ignore: false,
            with_filename: false,
            no_filename: false,
            max_line_bytes: 16384,
            timeout: 5,
            tpm: None,
            tpm_safety_factor: 1.2,
        };
        let one = request_body(
            &args,
            json!("text"),
            json!({"match": question(&args.query)}),
        );
        assert_eq!(one["state"], "text");
        assert_eq!(one["questions"].as_object().unwrap().len(), 1);
        assert_eq!(one["model"], "checkpoint");
        let group = request_body(
            &args,
            json!({"current_fragment":"A", "next_record":"B"}),
            json!({"same_event": question("same"), "useful_context": question("useful"),
                "match_next": question("next"), "match_extended": question("extended")}),
        );
        assert_eq!(group["state"]["current_fragment"], "A");
        assert_eq!(group["state"]["next_record"], "B");
        assert_eq!(group["questions"].as_object().unwrap().len(), 4);
    }

    #[test]
    fn retry_statuses_and_retry_after_values_are_bounded_safely() {
        for status in [408, 429, 500, 502, 503, 504, 529] {
            assert!(retryable_status(status));
        }
        for status in [200, 301, 400, 401, 403, 404, 422] {
            assert!(!retryable_status(status));
        }
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        assert!(retry_delay(1, Some("2"), now) >= Duration::from_secs(2));
        let future = now + Duration::from_secs(5);
        assert!(
            retry_delay(1, Some(&httpdate::fmt_http_date(future)), now) >= Duration::from_secs(5)
        );
        assert!(retry_delay(1, Some("not a date"), now) < Duration::from_secs(1));
        assert_eq!(
            retry_delay(1, Some("999999999999999999999999999999"), now),
            Duration::MAX
        );
        assert!(
            retry_delay(1, Some("Thu, 01 Jan 1970 00:00:00 GMT"), now) < Duration::from_secs(1)
        );
    }

    #[derive(Default)]
    struct TestClock {
        skipped: Cell<Duration>,
        sleeps: RefCell<Vec<Duration>>,
    }

    // Keep actual network time visible to request deadlines while advancing long
    // cooldowns instantly. The production HTTP transport remains under test.
    impl Clock for Rc<TestClock> {
        fn now(&self) -> Instant {
            Instant::now() + self.skipped.get()
        }

        fn sleep(&self, delay: Duration) {
            self.sleeps.borrow_mut().push(delay);
            self.skipped.set(self.skipped.get() + delay);
        }
    }

    fn scripted_http(responses: Vec<String>) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(5);
            let mut requests = Vec::new();
            for response in responses {
                loop {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            requests.push(read_request(&mut stream));
                            stream.write_all(response.as_bytes()).unwrap();
                            break;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < until, "expected another HTTP attempt");
                            thread::sleep(Duration::from_millis(1));
                        }
                        Err(error) => panic!("accept failed: {error}"),
                    }
                }
            }
            requests
        });
        (format!("http://{address}/v1/systemone"), server)
    }

    #[test]
    fn http_429_ignores_headers_and_waits_outside_timeout_even_without_tpm() {
        for header in [
            "",
            "Retry-After: 0\r\n",
            "Retry-After: 999999\r\n",
            "Retry-After: invalid\r\n",
        ] {
            let (url, server) = scripted_http(vec![
                format!(
                    "HTTP/1.1 429 Busy\r\n{header}Content-Length: 0\r\nConnection: close\r\n\r\n"
                ),
                http_response(200, r#"{"answers":{"match":{"noul":0.9}}}"#),
            ]);
            let args = args(url, 1, Some("same-token".into()));
            let mut api = Api::new(&args).unwrap();
            let clock = Rc::new(TestClock::default());
            api.clock = Box::new(clock.clone());
            assert_eq!(api.probability(&args, "text").unwrap(), 0.9);
            assert_eq!(*clock.sleeps.borrow(), vec![WINDOW]);
            let requests = server.join().unwrap();
            assert_eq!(requests[0], requests[1]);
        }
    }

    #[test]
    fn repeated_429_stops_after_four_attempts_and_three_cooldowns() {
        let (url, server) = scripted_http(vec![http_response(429, ""); 4]);
        let mut args = args(url, 1, None);
        args.tpm = Some(100_000);
        let mut api = Api::new(&args).unwrap();
        let clock = Rc::new(TestClock::default());
        api.clock = Box::new(clock.clone());
        let error = api.probability(&args, "text").unwrap_err().to_string();
        assert!(error.contains("attempts exhausted (4)"), "{error}");
        assert!(error.contains("HTTP 429"), "{error}");
        assert_eq!(*clock.sleeps.borrow(), vec![WINDOW; 3]);
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 4);
        assert!(requests.iter().all(|r| r == &requests[0]));
        // The final attempt has its own reservation after the prior one expires.
        assert_eq!(
            api.limiter.borrow().used,
            estimate_request_tokens(
                requests[0].split_once("\r\n\r\n").unwrap().1.as_bytes(),
                1.2
            )
            .unwrap()
        );
    }

    #[test]
    fn tpm_counts_retries_and_subsequent_evaluations_outside_api_timeout() {
        let success = http_response(200, r#"{"answers":{"match":{"noul":0.9}}}"#);
        let (url, server) = scripted_http(vec![http_response(503, ""), success.clone(), success]);
        let mut args = args(url, 1, None);
        let body = request_body(
            &args,
            json!("text"),
            json!({"match": question(&args.query)}),
        );
        let cost = estimate_request_tokens(&serde_json::to_vec(&body).unwrap(), 1.2).unwrap();
        args.tpm = Some(cost);
        let mut api = Api::new(&args).unwrap();
        let clock = Rc::new(TestClock::default());
        api.clock = Box::new(clock.clone());
        assert_eq!(api.probability(&args, "text").unwrap(), 0.9);
        assert_eq!(api.probability(&args, "text").unwrap(), 0.9);
        let sleeps = clock.sleeps.borrow();
        assert_eq!(sleeps.len(), 3); // ordinary backoff, retry TPM, next call TPM
        assert!(sleeps[0] < Duration::from_secs(1));
        assert!(sleeps[1] > Duration::from_secs(59));
        assert!(sleeps[2] > Duration::from_secs(59));
        assert_eq!(api.limiter.borrow().used, cost);
        let requests = server.join().unwrap();
        assert!(requests.iter().all(|r| r == &requests[0]));
    }

    #[test]
    fn full_growing_group_and_all_questions_are_charged() {
        let success = http_response(
            200,
            r#"{"answers":{"same_event":{"noul":0.9},"useful_context":{"noul":0.1},"match_next":{"noul":0.2},"match_extended":{"noul":0.8}}}"#,
        );
        let (url, server) = scripted_http(vec![success.clone(), success]);
        let mut args = args(url, 2, None);
        args.query = "ошибки выполнения команд".into();
        args.tpm = Some(100_000);
        let api = Api::new(&args).unwrap();
        api.group_probabilities(&args, "first\n", "second\n")
            .unwrap();
        api.group_probabilities(&args, "first\nsecond\n", "third\n")
            .unwrap();
        let requests = server.join().unwrap();
        let costs: Vec<_> = requests
            .iter()
            .map(|r| {
                let body = r.split_once("\r\n\r\n").unwrap().1;
                let payload: Value = serde_json::from_str(body).unwrap();
                assert_eq!(payload["questions"].as_object().unwrap().len(), 4);
                estimate_request_tokens(body.as_bytes(), 1.2).unwrap()
            })
            .collect();
        assert!(costs[1] > costs[0]);
        assert_eq!(api.limiter.borrow().used, costs.iter().sum::<u64>());
    }
}
