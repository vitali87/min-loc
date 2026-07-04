use crate::types::LOCStats;

pub fn count_loc(code: &str, lang: &str) -> LOCStats {
    let lines: Vec<&str> = code.lines().collect();
    let total = lines.len() as u32;
    let non_blank = lines.iter().filter(|l| !l.trim().is_empty()).count() as u32;
    let non_import = lines
        .iter()
        .filter(|l| {
            let trimmed = l.trim();
            !trimmed.is_empty() && !is_import_line(trimmed, lang)
        })
        .count() as u32;
    LOCStats {
        total,
        non_blank,
        non_import,
    }
}

fn is_import_line(line: &str, lang: &str) -> bool {
    match lang {
        "python" => line.starts_with("import ") || line.starts_with("from "),
        "rust" => line.starts_with("use "),
        "javascript" | "typescript" => {
            line.starts_with("import ") || (line.starts_with("const ") && line.contains("require("))
        }
        "go" => line.starts_with("import "),
        "java" => line.starts_with("import "),
        "cpp" | "c" => line.starts_with("#include"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_python_lines_and_imports() {
        let code = "import os\nfrom sys import argv\n\ndef f():\n    return 1\n";
        let stats = count_loc(code, "python");
        assert_eq!(stats.total, 5);
        assert_eq!(stats.non_blank, 4);
        assert_eq!(stats.non_import, 2);
    }

    #[test]
    fn counts_java_imports() {
        let code = "import java.util.List;\nclass A {}\n";
        assert_eq!(count_loc(code, "java").non_import, 1);
    }

    #[test]
    fn counts_c_family_includes() {
        let code = "#include <stdio.h>\nint main() { return 0; }\n";
        assert_eq!(count_loc(code, "c").non_import, 1);
        assert_eq!(count_loc(code, "cpp").non_import, 1);
    }

    #[test]
    fn counts_js_imports_and_requires() {
        let code = "import x from 'x';\nconst y = require('y');\nconst z = 1;\n";
        assert_eq!(count_loc(code, "javascript").non_import, 1);
    }
}
