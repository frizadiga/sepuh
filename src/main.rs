mod mod_openai;
mod mod_xai;
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
        "openai" => mod_openai::mod_openai(&args.prompt).await?,
        "xai" => mod_xai::mod_xai(&args.prompt).await?,
        other => {
            eprintln!("Error: Unknown vendor '{}'", other);
            std::process::exit(1);
        }
    }

    Ok(())
}
