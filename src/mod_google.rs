use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::io::Write;

use crate::utils::{get_env, get_model_to_use, write_resp_to_file};

pub async fn mod_google(prompt: &str) -> anyhow::Result<()> {
    let model = get_model_to_use("GOOGLE_MODEL", "gemini-2.0-flash");
    let api_key = get_env("GEMINI_API_KEY", "");

    if get_env("SESEPUH_HUB_RES_ONLY", "0") != "1" {
        println!("\nGoogle model: {}\n", model);
    }

    if api_key.is_empty() {
        anyhow::bail!("GEMINI_API_KEY is not set");
    }

    // The Gemini API expects bare model names (e.g. `gemini-2.5-flash`).
    // Strip a leading `google/` if the user selected an OpenRouter-style name.
    let api_model = model.strip_prefix("google/").unwrap_or(&model);

    let client = Client::new();

    if get_env("SESEPUH_HUB_STREAMING", "0") == "1" {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            api_model, api_key
        );
        mod_google_stream(&client, &url, prompt).await
    } else {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            api_model, api_key
        );
        mod_google_sync(&client, &url, prompt).await
    }
}

fn build_request_body(prompt: &str) -> Value {
    json!({
        "contents": [{
            "parts": [{
                "text": prompt
            }]
        }],
        "generationConfig": {
            "thinkingConfig": {
                "includeThoughts": true
            }
        }
    })
}

async fn mod_google_sync(client: &Client, url: &str, prompt: &str) -> anyhow::Result<()> {
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&build_request_body(prompt))
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        anyhow::bail!(
            "Gemini API request failed with status {}: {}",
            status,
            if text.is_empty() { "<empty body>" } else { &text }
        );
    }

    let json: Value = serde_json::from_str(&text).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse Gemini response as JSON ({}). Body: {}",
            e,
            if text.is_empty() { "<empty body>" } else { &text }
        )
    })?;

    let content = json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("");

    println!("{}", content);
    write_resp_to_file(content.as_bytes(), "")?;
    Ok(())
}

async fn mod_google_stream(client: &Client, url: &str, prompt: &str) -> anyhow::Result<()> {
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .json(&build_request_body(prompt))
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "Gemini API request failed with status {}: {}",
            status,
            if text.is_empty() { "<empty body>" } else { &text }
        );
    }

    let debug = get_env("SESEPUH_HUB_DEBUG", "0") == "1";
    let mut stream = response.bytes_stream();
    let mut buf = String::new();
    let mut result_buf = String::new();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let t0 = std::time::Instant::now();
    let mut chunk_no: u32 = 0;
    let mut in_thought = false;
    let mut saw_answer = false;

    // ANSI dim grey for thought tokens, reset when leaving thought mode.
    const THOUGHT_START: &str = "\x1b[2;37m"; // dim, white-ish
    const THOUGHT_END: &str = "\x1b[0m";

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        chunk_no += 1;
        if debug {
            eprintln!(
                "[chunk #{} t={}ms size={}B]",
                chunk_no,
                t0.elapsed().as_millis(),
                chunk.len()
            );
        }
        buf.push_str(&String::from_utf8_lossy(&chunk));

        // Drain complete lines (terminated by \n). Normalize \r.
        loop {
            let Some(nl) = buf.find('\n') else { break };
            let mut line: String = buf.drain(..=nl).collect();
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
            for event in handle_stream_line(&line) {
                match event {
                    PartEvent::Thought(text) => {
                        if !in_thought {
                            eprint!("{}", THOUGHT_START);
                            in_thought = true;
                        }
                        eprint!("{}", text);
                        stderr.flush().ok();
                    }
                    PartEvent::Answer(text) => {
                        if in_thought {
                            eprint!("{}", THOUGHT_END);
                            eprintln!();
                            in_thought = false;
                        }
                        saw_answer = true;
                        print!("{}", text);
                        stdout.flush().ok();
                        result_buf.push_str(&text);
                    }
                }
            }
        }
    }

    // Flush any trailing partial line (e.g. no final newline).
    if !buf.is_empty() {
        for event in handle_stream_line(buf.trim()) {
            match event {
                PartEvent::Thought(text) => {
                    if !in_thought {
                        eprint!("{}", THOUGHT_START);
                        in_thought = true;
                    }
                    eprint!("{}", text);
                    stderr.flush().ok();
                }
                PartEvent::Answer(text) => {
                    if in_thought {
                        eprint!("{}", THOUGHT_END);
                        eprintln!();
                        in_thought = false;
                    }
                    saw_answer = true;
                    print!("{}", text);
                    stdout.flush().ok();
                    result_buf.push_str(&text);
                }
            }
        }
    }

    if in_thought {
        eprint!("{}", THOUGHT_END);
        eprintln!();
    }
    if saw_answer {
        println!();
    }
    write_resp_to_file(result_buf.as_bytes(), "")?;
    Ok(())
}

enum PartEvent {
    Thought(String),
    Answer(String),
}

/// Parses one line from Gemini's streaming body into zero or more
/// `PartEvent`s (thought tokens and answer tokens). Supports both SSE
/// (`data: {json}`) and the JSON-array stream format.
///
/// Requires `generationConfig.thinkingConfig.includeThoughts=true` in the
/// request so reasoning models emit their thinking as `parts` with
/// `thought: true` and a `text` field.
fn handle_stream_line(line: &str) -> Vec<PartEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let payload = if let Some(rest) = trimmed.strip_prefix("data:") {
        let rest = rest.trim();
        if rest.is_empty() || rest == "[DONE]" {
            return Vec::new();
        }
        rest
    } else {
        let stripped = trimmed
            .trim_start_matches(['[', ',', ' '])
            .trim_end_matches([']', ',', ' ']);
        if stripped.is_empty() {
            return Vec::new();
        }
        stripped
    };

    let Ok(json): Result<Value, _> = serde_json::from_str(payload) else {
        return Vec::new();
    };
    let Some(parts) = json["candidates"][0]["content"]["parts"].as_array() else {
        return Vec::new();
    };

    let mut events = Vec::new();
    for part in parts {
        let Some(text) = part["text"].as_str() else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        if part["thought"].as_bool() == Some(true) {
            events.push(PartEvent::Thought(text.to_string()));
        } else {
            events.push(PartEvent::Answer(text.to_string()));
        }
    }
    events
}
