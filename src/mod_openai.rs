use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use futures::StreamExt;
use std::io::Write;

use crate::utils::{get_env, get_model_to_use, write_resp_to_file};

pub async fn mod_openai(prompt: &str) -> anyhow::Result<()> {
    let model = get_model_to_use("OPENAI_MODEL", "gpt-4o-mini");
    let api_key = get_env("OPENAI_API_KEY", "");

    if get_env("SESEPUH_HUB_RES_ONLY", "0") != "1" {
        println!("\nOpenAI model: {}\n", model);
    }

    if api_key.is_empty() {
        anyhow::bail!("OPENAI_API_KEY is not set");
    }

    let config = OpenAIConfig::new().with_api_key(api_key);
    let client = Client::with_config(config);

    if get_env("SESEPUH_HUB_STREAMING", "0") == "1" {
        mod_openai_stream(&client, prompt, &model).await
    } else {
        mod_openai_sync(&client, prompt, &model).await
    }
}

async fn mod_openai_sync(client: &Client<OpenAIConfig>, prompt: &str, model: &str) -> anyhow::Result<()> {
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

async fn mod_openai_stream(client: &Client<OpenAIConfig>, prompt: &str, model: &str) -> anyhow::Result<()> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .seed(0_i64)
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
                std::io::stdout().flush().ok(); // flush to ensure real-time streaming display
                result_buf.push_str(content);
            }
            if choice.finish_reason.is_some() {
                println!(); // newline after last stream chunk
            }
        }
    }

    write_resp_to_file(result_buf.as_bytes(), "")?;
    Ok(())
}
