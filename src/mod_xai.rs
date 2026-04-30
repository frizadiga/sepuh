use crate::openai_compat::{chat_stream, chat_sync};
use crate::sse::for_each_sse_payload;
use crate::utils::{get_env, get_model_to_use, write_resp_to_file};
use serde_json::{json, Value};
use std::io::Write;

/** Number of seconds before reqwest times out a request. */
static TIMEOUT_SECS: u64 = 120;

pub async fn mod_xai(prompt: &str) -> anyhow::Result<()> {
    let model = get_model_to_use("XAI_MODEL", "grok-4-fast-reasoning");
    let api_key = get_env("XAI_API_KEY", "");
    let base_url = get_env("XAI_URL", "https://api.x.ai/v1");

    if get_env("SEPUH_RES_ONLY", "0") != "1" {
        println!("\nXAI model: {}\n", model);
    }

    if api_key.is_empty() {
        anyhow::bail!("XAI_API_KEY is not set");
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .build()?;

    let is_debug = get_env("SEPUH_DEBUG", "0") == "1";
    let is_web_search = get_env("SEPUH_WEB_SEARCH", "0") == "1";
    let is_x_search = get_env("SEPUH_X_SEARCH", "0") == "1";
    if is_debug {
        println!(
            "[sepuh] SEPUH_WEB_SEARCH={} SEPUH_X_SEARCH={} -> use_responses_api={}",
            is_web_search,
            is_x_search,
            is_web_search || is_x_search
        );
    }

    let content = if is_web_search || is_x_search {
        responses_api(&client, &base_url, &api_key, &model, prompt, is_web_search, is_x_search).await?
    } else {
        let content = if get_env("SEPUH_STREAMING", "0") == "1" {
            chat_stream(&client, &base_url, &api_key, &model, prompt, Some(1)).await?
        } else {
            let content = chat_sync(&client, &base_url, &api_key, &model, prompt).await?;
            println!("{}", content);
            content
        };
        content
    };

    write_resp_to_file(content.as_bytes(), "")?;
    Ok(())
}

async fn responses_api(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    web_search: bool,
    x_search: bool,
) -> anyhow::Result<String> {
    let url = format!("{}/responses", base_url.trim_end_matches('/'));
    let mut tools = Vec::new();
    if web_search {
        tools.push(json!({ "type": "web_search" }));
    }
    if x_search {
        tools.push(json!({ "type": "x_search" }));
    }

    let streaming = get_env("SEPUH_STREAMING", "0") == "1";
    let is_debug = get_env("SEPUH_DEBUG", "0") == "1";

    let body = json!({
        "model": model,
        "input": [
            { "role": "user", "content": prompt }
        ],
        "tools": tools,
        "stream": streaming
    });
    let builder = client
        .post(&url)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .json(&body);

    if is_debug {
        println!("[sepuh] POST {}  streaming={}  tools={}", url, streaming, tools.len());
    }
    std::io::stdout().flush().ok();

    let response = builder.send().await?;
    let status = response.status();
    if is_debug {
        println!("[sepuh] response status: {}", status);
    }
    std::io::stdout().flush().ok();

    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "xAI Responses API failed with status {}: {}",
            status,
            if text.is_empty() { "<empty body>" } else { &text }
        );
    }

    if streaming {
        let mut result_buf = String::new();
        let mut stdout = std::io::stdout();
        let mut newline_done = false;
        for_each_sse_payload(response, |json| {
            let event_type = json["type"].as_str().unwrap_or("");
            if is_debug {
                println!(
                    "[sepuh][sse] type={} keys={:?}",
                    event_type,
                    json.as_object()
                        .map(|o| o.keys().cloned().collect::<Vec<_>>())
                        .unwrap_or_default()
                );
            }

            // xAI Responses API event types:
            //   response.output_text.delta  -> { delta: "text chunk" }
            //   response.output_text.done   -> { text: "full text" }
            //   response.completed          -> end of stream
            // Try several known patterns for delta text
            let delta_text = json["delta"].as_str()
                .or_else(|| json["delta"]["text"].as_str())
                .or_else(|| json["text"].as_str());

            if event_type == "response.output_text.delta" || event_type.ends_with(".delta") {
                if let Some(d) = delta_text {
                    if !d.is_empty() {
                        print!("{}", d);
                        stdout.flush().ok();
                        result_buf.push_str(d);
                    }
                }
            }

            if event_type == "response.completed" || event_type == "response.done" {
                if !newline_done {
                    println!();
                    newline_done = true;
                }
                if result_buf.is_empty() {
                    if let Some(outputs) = json["response"]["output"].as_array() {
                        for output in outputs {
                            if output["type"] == "message" {
                                if let Some(content_array) = output["content"].as_array() {
                                    for item in content_array {
                                        if let Some(text) = item["text"].as_str() {
                                            print!("{}", text);
                                            stdout.flush().ok();
                                            result_buf.push_str(text);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        })
        .await?;

        if !newline_done {
            println!();
        }

        Ok(result_buf)
    } else {
        let text = response.text().await?;
        if is_debug {
            println!("[sepuh] body length: {} bytes", text.len());
            println!("--- raw xAI responses API body ---");
            println!("{}", text);
            println!("--- end raw body ---");
        }
        std::io::stdout().flush().ok();

        let json: Value = serde_json::from_str(&text).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse xAI Responses API response as JSON ({}). Body: {}",
                e,
                &text
            )
        })?;

        // The responses API returns output array; look for type="message" and extract content
        let mut collected = String::new();
        if let Some(outputs) = json["output"].as_array() {
            for output in outputs {
                if output["type"] == "message" {
                    if let Some(content_array) = output["content"].as_array() {
                        for item in content_array {
                            if let Some(text) = item["text"].as_str() {
                                collected.push_str(text);
                            }
                        }
                    }
                }
            }
        }

        if collected.is_empty() {
            // Fallback patterns
            if let Some(s) = json["output_text"].as_str() {
                collected.push_str(s);
            } else if let Some(s) = json["text"].as_str() {
                collected.push_str(s);
            }
        }

        if collected.is_empty() {
            if is_debug {
                println!(
                    "[sepuh] WARN: no extractable text. Set SEPUH_DEBUG=1 to dump raw body."
                );
                println!("[sepuh] status field: {}", json["status"].as_str().unwrap_or("<none>"));
                if let Some(err) = json["error"].as_object() {
                    println!("[sepuh] error: {:?}", err);
                }
            }
        } else {
            println!("{}", collected);
        }
        Ok(collected)
    }
}
