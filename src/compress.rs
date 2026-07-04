use std::path::Path;
use std::process::Command;

use crate::counting;
use crate::testing;

const MAX_ROUNDS: u32 = 5;

const TRIAGE_PROMPT: &str = r#"Look at this code. Can it be rewritten in fewer lines while preserving identical behavior? Consider merging statements, using language idioms, removing redundancy.
Answer ONLY "YES" or "NO". Nothing else.

{code}"#;

const PYTHON_TESTS_PROMPT: &str = r#"Generate pytest tests for this python code. Import from the module '{module}'.
Test all core behavior paths. Cover edge cases.
Output ONLY the pytest test code. No explanation, no markdown fences.

{code}"#;

const RUST_TESTS_PROMPT: &str = r#"Generate Rust unit test functions for this code. Use #[test] attribute on each function.
Do NOT wrap in a mod tests block or use super::*. Just write the test functions directly.
Test all core behavior paths. Cover edge cases.
Output ONLY the test functions. No explanation, no markdown fences.

{code}"#;

const PYTHON_COMPRESS_PROMPT: &str = r#"Rewrite this python code using the absolute minimum number of lines while preserving identical behavior. Every line you can eliminate matters.

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

const RUST_COMPRESS_PROMPT: &str = r#"Rewrite this rust code using the absolute minimum number of lines while preserving identical behavior. Every line you can eliminate matters.

Techniques to apply:
- Iterator chains instead of for loops
- Closures instead of trivial named functions
- if let / match arms on one line where possible
- Combine statements with semicolons
- Use map, filter, fold, collect instead of manual accumulation
- Remove all comments and doc comments
- Merge multiple return paths with match or if-else expressions
- Use tuple destructuring and pattern matching

Output ONLY the rewritten code. No explanation, no markdown fences, no original code.

{code}"#;

const JAVA_TESTS_PROMPT: &str = r#"Generate JUnit 5 tests for this Java code. The class under test is named '{module}'.
Declare the same package as the code under test; omit the package declaration if the code has none. Use static imports for assertions.
Test all core behavior paths. Cover edge cases.
Output ONLY the JUnit test code. No explanation, no markdown fences.

{code}"#;

const JAVA_COMPRESS_PROMPT: &str = r#"Rewrite this Java code using the absolute minimum number of lines while preserving identical behavior. Every line you can eliminate matters.

Techniques to apply:
- Ternary expressions instead of if/else blocks
- Stream API (map, filter, reduce, collect) instead of loops
- Combine statements on fewer lines
- Lambda expressions instead of anonymous classes
- Method references where possible
- Remove all comments and Javadoc
- Merge multiple return paths
- Inline trivial variables
- var for local variable declarations

Output ONLY the rewritten code. No explanation, no markdown fences, no original code.

{code}"#;

const CPP_TESTS_PROMPT: &str = r#"Generate C++ test code for this code. Include "{module}" at the top.
Use <cassert> for assertions. Write a main() function that runs all tests and returns 0 on success.
Test all core behavior paths. Cover edge cases.
Output ONLY the C++ test code. No explanation, no markdown fences.

{code}"#;

const CPP_COMPRESS_PROMPT: &str = r#"Rewrite this C++ code using the absolute minimum number of lines while preserving identical behavior. Every line you can eliminate matters.

Techniques to apply:
- Range-based for with auto instead of index loops
- Algorithm/numeric headers (transform, accumulate, for_each) instead of manual loops
- Ternary expressions instead of if/else blocks
- Lambda expressions for short operations
- auto for type deduction
- Structured bindings
- Remove all comments and documentation
- Combine declarations on fewer lines
- Merge multiple return paths
- Use initializer lists and aggregate initialization

Output ONLY the rewritten code. No explanation, no markdown fences, no original code.

{code}"#;

const GO_TESTS_PROMPT: &str = r#"Generate Go test functions for this code. The package is '{module}'.
Use the standard testing package. Each test function takes *testing.T.
Test all core behavior paths. Cover edge cases.
Output ONLY the import declarations you need (including "testing") followed by the test functions. No package declaration.

{code}"#;

const GO_COMPRESS_PROMPT: &str = r#"Rewrite this Go code using the absolute minimum number of lines while preserving identical behavior. Every line you can eliminate matters.

Techniques to apply:
- Short variable declarations (:=) instead of var
- Combine assignments on fewer lines
- Use range loops efficiently
- Inline trivial helper functions
- Remove all comments
- Merge multiple return paths with switch or if-else expressions
- Use slice/map literals inline

Output ONLY the rewritten code. No explanation, no markdown fences, no original code.

