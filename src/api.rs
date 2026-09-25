use anyhow::{Context, Result, ensure};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::io::Read;
use std::time::Duration;

use crate::Args;

pub struct Api {
    client: Client,
}

const MAX_RESPONSE_BYTES: usize = 65536;

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
            "use LG_API_KEY instead of URL credentials"
        );
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(args.timeout))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
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
        let mut request = self.client.post(&args.api_url).json(body);
        if let Some(key) = &args.api_key {
            request = request.bearer_auth(key);
        }
        let response = request
            .send()
            .map_err(|e| e.without_url())
            .context("API request failed")?;
        ensure!(
            response.status().is_success(),
            "API returned HTTP {}",
            response.status()
        );
        let mut bytes = Vec::new();
        response
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .context("reading API response")?;
        ensure!(
            bytes.len() <= MAX_RESPONSE_BYTES,
            "API response exceeds {MAX_RESPONSE_BYTES} bytes"
        );
        let response: Value =
            serde_json::from_slice(&bytes).context("invalid JSON API response")?;
        let answers = response
            .get("answers")
            .and_then(Value::as_object)
            .context("API response is missing an answers object")?;
        Ok(Value::Object(answers.clone()))
    }
}

fn request_body(args: &Args, state: Value, questions: Value) -> Value {
    let mut body = json!({"state": state, "questions": questions});
    if let Some(model) = &args.model {
        body["model"] = json!(model);
    }
    body
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
        io::{Read, Write},
        net::TcpListener,
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
            model: Some("checkpoint".into()),
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
        }
    }

    fn http_response(status: u16, body: &str) -> String {
        format!(
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
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
            model: Some("checkpoint".into()),
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
}
