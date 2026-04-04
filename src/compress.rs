use std::path::Path;
use std::process::Command;

use crate::counting;
use crate::testing;

const MAX_ROUNDS: u32 = 5;

const GENERATE_TESTS_PROMPT: &str = r#"Generate pytest tests for this {lang} code. Import from the module '{module_name}'.
Test all core behavior paths. Cover edge cases.
Output ONLY the pytest test code. No explanation, no markdown fences.

{code}"#;

const COMPRESS_PROMPT: &str = r#"Rewrite this {lang} code using the absolute minimum number of lines while preserving identical behavior. Every line you can eliminate matters.

Techniques to apply:
- List/dict/set comprehensions instead of loops
- Walrus operator (:=) to merge assignment and condition
- Semicolons to combine independent statements on one line
- Lambda instead of trivial named functions
- Ternary expressions instead of if/else blocks
- stdlib one-liners (itertools, functools, collections)
- Unpack and inline wherever possible
- Remove all comments, docstrings, type hints
- Merge multiple return paths

Output ONLY the rewritten code. No explanation, no markdown fences, no original code.

{code}"#;

pub struct CompressResult {
    pub code: String,
    pub original_loc: u32,
    pub final_loc: u32,
    pub rounds: u32,
}

pub fn run(
    code: &str,
    lang: &str,
    module_name: &str,
    test_cmd: &str,
    timeout_secs: u32,
    cwd: &Path,
) -> Result<CompressResult, String> {
    log("generating tests via claude -p");
    let tests = generate_tests(code, lang, module_name)?;
    log(&format!(
        "generated tests ({} lines)",
        tests.lines().count()
    ));

    log("red check: tests should fail with empty module");
    let empty_module = "";
    let red = testing::run_tests(
        empty_module,
        &tests,
        module_name,
        test_cmd,
        timeout_secs,
        cwd,
    );
    if red.passed {
        return Err(
            "generated tests pass with empty implementation (tests are trivial or broken)"
                .to_string(),
        );
    }
    log("red check passed (tests failed as expected)");

    log("green check: tests should pass with submitted code");
    let green = testing::run_tests(code, &tests, module_name, test_cmd, timeout_secs, cwd);
    if !green.passed {
        return Err(format!(
            "code does not pass generated tests:\n{}",
            green.output
        ));
    }
    log("green check passed");

    let original_loc = counting::count_loc(code, lang).non_import;
    let mut champion = code.to_string();
    let mut champion_loc = original_loc;
    let mut rounds = 0u32;
    let mut stalls = 0u32;

    log(&format!(
        "starting compression (original: {} non-import lines)",
        original_loc
    ));

    while rounds < MAX_ROUNDS && stalls < 2 && champion_loc > 1 {
        rounds += 1;
        log(&format!("compression round {rounds}"));
        let compressed = match call_claude_compress(&champion, lang) {
            Ok(c) => c,
            Err(e) => {
                log(&format!("claude compress failed: {e}"));
                break;
            }
        };

        let new_loc = counting::count_loc(&compressed, lang).non_import;
        log(&format!("compressed to {new_loc} non-import lines"));

        if new_loc >= champion_loc {
            log("no improvement, stall");
            stalls += 1;
            continue;
        }

        let result = testing::run_tests(
            &compressed,
            &tests,
            module_name,
            test_cmd,
            timeout_secs,
            cwd,
        );
        if !result.passed {
            log(&format!(
                "compressed version fails tests: {}",
                result.output
            ));
            stalls += 1;
            continue;
        }

        log(&format!(
            "improvement: {} -> {} non-import lines",
            champion_loc, new_loc
        ));
        champion = compressed;
        champion_loc = new_loc;
        stalls = 0;
    }

    log(&format!(
        "done: {} -> {} non-import lines in {rounds} rounds",
        original_loc, champion_loc
    ));

    Ok(CompressResult {
        code: champion,
        original_loc,
        final_loc: champion_loc,
        rounds,
    })
}

fn log(msg: &str) {
    eprintln!("[min-loc] {msg}");
}

fn generate_tests(code: &str, lang: &str, module_name: &str) -> Result<String, String> {
    let prompt = GENERATE_TESTS_PROMPT
        .replace("{lang}", lang)
        .replace("{module_name}", module_name)
        .replace("{code}", code);
    let output = call_claude(&prompt)?;
    Ok(extract_code_block(&output))
}

fn call_claude_compress(code: &str, lang: &str) -> Result<String, String> {
    let prompt = COMPRESS_PROMPT
        .replace("{lang}", lang)
        .replace("{code}", code);
    let output = call_claude(&prompt)?;
    Ok(extract_code_block(&output))
}

fn call_claude(prompt: &str) -> Result<String, String> {
    let output = Command::new("claude")
        .arg("-p")
        .arg("--output-format")
        .arg("text")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(prompt.as_bytes()).ok();
            }
            drop(child.stdin.take());
            child.wait_with_output()
        })
        .map_err(|e| format!("failed to run claude: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("claude exited with error: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err("claude returned empty output".to_string());
    }
    Ok(stdout)
}

fn extract_code_block(s: &str) -> String {
    let text = s.trim();
    if let Some(start) = text.find("```") {
        let after_backticks = &text[start + 3..];
        let code_start = after_backticks.find('\n').map(|i| i + 1).unwrap_or(0);
        let code_body = &after_backticks[code_start..];
        if let Some(end) = code_body.find("```") {
            return code_body[..end].trim().to_string();
        }
        return code_body.trim().to_string();
    }
    text.to_string()
}
