//! Local model directory inspection, staging promotion, and GGUF matching.

use std::path::{Path, PathBuf};

/// Delete files `hf-hub` would mistake for a finished download. It creates the
/// destination before the first byte, and later skips any file that merely
/// exists, so a transfer that died at zero bytes would be on disk forever.
pub(super) fn discard_empty(dir: &Path) {
    files(dir)
        .into_iter()
        .filter(|path| path.metadata().is_ok_and(|meta| meta.len() == 0))
        .for_each(|path| {
            let _ = std::fs::remove_file(path);
        });
}

/// Move completed files from `staging` into `dir` and remove `staging`.
pub(super) fn promote(staging: &Path, dir: &Path) -> std::io::Result<()> {
    for src in files(staging) {
        let Ok(rel) = src.strip_prefix(staging) else {
            continue;
        };
        let target = dir.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&src, &target)?;
    }
    let _ = std::fs::remove_dir_all(staging);
    Ok(())
}

/// Whether `dir` holds a model of this quant, at any depth.
pub(super) fn holds_a_model(dir: &Path, quant: &str) -> bool {
    let token = quant.to_lowercase();
    files(dir).iter().any(|path| carries_quant(path, &token))
}

fn is_quant_gguf(path: &Path, token: &str) -> bool {
    let Some(name) = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
    else {
        return false;
    };
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
        && !name.contains("mmproj")
        && !name.contains("imatrix")
        && name.contains(token)
}

pub(super) fn carries_quant(path: &Path, token: &str) -> bool {
    is_quant_gguf(path, token) && path.metadata().is_ok_and(|meta| meta.len() > 0)
}

/// The repository paths carrying this quant. Taking every one of them is what
/// makes a split model arrive whole: its shards all match, so all are fetched.
pub(super) fn quant_files(paths: &[String], quant: &str) -> Vec<String> {
    let token = quant.to_lowercase();
    paths
        .iter()
        .filter(|path| is_quant_gguf(Path::new(path), &token))
        .cloned()
        .collect()
}

/// Every file under `dir`, at any depth. Directory links are not followed, so a
/// loop in the folder cannot overflow the stack; hidden staging folders and
/// `.incomplete` transfers are skipped.
pub(super) fn files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') || name_str.ends_with(".incomplete") {
            continue;
        }
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            found.extend(files(&path));
        } else {
            found.push(path);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::{discard_empty, holds_a_model, promote, quant_files};

    #[test]
    fn every_file_of_the_quant_is_fetched_so_a_split_arrives_whole() {
        let listing = [
            "Qwen3-8B-Q4_K_M.gguf".to_string(),
            "Qwen3-8B-Q6_K.gguf".to_string(),
            "mmproj-Qwen3-8B-Q4_K_M.gguf".to_string(),
            "Qwen3-8B-Q4_K_M-00001-of-00002.gguf".to_string(),
            "Qwen3-8B-Q4_K_M-00002-of-00002.gguf".to_string(),
            "README.md".to_string(),
        ];

        let mut chosen = quant_files(&listing, "Q4_K_M");
        chosen.sort();

        assert_eq!(
            chosen,
            vec![
                "Qwen3-8B-Q4_K_M-00001-of-00002.gguf",
                "Qwen3-8B-Q4_K_M-00002-of-00002.gguf",
                "Qwen3-8B-Q4_K_M.gguf",
            ]
        );
    }

    #[test]
    fn a_folder_holding_the_model_of_this_quant_counts_as_downloaded() {
        let dir = tempfile::TempDir::new().unwrap();
        let nested = dir.path().join("gguf");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("Qwen3-8B-Q4_K_M.gguf"), [0; 4]).unwrap();

        assert!(holds_a_model(dir.path(), "Q4_K_M"));
        assert!(!holds_a_model(dir.path(), "Q2_K"));
    }

    #[test]
    fn an_empty_file_is_not_a_downloaded_model() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Qwen3-8B-Q4_K_M.gguf"), []).unwrap();

        assert!(
            !holds_a_model(dir.path(), "Q4_K_M"),
            "a file cut short by an interrupted write"
        );
    }

    #[test]
    fn a_staged_or_incomplete_file_is_not_a_downloaded_model() {
        let dir = tempfile::TempDir::new().unwrap();
        let staging = dir.path().join(".staging");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("Qwen3-8B-Q4_K_M.gguf"), [1; 16]).unwrap();
        std::fs::write(dir.path().join("Qwen3-8B-Q4_K_M.gguf.incomplete"), [1; 16]).unwrap();

        assert!(!holds_a_model(dir.path(), "Q4_K_M"));

        promote(&staging, dir.path()).unwrap();
        assert!(holds_a_model(dir.path(), "Q4_K_M"));
        assert!(!staging.exists());
    }

    #[test]
    fn a_transfer_that_died_at_zero_bytes_is_not_left_to_be_skipped() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Qwen3-8B-Q4_K_M.gguf"), []).unwrap();
        std::fs::write(dir.path().join("config.json"), [0; 8]).unwrap();

        discard_empty(dir.path());

        assert!(
            !dir.path().join("Qwen3-8B-Q4_K_M.gguf").exists(),
            "hf-hub skips any file that exists, so this one is never fetched"
        );
        assert!(dir.path().join("config.json").exists());
    }
}
