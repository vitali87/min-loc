use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use wait_timeout::ChildExt;

pub struct TestResult {
    pub passed: bool,
    pub output: String,
}

pub struct Workspace {
    _tmpdir: tempfile::TempDir,
    root: PathBuf,
    file_rel_path: String,
    lang: String,
    test_cmd: String,
    timeout_secs: u32,
}

impl Workspace {
    pub fn new(
        lang: &str,
        test_cmd: &str,
        timeout_secs: u32,
        cwd: &Path,
        file_rel_path: &str,
    ) -> Result<Self, String> {
        let tmpdir = tempfile::tempdir().map_err(|e| format!("failed to create temp dir: {e}"))?;
        let (excludes, symlinks): (&[&str], &[&str]) = match lang {
            "rust" => (&["target"], &[]),
            "python" => (&["__pycache__", "*.pyc", ".venv"], &[".venv"]),
            "javascript" | "typescript" => (&["node_modules"], &["node_modules"]),
            "java" => (&["target", "build", ".gradle"], &[]),
            "cpp" | "c" => (&["build", "cmake-build-*"], &[]),
            _ => (&[], &[]),
        };
        copy_project(cwd, tmpdir.path(), excludes, symlinks)?;
        let root = tmpdir.path().to_path_buf();
        Ok(Self {
            _tmpdir: tmpdir,
            root,
            file_rel_path: file_rel_path.to_string(),
            lang: lang.to_string(),
            test_cmd: test_cmd.to_string(),
            timeout_secs,
        })
    }

    pub fn run_tests(&self, code: &str, test_code: &str) -> TestResult {
        match self.lang.as_str() {
            "rust" => self.run_rust(code, test_code),
            "cpp" | "c" => self.run_cpp(code, test_code),
            "javascript" | "typescript" => self.run_js(code, test_code),
            "java" => self.run_java(code, test_code),
            "go" => self.run_go(code, test_code),
            _ => self.run_python(code, test_code),
        }
    }

    fn run_rust(&self, code: &str, test_code: &str) -> TestResult {
        let combined =
            format!("{code}\n\n#[cfg(test)]\nmod tests {{\n    use super::*;\n{test_code}\n}}");
        if self
            .write_file(Path::new(&self.file_rel_path), &combined)
            .is_none()
        {
            return fail("failed to write source");
        }
        self.run_test_cmd(&[], None)
    }

    fn run_python(&self, code: &str, test_code: &str) -> TestResult {
        if self
            .write_file(Path::new(&self.file_rel_path), code)
            .is_none()
        {
            return fail("failed to write source");
        }
        let rel = Path::new(&self.file_rel_path);
        let stem = rel.file_stem().unwrap_or_default().to_string_lossy();
        let test_rel = rel
            .parent()
            .unwrap_or(Path::new(""))
            .join(format!("test_{stem}.py"));
        let Some(test_path) = self.write_file(&test_rel, test_code) else {
            return fail("failed to write tests");
        };
        let test_path_str = test_path.to_string_lossy().to_string();
        self.run_test_cmd(
            &[&test_path_str, "--tb=short", "-q"],
            Some(("PYTHONPATH", self.root.to_str().unwrap_or(""))),
        )
    }

    fn run_js(&self, code: &str, test_code: &str) -> TestResult {
        if self
            .write_file(Path::new(&self.file_rel_path), code)
            .is_none()
        {
            return fail("failed to write source");
        }
        let ext = match self.lang.as_str() {
            "typescript" => "ts",
            _ => "js",
        };
        let rel = Path::new(&self.file_rel_path);
        let stem = rel.file_stem().unwrap_or_default().to_string_lossy();
        let test_rel = rel
            .parent()
            .unwrap_or(Path::new(""))
            .join(format!("{stem}.test.{ext}"));
        if self.write_file(&test_rel, test_code).is_none() {
            return fail("failed to write tests");
        }
        self.run_test_cmd(&["--no-cache"], None)
    }

    fn run_java(&self, code: &str, test_code: &str) -> TestResult {
        if self
            .write_file(Path::new(&self.file_rel_path), code)
            .is_none()
        {
            return fail("failed to write source");
        }
        let test_rel = java_test_rel_path(&self.file_rel_path);
        if self.write_file(Path::new(&test_rel), test_code).is_none() {
            return fail("failed to write tests");
        }
        self.run_test_cmd(&[], None)
    }

    fn run_go(&self, code: &str, test_code: &str) -> TestResult {
        let Some(target) = self.write_file(Path::new(&self.file_rel_path), code) else {
            return fail("failed to write source");
        };
        let pkg = code
            .lines()
            .find(|l| l.trim_start().starts_with("package "))
            .unwrap_or("package main");
        let combined = format!("{pkg}\n\n{test_code}");
        let rel = Path::new(&self.file_rel_path);
        let stem = rel.file_stem().unwrap_or_default().to_string_lossy();
        let test_rel = rel
            .parent()
            .unwrap_or(Path::new(""))
            .join(format!("{stem}_test.go"));
        if self.write_file(&test_rel, &combined).is_none() {
            return fail("failed to write tests");
        }
        let test_dir = target.parent().unwrap_or(&self.root);
        let test_dir_str = test_dir.to_string_lossy().to_string();
        self.run_test_cmd(&[&test_dir_str], None)
    }

