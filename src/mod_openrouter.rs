use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use futures::StreamExt;
use std::io::Write;

use crate::utils::{get_env, get_model_to_use, write_resp_to_file};

pub async fn mod_openrouter(prompt: &str) -> anyhow::Result<()> {
    let model = get_model_to_use("OPENROUTER_MODEL", "google/gemini-3-flash-preview");
    let api_key = get_env("OPENROUTER_API_KEY", "");

    if get_env("SESEPUH_HUB_RES_ONLY", "0") != "1" {
        println!("\nOpenRouter model: {}\n", model);
    }

    if api_key.is_empty() {
        anyhow::bail!("OPENROUTER_API_KEY is not set");
    }

    let config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base("https://openrouter.ai/api/v1");
    let client = Client::with_config(config);

    if get_env("SESEPUH_HUB_STREAMING", "0") == "1" {
        mod_openrouter_stream(&client, prompt, &model).await
    } else {
        mod_openrouter_sync(&client, prompt, &model).await
    }
}

async fn mod_openrouter_sync(client: &Client<OpenAIConfig>, prompt: &str, model: &str) -> anyhow::Result<()> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages([ChatCompletionRequestUserMessageArgs::default()
            .content(prompt)
            .build()?
            .into()])
        .build()?;

    let response = client.chat().create(request).await?;
    let content = response
        .choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .unwrap_or("");

    println!("{}", content);
    write_resp_to_file(content.as_bytes(), "")?;
    Ok(())
}

async fn mod_openrouter_stream(client: &Client<OpenAIConfig>, prompt: &str, model: &str) -> anyhow::Result<()> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages([ChatCompletionRequestUserMessageArgs::default()
            .content(prompt)
            .build()?
            .into()])
        .build()?;

    let mut stream = client.chat().create_stream(request).await?;
    let mut result_buf = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        for choice in &chunk.choices {
            if let Some(content) = choice.delta.content.as_deref() {
                print!("{}", content);
                std::io::stdout().flush().ok();
                result_buf.push_str(content);
            }
            if choice.finish_reason.is_some() {
                println!();
            }
        }
    }

    write_resp_to_file(result_buf.as_bytes(), "")?;
    Ok(())
}