{code}"#;

const C_TESTS_PROMPT: &str = r#"Generate C test code for this code. Include "{module}" at the top.
Use <assert.h> for assertions. Write a main() function that runs all tests and returns 0 on success.
Test all core behavior paths. Cover edge cases.
Output ONLY the C test code. No explanation, no markdown fences.

{code}"#;

const C_COMPRESS_PROMPT: &str = r#"Rewrite this C code using the absolute minimum number of lines while preserving identical behavior. Every line you can eliminate matters.

Techniques to apply:
- Ternary expressions instead of if/else blocks
- Combine declarations on fewer lines
- Comma operator to combine statements
- Remove all comments
- Merge multiple return paths
- Use compound literals and designated initializers
- Inline trivial helper functions

Output ONLY the rewritten code. No explanation, no markdown fences, no original code.

{code}"#;

const JS_TESTS_PROMPT: &str = r#"Generate Jest tests for this JavaScript code. Import from '{module}'.
Test all core behavior paths. Cover edge cases.
Output ONLY the Jest test code. No explanation, no markdown fences.

{code}"#;

const TS_TESTS_PROMPT: &str = r#"Generate Jest tests for this TypeScript code. Import from '{module}'.
Test all core behavior paths. Cover edge cases.
Output ONLY the Jest test code. No explanation, no markdown fences.

{code}"#;

const JS_COMPRESS_PROMPT: &str = r#"Rewrite this JavaScript code using the absolute minimum number of lines while preserving identical behavior. Every line you can eliminate matters.

Techniques to apply:
- Arrow functions instead of function declarations
- Ternary expressions instead of if/else blocks
- Array methods (map, filter, reduce) instead of loops
- Destructuring and spread operators
- Comma operator to combine statements
- Template literals for string concatenation
- Short-circuit evaluation (&&, ||, ??)
- Remove all comments and JSDoc
- Object shorthand properties
- Optional chaining and nullish coalescing

Output ONLY the rewritten code. No explanation, no markdown fences, no original code.

{code}"#;

const TS_COMPRESS_PROMPT: &str = r#"Rewrite this TypeScript code using the absolute minimum number of lines while preserving identical behavior. Every line you can eliminate matters.

Techniques to apply:
- Arrow functions instead of function declarations
- Ternary expressions instead of if/else blocks
- Array methods (map, filter, reduce) instead of loops
- Destructuring and spread operators
- Comma operator to combine statements
- Template literals for string concatenation
- Short-circuit evaluation (&&, ||, ??)
- Remove all comments, JSDoc, and unnecessary type annotations
- Object shorthand properties
- Optional chaining and nullish coalescing
- Inferred types where possible (remove explicit annotations the compiler can infer)

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
    test_cmd: &str,
    timeout_secs: u32,
    cwd: &Path,
    file_rel_path: &str,
) -> Result<CompressResult, String> {
    let original_loc = counting::count_loc(code, lang).non_import;

    log("triage: asking LLM if code can be compressed");
    let triage_prompt = TRIAGE_PROMPT.replace("{code}", code);
    match call_claude(&triage_prompt) {
        Ok(answer) => {
            let trimmed = answer.trim().to_uppercase();
            if trimmed.starts_with("NO") {
                log("triage: LLM says code is already minimal");
                return Ok(CompressResult {
                    code: code.to_string(),
                    original_loc,
                    final_loc: original_loc,
                    rounds: 0,
                });
            }
            log("triage: LLM says code can be compressed, proceeding");
        }
        Err(e) => {
            log(&format!(
                "triage failed ({e}), proceeding with full pipeline"
            ));
        }
    }

    log("creating workspace (one-time project copy)");
    let workspace = testing::Workspace::new(lang, test_cmd, timeout_secs, cwd, file_rel_path)?;

    log("generating tests via claude -p");
    let tests = generate_tests(code, lang, file_rel_path)?;
    log(&format!(
        "generated tests ({} lines)",
        tests.lines().count()
    ));

    log("red check: tests should fail with empty module");
    let empty_module = "";
    let red = workspace.run_tests(empty_module, &tests);
    if red.passed {
        return Err(
            "generated tests pass with empty implementation (tests are trivial or broken)"
                .to_string(),
        );
    }
    log("red check passed (tests failed as expected)");

    log("green check: tests should pass with submitted code");
    let green = workspace.run_tests(code, &tests);
    if !green.passed {
        return Err(format!(
            "code does not pass generated tests:\n{}",
            green.output
        ));
    }
    log("green check passed");

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

        let result = workspace.run_tests(&compressed, &tests);
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
    clear_status();

    Ok(CompressResult {
        code: champion,
        original_loc,
        final_loc: champion_loc,
        rounds,
    })
}

