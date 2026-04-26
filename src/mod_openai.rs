use crate::openai_compat::{chat_stream, chat_sync};
use crate::utils::{get_env, get_model_to_use, write_resp_to_file};

pub async fn mod_openai(prompt: &str) -> anyhow::Result<()> {
    let model = get_model_to_use("OPENAI_MODEL", "gpt-4o-mini");
    let api_key = get_env("OPENAI_API_KEY", "");
    let base_url = get_env("OPENAI_URL", "https://api.openai.com/v1");

    if get_env("SESEPUH_HUB_RES_ONLY", "0") != "1" {
        println!("\nOpenAI model: {}\n", model);
    }

    if api_key.is_empty() {
        anyhow::bail!("OPENAI_API_KEY is not set");
    }

    let client = reqwest::Client::new();

    let content = if get_env("SESEPUH_HUB_STREAMING", "0") == "1" {
        chat_stream(&client, &base_url, &api_key, &model, prompt, Some(0)).await?
    } else {
        let content = chat_sync(&client, &base_url, &api_key, &model, prompt).await?;
        println!("{}", content);
        content
    };

    write_resp_to_file(content.as_bytes(), "")?;
    Ok(())
}
