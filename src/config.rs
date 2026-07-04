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

pub fn lang_from_path(file_path: &str) -> &'static str {
    match file_path.rsplit('.').next() {
        Some("py") => "python",
        Some("rs") => "rust",
        Some("js") => "javascript",
        Some("ts") => "typescript",
        Some("go") => "go",
        Some("java") => "java",
        Some("c") => "c",
        Some("cpp" | "cc" | "cxx" | "hpp" | "hxx" | "h") => "cpp",
        _ => "python",
    }
}

pub fn test_cmd_for_lang(config: &Config, lang: &str) -> String {
    config
        .test_cmd
        .clone()
        .unwrap_or_else(|| default_test_cmd(lang).to_string())
}

pub fn default_test_cmd(lang: &str) -> &'static str {
    match lang {
        "rust" => "cargo test",
        "javascript" | "typescript" => "npx jest",
        "java" => "mvn test",
        "cpp" => "g++ -std=c++17",
        "c" => "gcc -std=c17",
        "go" => "go test",
        _ => "pytest",
    }
}

pub fn timeout_secs(config: &Config) -> u32 {
    config.timeout.unwrap_or(30)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_from_path_maps_known_extensions() {
        assert_eq!(lang_from_path("src/app.py"), "python");
        assert_eq!(lang_from_path("lib.rs"), "rust");
        assert_eq!(lang_from_path("a/b/index.js"), "javascript");
        assert_eq!(lang_from_path("index.ts"), "typescript");
        assert_eq!(lang_from_path("main.go"), "go");
        assert_eq!(lang_from_path("Foo.java"), "java");
        assert_eq!(lang_from_path("util.c"), "c");
        assert_eq!(lang_from_path("util.cpp"), "cpp");
        assert_eq!(lang_from_path("util.hpp"), "cpp");
    }

    #[test]
    fn test_cmd_override_wins() {
        let cfg = Config {
            test_cmd: Some("just test".to_string()),
            ..Default::default()
        };
        assert_eq!(test_cmd_for_lang(&cfg, "rust"), "just test");
    }

    #[test]
    fn test_cmd_defaults_per_lang() {
        let cfg = Config::default();
        assert_eq!(test_cmd_for_lang(&cfg, "rust"), "cargo test");
        assert_eq!(test_cmd_for_lang(&cfg, "typescript"), "npx jest");
        assert_eq!(test_cmd_for_lang(&cfg, "cpp"), "g++ -std=c++17");
        assert_eq!(test_cmd_for_lang(&cfg, "c"), "gcc -std=c17");
        assert_eq!(test_cmd_for_lang(&cfg, "python"), "pytest");
    }

    #[test]
    fn file_matches_respects_include_and_exclude() {
        let cfg = Config {
            include: Some(vec!["**/*.py".to_string()]),
            exclude: Some(vec!["**/tests/*".to_string()]),
            ..Default::default()
        };
        assert!(file_matches("src/app.py", &cfg));
        assert!(!file_matches("src/tests/test_app.py", &cfg));
        assert!(!file_matches("src/app.rs", &cfg));
    }
}
