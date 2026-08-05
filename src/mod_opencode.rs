use std::io::Write;

use futures::StreamExt;
use serde_json::{json, Value};

use crate::utils::{get_env, get_model_to_use, write_resp_to_file};

const DEFAULT_BASE_URL: &str = "http://localhost:4096";
const DEFAULT_AGENT: &str = "plan";

pub async fn mod_opencode(prompt: &str) -> anyhow::Result<()> {
    let model = get_model_to_use("OPENCODE_MODEL", "");
    let base_url = get_env("OPENCODE_BASE_URL", DEFAULT_BASE_URL);
    let agent = get_env("OPENCODE_AGENT", DEFAULT_AGENT);

    if get_env("SEPUH_RES_ONLY", "0") != "1" {
        println!("\nOpenCode agent: {}{}\n", agent, model_label(&model));
        if get_env("SEPUH_REASONING", "0") == "1" {
            println!("reasoning \u{2192} stderr\n");
        }
    }

    let client = reqwest::Client::new();

    let content = if get_env("SEPUH_STREAMING", "0") == "1" {
        opencode_stream(&client, &base_url, &agent, &model, prompt).await?
    } else {
        let content = opencode_sync(&client, &base_url, &agent, &model, prompt).await?;
        println!("{}", content);
        content
    };

    write_resp_to_file(content.as_bytes(), "")?;
    Ok(())
}

fn model_label(model: &str) -> String {
    if model.is_empty() {
        String::new()
    } else {
        format!(" | model: {}", model)
    }
}

fn auth(rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    let password = get_env("OPENCODE_SERVER_PASSWORD", "");
    if password.is_empty() {
        return rb;
    }
    let username = get_env("OPENCODE_SERVER_USERNAME", "opencode");
    rb.basic_auth(username, Some(password))
}

fn build_prompt_body(agent: &str, model: &str, prompt: &str) -> Value {
    let mut body = json!({
        "parts": [{ "type": "text", "text": prompt }],
        "agent": agent,
    });
    if let Some((provider_id, model_id)) = split_model(model) {
        body["model"] = json!({
            "providerID": provider_id,
            "modelID": model_id,
        });
    }
    body
}

/// Split `openrouter/z-ai/glm-5.2` into `(openrouter, z-ai/glm-5.2)` by
/// splitting on the first slash. Returns `None` for empty input.
fn split_model(model: &str) -> Option<(&str, &str)> {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (provider_id, model_id) = trimmed.split_once('/')?;
    if provider_id.is_empty() || model_id.is_empty() {
        return None;
    }
    Some((provider_id, model_id))
}

async fn create_session(client: &reqwest::Client, base_url: &str) -> anyhow::Result<String> {
    let url = format!("{}/session", base_url.trim_end_matches('/'));
    let resp = auth(client.post(&url)).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!(
            "opencode session creation failed ({}): {}",
            status,
            if text.is_empty() { "<empty body>" } else { &text }
        );
    }
    let value: Value = serde_json::from_str(&text).map_err(|e| {
        anyhow::anyhow!("failed to parse opencode session response ({}): {}", e, text)
    })?;
    value["id"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("opencode session response missing `id` field: {}", text))
}

async fn opencode_sync(
    client: &reqwest::Client,
    base_url: &str,
    agent: &str,
    model: &str,
    prompt: &str,
) -> anyhow::Result<String> {
    let session_id = create_session(client, base_url).await?;
    let url = format!(
        "{}/session/{}/message",
        base_url.trim_end_matches('/'),
        session_id
    );
    let body = build_prompt_body(agent, model, prompt);
    let resp = auth(client.post(&url))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!(
            "opencode message request failed ({}): {}",
            status,
            if text.is_empty() { "<empty body>" } else { &text }
        );
    }
    let value: Value = serde_json::from_str(&text).map_err(|e| {
        anyhow::anyhow!("failed to parse opencode message response ({}): {}", e, text)
    })?;
    Ok(extract_text_parts(&value))
}

fn extract_text_parts(value: &Value) -> String {
    let parts = value["parts"].as_array();
    let mut text = String::new();
    if let Some(parts) = parts {
        for part in parts {
            if part["type"].as_str() == Some("text") {
                if let Some(t) = part["text"].as_str() {
                    if !t.is_empty() {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(t);
                    }
                }
            }
        }
    }
    text
}

