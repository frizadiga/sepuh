use std::io::Write;

use futures::StreamExt;
use serde_json::{json, Value};

use crate::utils::{get_env, get_model_to_use, write_resp_to_file};

const DEFAULT_BASE_URL: &str = "http://localhost:4096";
const DEFAULT_BASE_URL_V2: &str = "http://localhost:4097";
const DEFAULT_AGENT: &str = "plan";

pub async fn mod_opencode(prompt: &str) -> anyhow::Result<()> {
    // `OPENCODE_USE_VERSION` selects the OpenCode server generation to talk to.
    // Default is "1" so existing v1 setups keep working unchanged.
    let version = get_env("OPENCODE_USE_VERSION", "1");
    let is_v2 = version == "2";
    let streaming = get_env("SEPUH_STREAMING", "0") == "1";

    let model = get_model_to_use("OPENCODE_MODEL", "");
    let base_url = get_env(
        "OPENCODE_BASE_URL",
        if is_v2 {
            DEFAULT_BASE_URL_V2
        } else {
            DEFAULT_BASE_URL
        },
    );
    let agent = get_env("OPENCODE_AGENT", DEFAULT_AGENT);

    if get_env("SEPUH_RES_ONLY", "0") != "1" {
        println!(
            "\nOpenCode agent: {}{}{}\n",
            agent,
            model_label(&model),
            if is_v2 { " (v2)" } else { "" }
        );
        if get_env("SEPUH_REASONING", "0") == "1" {
            println!("reasoning → stderr\n");
        }
    }

    let client = reqwest::Client::new();

    // v2 has no sync response body: the assistant reply is delivered only over the
    // /api/event SSE stream, so `opencode_v2` owns both live-printing (when
    // streaming) and buffering. For the non-streaming case it returns the buffered
    // text, which we print once below.
    let content = if is_v2 {
        opencode_v2(&client, &base_url, &agent, &model, prompt).await?
    } else if streaming {
        opencode_stream(&client, &base_url, &agent, &model, prompt).await?
    } else {
        opencode_sync(&client, &base_url, &agent, &model, prompt).await?
    };

    if !streaming {
        println!("{}", content);
    }

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

// ---------------------------------------------------------------------------
// OpenCode v1 (`opencode serve`, port 4096)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// OpenCode v2 (`opencode2 serve`, port 4097)
//
// Differences from v1:
//   * routes are prefixed with /api (e.g. /api/session, /api/event)
//   * session creation response is wrapped: {"data":{...}}
//   * the prompt is sent as {"text": ...} and the agent/model are chosen at
//     session creation time; model override uses {"providerID","id"}
//   * the answer arrives only over the SSE stream as `session.text.delta`
//     events; completion is signalled by `session.execution.succeeded`
// ---------------------------------------------------------------------------

async fn create_session_v2(
    client: &reqwest::Client,
    base_url: &str,
    agent: &str,
    model: &str,
) -> anyhow::Result<String> {
    let url = format!("{}/api/session", base_url.trim_end_matches('/'));
    let mut body = json!({ "agent": agent });
    if let Some((provider_id, model_id)) = split_model(model) {
        // v2 names the model id field `id` (v1 uses `modelID`).
        body["model"] = json!({ "providerID": provider_id, "id": model_id });
    }
    let resp = auth(client.post(&url))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!(
            "opencode v2 session creation failed ({}): {}",
            status,
            if text.is_empty() { "<empty body>" } else { &text }
        );
    }
    let value: Value = serde_json::from_str(&text).map_err(|e| {
        anyhow::anyhow!("failed to parse opencode v2 session response ({}): {}", e, text)
    })?;
    value["data"]["id"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| {
            anyhow::anyhow!("opencode v2 session response missing `data.id` field: {}", text)
        })
}

