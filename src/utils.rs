use std::env;
use std::fs;
use std::path::PathBuf;

/// Returns the model to use, checking SESEPUH_HUB_MODEL first, then the vendor-specific
/// env var, and finally falling back to the default model.
pub fn get_model_to_use(env_var_name: &str, default_model: &str) -> String {
    if let Ok(model) = env::var("SESEPUH_HUB_MODEL") {
        if !model.is_empty() {
            return model;
        }
    }

    if let Ok(model) = env::var(env_var_name) {
        if !model.is_empty() {
            return model;
        }
    }

    default_model.to_string()
}

/// Returns the value of the environment variable, or the default if not set.
pub fn get_env(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Writes response content to `$XDG_CONFIG_HOME/sepuh/.response.txt` by default,
/// or to the provided path if non-empty.
pub fn write_resp_to_file(content: &[u8], filename: &str) -> anyhow::Result<()> {
    let path: PathBuf = if filename.is_empty() {
        let config_home = match env::var("XDG_CONFIG_HOME") {
            Ok(value) if !value.is_empty() => PathBuf::from(value),
            _ => PathBuf::from(get_env("HOME", ".")).join(".config"),
        };
        let sepuh_dir = config_home.join("sepuh");
        fs::create_dir_all(&sepuh_dir)?;
        sepuh_dir.join(".response.txt")
    } else {
        PathBuf::from(filename)
    };

    fs::write(path, content)?;
    Ok(())
}