    fn run_cpp(&self, code: &str, test_code: &str) -> TestResult {
        let rel = Path::new(&self.file_rel_path);
        let dir = rel.parent().unwrap_or(Path::new(""));
        let header_rel = dir.join(header_file_name(&self.file_rel_path, &self.lang));
        if self.write_file(&header_rel, code).is_none() {
            return fail("failed to write source header");
        }
        let stem = rel.file_stem().unwrap_or_default().to_string_lossy();
        let test_ext = if self.lang == "c" { "c" } else { "cpp" };
        let test_rel = dir.join(format!("{stem}_test.{test_ext}"));
        let Some(test_path) = self.write_file(&test_rel, test_code) else {
            return fail("failed to write tests");
        };
        let binary = self.root.join(format!("{stem}_test_runner"));
        let binary_str = binary.to_string_lossy().to_string();
        let test_path_str = test_path.to_string_lossy().to_string();

        let parts: Vec<&str> = self.test_cmd.split_whitespace().collect();
        if parts.is_empty() {
            return fail("empty test command");
        }
        let compile_result = run_command(
            parts[0],
            &[parts[1..].to_vec(), vec!["-o", &binary_str, &test_path_str]].concat(),
            None,
            self.timeout_secs,
            &self.root,
        );
        if !compile_result.passed {
            return fail(&format!("compilation failed: {}", compile_result.output));
        }
        run_command(&binary_str, &[], None, self.timeout_secs, &self.root)
    }

    fn write_file(&self, rel_path: &Path, contents: &str) -> Option<PathBuf> {
        let target = self.root.join(rel_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&target, contents).ok()?;
        Some(target)
    }

    fn run_test_cmd(&self, extra_args: &[&str], env_var: Option<(&str, &str)>) -> TestResult {
        let parts: Vec<&str> = self.test_cmd.split_whitespace().collect();
        if parts.is_empty() {
            return fail("empty test command");
        }
        run_command(
            parts[0],
            &[parts[1..].to_vec(), extra_args.to_vec()].concat(),
            env_var,
            self.timeout_secs,
            &self.root,
        )
    }
}

pub fn java_test_rel_path(file_rel_path: &str) -> String {
    let path = Path::new(file_rel_path);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let test_name = format!("{stem}Test.java");
    let dir = path
        .parent()
        .unwrap_or(Path::new(""))
        .to_string_lossy()
        .to_string();
    let test_dir = match dir.strip_prefix("src/main/java") {
        Some(rest) => format!("src/test/java{rest}"),
        None => dir,
    };
    if test_dir.is_empty() {
        test_name
    } else {
        format!("{test_dir}/{test_name}")
    }
}

pub fn header_file_name(file_rel_path: &str, lang: &str) -> String {
    let stem = Path::new(file_rel_path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let ext = if lang == "c" { "h" } else { "hpp" };
    format!("{stem}.{ext}")
}

fn fail(msg: &str) -> TestResult {
    TestResult {
        passed: false,
        output: msg.to_string(),
    }
}

fn copy_project(
    src: &Path,
    dst: &Path,
    excludes: &[&str],
    symlinks: &[&str],
) -> Result<(), String> {
    let mut cmd = Command::new("rsync");
    cmd.arg("-a").arg("--quiet");
    cmd.arg("--exclude").arg(".git");
    for exc in excludes {
        cmd.arg("--exclude").arg(*exc);
    }
    let src_str = format!("{}/", src.display());
    let dst_str = format!("{}/", dst.display());
    cmd.arg(&src_str).arg(&dst_str);
    let output = cmd.output().map_err(|e| format!("rsync failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "rsync: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    for dir in symlinks {
        let src_dir = src.join(dir);
        let dst_dir = dst.join(dir);
        if src_dir.exists() && !dst_dir.exists() {
            std::os::unix::fs::symlink(&src_dir, &dst_dir).ok();
        }
    }
    Ok(())
}

fn run_command(
    program: &str,
    args: &[&str],
    env_var: Option<(&str, &str)>,
    timeout_secs: u32,
    cwd: &Path,
) -> TestResult {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    if let Some((key, val)) = env_var {
        cmd.env(key, val);
    }

    let mut child = match cmd.spawn() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_test_path_mirrors_maven_layout() {
        assert_eq!(
            java_test_rel_path("src/main/java/com/x/Foo.java"),
            "src/test/java/com/x/FooTest.java"
        );
        assert_eq!(
            java_test_rel_path("src/main/java/Foo.java"),
            "src/test/java/FooTest.java"
        );
    }

    #[test]
    fn java_test_path_sits_next_to_non_maven_source() {
        assert_eq!(java_test_rel_path("Foo.java"), "FooTest.java");
        assert_eq!(java_test_rel_path("lib/Foo.java"), "lib/FooTest.java");
    }

    #[test]
    fn header_name_matches_lang() {
        assert_eq!(
            header_file_name("src/geometry/shapes.cpp", "cpp"),
            "shapes.hpp"
        );
        assert_eq!(header_file_name("mathutils.c", "c"), "mathutils.h");
        assert_eq!(header_file_name("include/vec.hpp", "cpp"), "vec.hpp");
    }
}
