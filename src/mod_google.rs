use reqwest::Client;
use serde_json::{json, Value};
use std::io::Write;

use crate::sse::for_each_sse_payload;
use crate::utils::{get_env, get_model_to_use, write_resp_to_file};

pub async fn mod_google(prompt: &str) -> anyhow::Result<()> {
    let model = get_model_to_use("GOOGLE_MODEL", "gemini-3-flash-preview");
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

    let mut result_buf = String::new();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let mut in_thought = false;
    let mut saw_answer = false;

    // ANSI dim grey for thought tokens, reset when leaving thought mode.
    const THOUGHT_START: &str = "\x1b[2;37m"; // dim, white-ish
    const THOUGHT_END: &str = "\x1b[0m";

    for_each_sse_payload(response, |json| {
        let Some(parts) = json["candidates"][0]["content"]["parts"].as_array() else {
            return Ok(());
        };
        for part in parts {
            let Some(text) = part["text"].as_str() else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            if part["thought"].as_bool() == Some(true) {
                if !in_thought {
                    eprint!("{}", THOUGHT_START);
                    in_thought = true;
                }
                eprint!("{}", text);
                stderr.flush().ok();
            } else {
                if in_thought {
                    eprint!("{}", THOUGHT_END);
                    eprintln!();
                    in_thought = false;
                }
                saw_answer = true;
                print!("{}", text);
                stdout.flush().ok();
                result_buf.push_str(text);
            }
        }
        Ok(())
    })
    .await?;

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
