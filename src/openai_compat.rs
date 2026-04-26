use serde_json::{json, Value};
use std::io::Write;

use crate::sse::for_each_sse_payload;

/// Sends a non-streaming chat completion request to an OpenAI-compatible
/// endpoint (OpenAI, OpenRouter, xAI, ...). Returns the assistant text from
/// `choices[0].message.content` (empty string if missing).
pub async fn chat_sync(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
) -> anyhow::Result<String> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "stream": false,
    });

    let response = client
        .post(&url)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        anyhow::bail!(
            "Chat completion request failed with status {}: {}",
            status,
            if text.is_empty() { "<empty body>" } else { &text }
        );
    }

    let json: Value = serde_json::from_str(&text).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse chat completion response as JSON ({}). Body: {}",
            e,
            if text.is_empty() { "<empty body>" } else { &text }
        )
    })?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    Ok(content)
}

/// Sends a streaming chat completion request and prints each token to stdout
/// as it arrives. Returns the full collected text. `seed` is forwarded to the
/// API when `Some`.
pub async fn chat_stream(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    seed: Option<i64>,
) -> anyhow::Result<String> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let mut body = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "stream": true,
    });
    if let Some(seed) = seed {
        body["seed"] = json!(seed);
    }

    let response = client
        .post(&url)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .json(&body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "Chat completion stream request failed with status {}: {}",
            status,
            if text.is_empty() { "<empty body>" } else { &text }
        );
    }

    let mut result_buf = String::new();
    let mut stdout = std::io::stdout();
    let mut newline_done = false;

    for_each_sse_payload(response, |json| {
        let Some(choices) = json["choices"].as_array() else {
            return Ok(());
        };
        for choice in choices {
            if let Some(content) = choice["delta"]["content"].as_str() {
                if !content.is_empty() {
                    print!("{}", content);
                    stdout.flush().ok();
                    result_buf.push_str(content);
                }
            }
            if !choice["finish_reason"].is_null() && !newline_done {
                println!();
                newline_done = true;
            }
        }
        Ok(())
    })
    .await?;

    if !newline_done {
        println!();
    }

    Ok(result_buf)
}
