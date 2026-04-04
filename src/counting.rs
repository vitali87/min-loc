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
            line.starts_with("import ") || line.starts_with("const ") && line.contains("require(")
        }
        "go" => line.starts_with("import "),
        _ => false,
    }
}