async fn opencode_stream(
    client: &reqwest::Client,
    base_url: &str,
    agent: &str,
    model: &str,
    prompt: &str,
) -> anyhow::Result<String> {
    let session_id = create_session(client, base_url).await?;

    // Open the global SSE event stream before firing the prompt so we
    // don't miss early `message.part.*` events. The `/event` stream is
    // shared across all sessions; we filter by `sessionID` below.
    let event_url = format!("{}/event", base_url.trim_end_matches('/'));
    let event_resp = auth(client.get(&event_url))
        .header("Accept", "text/event-stream")
        .send()
        .await?;
    let event_status = event_resp.status();
    if !event_status.is_success() {
        let body = event_resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "opencode /event stream open failed ({}): {}",
            event_status,
            if body.is_empty() { "<empty body>" } else { &body }
        );
    }

    // Fire the prompt asynchronously. The server replies 204 immediately
    // and drives the turn to completion, emitting events on the `/event`
    // stream we already hold open above.
    let prompt_url = format!(
        "{}/session/{}/prompt_async",
        base_url.trim_end_matches('/'),
        session_id
    );
    let body = build_prompt_body(agent, model, prompt);
    let prompt_resp = auth(client.post(&prompt_url))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;
    let prompt_status = prompt_resp.status();
    if !prompt_status.is_success() {
        let resp_body = prompt_resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "opencode prompt_async failed ({}): {}",
            prompt_status,
            if resp_body.is_empty() { "<empty body>" } else { &resp_body }
        );
    }

    let mut result_buf = String::new();
    let mut reasoning_part_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let show_reasoning = get_env("SEPUH_REASONING", "0") == "1";
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let mut stream = event_resp.bytes_stream();
    let mut buf = String::new();
    let mut done = false;

    // Manual SSE loop. Unlike the shared `for_each_sse_payload` helper,
    // we break out as soon as `session.idle` arrives — opencode keeps the
    // global `/event` stream open forever, so waiting for EOF would hang.
    while !done {
        let chunk = match stream.next().await {
            Some(Ok(c)) => c,
            Some(Err(e)) => return Err(anyhow::anyhow!("opencode /event stream error: {}", e)),
            None => break,
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));

        // Drain complete SSE frames (terminated by a blank line).
        while let Some(pos) = buf.find("\n\n") {
            let frame: String = buf.drain(..pos + 2).collect();
            for payload in parse_event_payloads(&frame) {
                let props = &payload["properties"];
                if props["sessionID"].as_str() != Some(session_id.as_str()) {
                    continue;
                }
                let event_type = payload["type"].as_str().unwrap_or("");
                match event_type {
                    "message.part.updated" => {
                        // Reasoning parts arrive as a `part.updated` first
                        // (which creates them empty), then stream their text
                        // via `part.delta` with field "text" — same as the
                        // final answer. Track their partIDs so we can tell
                        // the two streams apart.
                        if props["part"]["type"].as_str() == Some("reasoning") {
                            if let Some(id) = props["part"]["id"].as_str() {
                                reasoning_part_ids.insert(id.to_string());
                            }
                        }
                    }
                    "message.part.delta" => {
                        if props["field"].as_str() == Some("text") {
                            if let Some(delta) = props["delta"].as_str() {
                                if !delta.is_empty() {
                                    let is_reasoning = props["partID"]
                                        .as_str()
                                        .map(|id| reasoning_part_ids.contains(id))
                                        .unwrap_or(false);
                                    if is_reasoning {
                                        if show_reasoning {
                                            eprint!("{}", delta);
                                            stderr.flush().ok();
                                        }
                                    } else {
                                        print!("{}", delta);
                                        stdout.flush().ok();
                                        result_buf.push_str(delta);
                                    }
                                }
                            }
                        }
                    }
                    "session.idle" => {
                        println!();
                        stdout.flush().ok();
                        done = true;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(result_buf)
}

/// Parse an SSE frame (one or more `data:` lines followed by a blank
/// line) into JSON payloads. Skips comments, blank lines, `[DONE]`
/// sentinels, and lines that fail JSON parsing.
fn parse_event_payloads(frame: &str) -> Vec<Value> {
    let mut payloads = Vec::new();
    let mut data_lines: Vec<&str> = Vec::new();
    for line in frame.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(':') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("data:") {
            let rest = rest.trim_start();
            if rest.is_empty() || rest == "[DONE]" {
                continue;
            }
            data_lines.push(rest);
        }
    }
    if data_lines.is_empty() {
        return payloads;
    }
    let joined = data_lines.join("\n");
    if let Ok(value) = serde_json::from_str::<Value>(&joined) {
        payloads.push(value);
    }
    payloads
}
