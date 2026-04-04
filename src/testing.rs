use std::path::Path;
use std::process::Command;
use std::time::Duration;

use wait_timeout::ChildExt;

pub struct TestResult {
    pub passed: bool,
    pub output: String,
}

pub fn run_tests(
    code: &str,
    test_code: &str,
    module_name: &str,
    test_cmd: &str,
    timeout_secs: u32,
    cwd: &Path,
) -> TestResult {
    let tmpdir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            return TestResult {
                passed: false,
                output: format!("failed to create temp dir: {e}"),
            };
        }
    };

    let solution_path = tmpdir.path().join(format!("{module_name}.py"));
    if std::fs::write(&solution_path, code).is_err() {
        return TestResult {
            passed: false,
            output: "failed to write solution to temp dir".to_string(),
        };
    }

    let test_path = tmpdir.path().join("test_solution.py");
    if std::fs::write(&test_path, test_code).is_err() {
        return TestResult {
            passed: false,
            output: "failed to write tests to temp dir".to_string(),
        };
    }

    let parts: Vec<&str> = test_cmd.split_whitespace().collect();
    if parts.is_empty() {
        return TestResult {
            passed: false,
            output: "empty test command".to_string(),
        };
    }

    let test_path_str = test_path.to_string_lossy();
    let mut child = match Command::new(parts[0])
        .args(&parts[1..])
        .arg(&*test_path_str)
        .arg("--tb=short")
        .arg("-q")
        .env("PYTHONPATH", tmpdir.path())
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return TestResult {
                passed: false,
                output: format!("failed to spawn test command: {e}"),
            };
        }
    };

    let timeout = Duration::from_secs(timeout_secs.into());
    match child.wait_timeout(timeout) {
        Ok(Some(status)) => {
            let stdout = read_pipe(&mut child.stdout);
            let stderr = read_pipe(&mut child.stderr);
            let combined = if stderr.is_empty() {
                stdout
            } else {
                format!("{stdout}\n{stderr}")
            };
            TestResult {
                passed: status.success(),
                output: truncate(&combined, 500),
            }
        }
        Ok(None) => {
            let _ = child.kill();
            TestResult {
                passed: false,
                output: "test timed out".to_string(),
            }
        }
        Err(e) => TestResult {
            passed: false,
            output: format!("failed to wait for test: {e}"),
        },
    }
}

fn read_pipe<R: std::io::Read>(pipe: &mut Option<R>) -> String {
    pipe.take()
        .map(|mut s| {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut s, &mut buf).ok();
            buf
        })
        .unwrap_or_default()
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...[truncated]", &s[..max_len])
    }
}
