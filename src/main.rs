mod compress;
mod config;
mod counting;
mod hook;
mod testing;
mod types;

use std::io::Read;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use crate::types::{HookInput, ParsedToolInput};

#[derive(Parser)]
#[command(name = "min-loc", about = "Minimum LOC enforcer for Claude Code")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Check {
        file: PathBuf,
        #[arg(short = 'f', long)]
        test_file: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Check { file, test_file }) => run_check(&file, test_file),
        None => run_hook(),
    }
}

fn run_hook() {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() || input.trim().is_empty() {
        return;
    }

    let hook_input: HookInput = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return,
    };

    let cwd = hook_input
        .cwd
        .as_deref()
        .map(Path::new)
        .unwrap_or(Path::new("."));

    let cfg = match config::load_config(cwd) {
        Some(c) => c,
        None => return,
    };

    let parsed = match parse_tool_input(&hook_input) {
        Some(p) => p,
        None => return,
    };

    let (file_path, content) = match &parsed {
        ParsedToolInput::Write { file_path, content } => (file_path.as_str(), content.clone()),
        ParsedToolInput::Edit {
            file_path,
            old_string,
            new_string,
        } => {
            let full = reconstruct_edit(file_path, old_string, new_string);
            (file_path.as_str(), full)
        }
    };

    let relative = make_relative(file_path, &cwd.to_string_lossy());
    if !config::file_matches(&relative, &cfg) {
        return;
    }

    let lang = config::lang_from_path(file_path);
    let test_cmd = config::test_cmd_for_lang(&cfg, lang);
    let timeout = config::timeout_secs(&cfg);

    match compress::run(&content, lang, &test_cmd, timeout, cwd, &relative) {
        Ok(result) => {
            if result.final_loc < result.original_loc {
                let reason = format!(
                    "min-loc: compressed from {} to {} non-import lines in {} rounds. \
                     Replace your code with this shorter version:\n\n{}\n",
                    result.original_loc, result.final_loc, result.rounds, result.code,
                );
                print!("{}", hook::deny(reason));
            }
        }
        Err(e) => {
            eprintln!("[min-loc] compression skipped: {e}");
        }
    }
}

fn run_check(file: &Path, test_file: Option<String>) {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error reading {}: {e}", file.display());
            std::process::exit(1);
        }
    };

    let file_str = file.to_string_lossy();
    let lang = config::lang_from_path(&file_str);
    let test_cmd = config::default_test_cmd(lang);

    let stats = counting::count_loc(&content, lang);
    println!("total: {}", stats.total);
    println!("non-blank: {}", stats.non_blank);
    println!("non-import: {}", stats.non_import);

    if test_file.is_some() {
        let cwd = file.parent().unwrap_or(Path::new("."));
        let file_name = file.file_name().unwrap_or_default().to_string_lossy();
        match compress::run(&content, lang, test_cmd, 30, cwd, &file_name) {
            Ok(result) => {
                println!(
                    "compressed: {} -> {} lines ({} rounds)",
                    result.original_loc, result.final_loc, result.rounds
                );
                if result.final_loc < result.original_loc {
                    println!("--- shorter version ---");
                    println!("{}", result.code);
                } else {
                    println!("already minimal");
                }
            }
            Err(e) => {
                eprintln!("compression failed: {e}");
                std::process::exit(1);
            }
        }
    }
}

fn parse_tool_input(input: &HookInput) -> Option<ParsedToolInput> {
    let obj = input.tool_input.as_object()?;
    match input.tool_name.as_str() {
        "Write" => {
            let file_path = obj.get("file_path")?.as_str()?.to_string();
            let content = obj.get("content")?.as_str()?.to_string();
            Some(ParsedToolInput::Write { file_path, content })
        }
        "Edit" => {
            let file_path = obj.get("file_path")?.as_str()?.to_string();
            let old_string = obj.get("old_string")?.as_str()?.to_string();
            let new_string = obj.get("new_string")?.as_str()?.to_string();
            Some(ParsedToolInput::Edit {
                file_path,
                old_string,
                new_string,
            })
        }
        _ => None,
    }
}

fn reconstruct_edit(file_path: &str, old_string: &str, new_string: &str) -> String {
    match std::fs::read_to_string(file_path) {
        Ok(existing) => existing.replacen(old_string, new_string, 1),
        Err(_) => new_string.to_string(),
    }
}

fn make_relative(file_path: &str, cwd: &str) -> String {
    file_path
        .strip_prefix(cwd)
        .unwrap_or(file_path)
        .trim_start_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconstruct_edit_splices_old_for_new() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.py");
        std::fs::write(&path, "a\nb\nc\n").unwrap();
        let result = reconstruct_edit(&path.to_string_lossy(), "b", "B");
        assert_eq!(result, "a\nB\nc\n");
    }

    #[test]
    fn reconstruct_edit_falls_back_to_new_string() {
        assert_eq!(reconstruct_edit("/nonexistent/x.py", "a", "b"), "b");
    }

    #[test]
    fn make_relative_strips_cwd() {
        assert_eq!(make_relative("/repo/src/a.py", "/repo"), "src/a.py");
        assert_eq!(make_relative("src/a.py", "/other"), "src/a.py");
    }
}