async fn opencode_v2(
    client: &reqwest::Client,
    base_url: &str,
    agent: &str,
    model: &str,
    prompt: &str,
) -> anyhow::Result<String> {
    let session_id = create_session_v2(client, base_url, agent, model).await?;

    // Open the global SSE event stream before firing the prompt so we don't
    // miss early events. The stream is shared across all sessions; we filter
    // by `data.sessionID` below. Each `data:` line is one JSON event:
    // {"id","type","data":{...},"location":{...},"durable":{...}}.
    let event_url = format!("{}/api/event", base_url.trim_end_matches('/'));
    let event_resp = auth(client.get(&event_url))
        .header("Accept", "text/event-stream")
        .send()
        .await?;
    let event_status = event_resp.status();
    if !event_status.is_success() {
        let body = event_resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "opencode v2 /api/event stream open failed ({}): {}",
            event_status,
            if body.is_empty() { "<empty body>" } else { &body }
        );
    }

    // Fire the prompt. The server replies 200 immediately (echoing the user
    // message) and drives the turn to completion, emitting events on /api/event.
    //
    // OpenCode 2 builds disagree (and have drifted) on the prompt field shape,
    // so we try candidate bodies in order and use the first one the server
    // accepts: `text` (plain string, current builds), `prompt` (plain string,
    // older builds), then `prompt` as a `PromptInput` object (future builds).
    let prompt_url = format!(
        "{}/api/session/{}/prompt",
        base_url.trim_end_matches('/'),
        session_id
    );
    let candidates = [
        json!({ "text": prompt }),
        json!({ "prompt": prompt }),
        json!({ "prompt": { "text": prompt } }),
    ];
    let mut prompt_ok = false;
    let mut last_status = 0u16;
    let mut last_body = String::new();
    for body in &candidates {
        let resp = auth(client.post(&prompt_url))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            prompt_ok = true;
            break;
        }
        last_status = status.as_u16();
        last_body = resp.text().await.unwrap_or_default();
    }
    if !prompt_ok {
        anyhow::bail!(
            "opencode v2 prompt failed ({}): {}",
            last_status,
            if last_body.is_empty() { "<empty body>" } else { &last_body }
        );
    }

    let show_reasoning = get_env("SEPUH_REASONING", "0") == "1";
    let streaming = get_env("SEPUH_STREAMING", "0") == "1";
    let mut result_buf = String::new();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let mut stream = event_resp.bytes_stream();
    let mut buf = String::new();
    let mut done = false;

    while !done {
        let chunk = match stream.next().await {
            Some(Ok(c)) => c,
            Some(Err(e)) => return Err(anyhow::anyhow!("opencode v2 /api/event stream error: {}", e)),
            None => break,
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buf.find("\n\n") {
            let frame: String = buf.drain(..pos + 2).collect();
            for payload in parse_event_payloads(&frame) {
                let event = match payload.get("data") {
                    Some(e) => e,
                    None => continue,
                };
                if event["sessionID"].as_str() != Some(session_id.as_str()) {
                    continue;
                }
                let event_type = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match event_type {
                    "session.text.delta" => {
                        if let Some(delta) = event["delta"].as_str() {
                            if !delta.is_empty() {
                                if streaming {
                                    print!("{}", delta);
                                    stdout.flush().ok();
                                }
                                result_buf.push_str(delta);
                            }
                        }
                    }
                    "session.reasoning.delta" => {
                        if show_reasoning {
                            if let Some(delta) = event["delta"].as_str() {
                                if !delta.is_empty() {
                                    eprint!("{}", delta);
                                    stderr.flush().ok();
                                }
                            }
                        }
                    }
                    "session.execution.succeeded" => {
                        println!();
                        stdout.flush().ok();
                        done = true;
                    }
                    "session.execution.failed" => {
                        let msg = event["error"]["message"]
                            .as_str()
                            .unwrap_or("unknown error");
                        return Err(anyhow::anyhow!("opencode v2 execution failed: {}", msg));
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
