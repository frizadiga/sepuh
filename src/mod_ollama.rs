use std::io::Write;
use tokio::io::{AsyncBufReadExt, BufReader as AsyncBufReader};
use tokio::process::Command;

use crate::utils::{get_env, get_model_to_use, write_resp_to_file};

pub async fn mod_ollama(prompt: &str) -> anyhow::Result<()> {
    let model = get_model_to_use("OLLAMA_MODEL", "gemma4:e2b");

    if get_env("SEPUH_RES_ONLY", "0") != "1" {
        println!("\nOllama model: {}\n", model);
    }

    if get_env("SEPUH_STREAMING", "0") == "1" {
        mod_ollama_stream(prompt, &model).await
    } else {
        mod_ollama_sync(prompt, &model).await
    }
}

async fn mod_ollama_sync(prompt: &str, model: &str) -> anyhow::Result<()> {
    let output = Command::new("ollama")
        .args(["run", model, prompt])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ollama command failed: {}", stderr);
    }

    let content = String::from_utf8_lossy(&output.stdout);
    print!("{}", content);
    write_resp_to_file(output.stdout.as_slice(), "")?;
    Ok(())
}

async fn mod_ollama_stream(prompt: &str, model: &str) -> anyhow::Result<()> {
    use std::process::Stdio;

    let mut child = tokio::process::Command::new("ollama")
        .args(["run", model, prompt])
        .stdout(Stdio::piped())
        .spawn()?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to capture ollama stdout"))?;

    let mut result_buf = String::new();

    let mut reader = AsyncBufReader::new(stdout);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        let trimmed = line.trim_end_matches('\n');
        if !trimmed.is_empty() {
            println!("{}", trimmed);
            std::io::stdout().flush().ok();
            result_buf.push_str(trimmed);
            result_buf.push('\n');
        }
    }

    let status = child.wait().await?;
    if !status.success() {
        anyhow::bail!("ollama command exited with status: {}", status);
    }

    write_resp_to_file(result_buf.as_bytes(), "")?;
    Ok(())
}
