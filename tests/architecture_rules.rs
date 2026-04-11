//! Architecture compliance tests.
//!
//! Enforce module isolation rules so dependencies flow in one
//! direction as the codebase grows. Run with:
//!   cargo test --test architecture_rules

use std::fs;
use std::path::{Path, PathBuf};

/// Recursively collect all `.rs` files under a directory.
fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return files;
    }
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(collect_rs_files(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    files
}

/// Check that no file in `module_dir` contains a `use crate::<forbidden>`
/// import. Returns a list of violations.
fn check_forbidden_imports(module_dir: &Path, forbidden: &[&str]) -> Vec<String> {
    let mut violations = Vec::new();
    for file in collect_rs_files(module_dir) {
        let content = fs::read_to_string(&file).unwrap();
        for (line_no, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            // Skip comments
            if trimmed.starts_with("//") {
                continue;
            }
            for &module in forbidden {
                let pattern = format!("crate::{module}");
                if trimmed.contains(&pattern) {
                    violations.push(format!(
                        "{}:{}: `{}` imports from `{}`\n    {}",
                        file.display(),
                        line_no + 1,
                        module_dir.file_name().unwrap().to_str().unwrap(),
                        module,
                        trimmed,
                    ));
                }
            }
        }
    }
    violations
}

/// CRD types must not depend on controller logic (when it exists).
#[test]
fn crd_does_not_import_controller() {
    let crd_dir = Path::new("src/crd");
    if !crd_dir.exists() {
        return;
    }
    let violations = check_forbidden_imports(crd_dir, &["controller", "reconciler"]);
    assert!(
        violations.is_empty(),
        "crd module must not import controller logic:\n{}",
        violations.join("\n")
    );
}
