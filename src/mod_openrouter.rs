use crate::openai_compat::{chat_stream, chat_sync};
use crate::utils::{get_env, get_model_to_use, write_resp_to_file};

pub async fn mod_openrouter(prompt: &str) -> anyhow::Result<()> {
    let model = get_model_to_use("OPENROUTER_MODEL", "google/gemini-3-flash-preview");
    let api_key = get_env("OPENROUTER_API_KEY", "");
    let base_url = get_env("OPENROUTER_URL", "https://openrouter.ai/api/v1");

    if get_env("SESEPUH_HUB_RES_ONLY", "0") != "1" {
        println!("\nOpenRouter model: {}\n", model);
    }

    if api_key.is_empty() {
        anyhow::bail!("OPENROUTER_API_KEY is not set");
    }

    let client = reqwest::Client::new();

    let content = if get_env("SESEPUH_HUB_STREAMING", "0") == "1" {
        chat_stream(&client, &base_url, &api_key, &model, prompt, None).await?
    } else {
        let content = chat_sync(&client, &base_url, &api_key, &model, prompt).await?;
        println!("{}", content);
        content
    };

    write_resp_to_file(content.as_bytes(), "")?;
    Ok(())
}