fn log(msg: &str) {
    eprintln!("[min-loc] {msg}");
    let status_path = format!("{}/min-loc-stage", std::env::temp_dir().display());
    let _ = std::fs::write(&status_path, msg);
}

fn clear_status() {
    let status_path = format!("{}/min-loc-stage", std::env::temp_dir().display());
    let _ = std::fs::remove_file(&status_path);
}

fn module_path_from_file(file_rel_path: &str, lang: &str) -> String {
    match lang {
        "python" => file_rel_path
            .strip_suffix(".py")
            .unwrap_or(file_rel_path)
            .replace('/', "."),
        "javascript" | "typescript" => {
            let stem = std::path::Path::new(file_rel_path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            format!("./{stem}")
        }
        "java" => std::path::Path::new(file_rel_path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        "cpp" | "c" => testing::header_file_name(file_rel_path, lang),
        "go" => {
            let dir = std::path::Path::new(file_rel_path)
                .parent()
                .and_then(|p| p.file_name())
                .unwrap_or_default()
                .to_string_lossy();
            if dir.is_empty() {
                "main".to_string()
            } else {
                dir.to_string()
            }
        }
        _ => "solution".to_string(),
    }
}

fn generate_tests(code: &str, lang: &str, file_rel_path: &str) -> Result<String, String> {
    let template = match lang {
        "rust" => RUST_TESTS_PROMPT,
        "javascript" => JS_TESTS_PROMPT,
        "typescript" => TS_TESTS_PROMPT,
        "java" => JAVA_TESTS_PROMPT,
        "cpp" => CPP_TESTS_PROMPT,
        "c" => C_TESTS_PROMPT,
        "go" => GO_TESTS_PROMPT,
        _ => PYTHON_TESTS_PROMPT,
    };
    let module = module_path_from_file(file_rel_path, lang);
    let prompt = template
        .replace("{code}", code)
        .replace("{module}", &module);
    let output = call_claude(&prompt)?;
    Ok(extract_code_block(&output))
}

fn call_claude_compress(code: &str, lang: &str) -> Result<String, String> {
    let template = match lang {
        "rust" => RUST_COMPRESS_PROMPT,
        "javascript" => JS_COMPRESS_PROMPT,
        "typescript" => TS_COMPRESS_PROMPT,
        "java" => JAVA_COMPRESS_PROMPT,
        "cpp" => CPP_COMPRESS_PROMPT,
        "c" => C_COMPRESS_PROMPT,
        "go" => GO_COMPRESS_PROMPT,
        _ => PYTHON_COMPRESS_PROMPT,
    };
    let prompt = template.replace("{code}", code);
    let output = call_claude(&prompt)?;
    Ok(extract_code_block(&output))
}

fn call_claude(prompt: &str) -> Result<String, String> {
    let output = Command::new("claude")
        .arg("-p")
        .arg("--output-format")
        .arg("text")
        .arg("--settings")
        .arg(r#"{"alwaysThinkingEnabled": false}"#)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_path_python_dotted() {
        assert_eq!(
            module_path_from_file("src/utils/helpers.py", "python"),
            "src.utils.helpers"
        );
    }

    #[test]
    fn module_path_js_relative_stem() {
        assert_eq!(module_path_from_file("lib/math.js", "javascript"), "./math");
        assert_eq!(module_path_from_file("lib/math.ts", "typescript"), "./math");
    }

    #[test]
    fn module_path_java_class_name() {
        assert_eq!(
            module_path_from_file("src/main/java/com/x/Foo.java", "java"),
            "Foo"
        );
    }

    #[test]
    fn module_path_c_family_matches_workspace_header() {
        assert_eq!(module_path_from_file("src/shapes.cpp", "cpp"), "shapes.hpp");
        assert_eq!(module_path_from_file("mathutils.c", "c"), "mathutils.h");
    }

    #[test]
    fn module_path_go_package_from_dir() {
        assert_eq!(
            module_path_from_file("pkg/geometry/shapes.go", "go"),
            "geometry"
        );
        assert_eq!(module_path_from_file("main.go", "go"), "main");
    }

    #[test]
    fn extract_code_block_strips_fences() {
        assert_eq!(extract_code_block("```python\nx = 1\n```"), "x = 1");
        assert_eq!(extract_code_block("x = 1"), "x = 1");
        assert_eq!(extract_code_block("```\ny = 2\n```\n"), "y = 2");
    }
}
