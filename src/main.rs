mod mod_google;
mod mod_ollama;
mod mod_openai;
mod mod_openrouter;
mod mod_xai;
mod openai_compat;
mod sse;
mod utils;

use clap::Parser;
use utils::get_env;

#[derive(Parser)]
#[command(name = "sepuh")]
struct Args {
    #[arg(long, required = true)]
    prompt: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let vendor = get_env("SESEPUH_HUB_VENDOR", "");

    if get_env("SESEPUH_HUB_RES_ONLY", "0") != "1" {
        println!("🧙 Sepuh");
    }

    match vendor.as_str() {
        "google" => mod_google::mod_google(&args.prompt).await?,
        "ollama" => mod_ollama::mod_ollama(&args.prompt).await?,
        "openai" => mod_openai::mod_openai(&args.prompt).await?,
        "openrouter" => mod_openrouter::mod_openrouter(&args.prompt).await?,
        "xai" => mod_xai::mod_xai(&args.prompt).await?,
        other => {
            eprintln!("Error: Unknown vendor '{}'", other);
            std::process::exit(1);
        }
    }

    Ok(())
}
