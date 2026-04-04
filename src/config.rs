use std::path::Path;

use crate::types::Config;

const CONFIG_FILE: &str = ".min-loc.toml";

pub fn load_config(cwd: &Path) -> Option<Config> {
    let path = cwd.join(CONFIG_FILE);
    let content = std::fs::read_to_string(path).ok()?;
    let config: Config = toml::from_str(&content).unwrap_or_default();
    Some(config)
}

pub fn file_matches(file_path: &str, config: &Config) -> bool {
    if let Some(ref excludes) = config.exclude {
        for pattern in excludes {
            if glob_match::glob_match(pattern, file_path) {
                return false;
            }
        }
    }
    if let Some(ref includes) = config.include {
        return includes
            .iter()
            .any(|pattern| glob_match::glob_match(pattern, file_path));
    }
    true
}

pub fn lang(config: &Config) -> &str {
    config.lang.as_deref().unwrap_or("python")
}

pub fn test_cmd(config: &Config) -> &str {
    config.test_cmd.as_deref().unwrap_or("pytest")
}

pub fn module_name(config: &Config) -> &str {
    config.module_name.as_deref().unwrap_or("solution")
}

pub fn timeout_secs(config: &Config) -> u32 {
    config.timeout.unwrap_or(30)
}
