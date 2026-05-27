mod mod_anthropic;
mod mod_google;
mod mod_ollama;
mod mod_openai;
mod mod_openrouter;
mod mod_xai;
mod openai_compat;
mod sse;
mod utils;

use std::io::Read;

use clap::Parser;
use utils::get_env;

#[derive(Parser)]
#[command(name = "sepuh")]
struct Args {
    #[arg(long)]
    prompt: Option<String>,
}

fn read_prompt(args: &Args) -> String {
    if let Some(ref prompt) = args.prompt {
        return prompt.clone();
    }
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .expect("Failed to read prompt from stdin");
    buf
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let prompt = read_prompt(&args);
    let provider = get_env("SEPUH_PROVIDER", "");

    if get_env("SEPUH_RES_ONLY", "0") != "1" {
        println!("🧙 Sepuh");
    }

    match provider.as_str() {
        "anthropic" => mod_anthropic::mod_anthropic(&prompt).await?,
        "google" => mod_google::mod_google(&prompt).await?,
        "ollama" => mod_ollama::mod_ollama(&prompt).await?,
        "openai" => mod_openai::mod_openai(&prompt).await?,
        "openrouter" => mod_openrouter::mod_openrouter(&prompt).await?,
        "xai" => mod_xai::mod_xai(&prompt).await?,
        other => {
            eprintln!("Error: Unknown provider '{}'", other);
            std::process::exit(1);
        }
    }

    Ok(())
}
