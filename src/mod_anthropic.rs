use crate::openai_compat::{chat_stream, chat_sync};
use crate::utils::{get_env, get_model_to_use, write_resp_to_file};

pub async fn mod_anthropic(prompt: &str) -> anyhow::Result<()> {
    let model = get_model_to_use("ANTHROPIC_MODEL", "claude-sonnet-4-6");
    let api_key = get_env("ANTHROPIC_API_KEY", "");
    let base_url = get_env("ANTHROPIC_URL", "https://api.anthropic.com/v1");

    if get_env("SEPUH_RES_ONLY", "0") != "1" {
        println!("\nAnthropic model: {}\n", model);
    }

    if api_key.is_empty() {
        anyhow::bail!("ANTHROPIC_API_KEY is not set");
    }

    let client = reqwest::Client::new();

    let content = if get_env("SEPUH_STREAMING", "0") == "1" {
        chat_stream(&client, &base_url, &api_key, &model, prompt, Some(2)).await?
    } else {
        let content = chat_sync(&client, &base_url, &api_key, &model, prompt).await?;
        println!("{}", content);
        content
    };

    write_resp_to_file(content.as_bytes(), "")?;
    Ok(())
}
