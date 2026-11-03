//! Embed the git commit ingot is built from as `INGOT_GIT_COMMIT`, suffixed `-dirty` when
//! tracked files differ from it, or `unknown` outside a git checkout. Contract mode stamps it
//! into the tables and refuses a dirty or unknown build.

use std::process::Command;

/// Output of `git <args>`, trimmed, or `None` when git is missing or fails.
///
/// @version 1
/// @req REQ-ING-TIV-013
fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Emit the commit and the paths whose change must re-run this script.
///
/// @version 1
/// @req REQ-ING-TIV-013
fn main() {
    for path in [
        ".git/HEAD",
        ".git/index",
        ".git/refs",
        "src",
        "templates",
        "Cargo.toml",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let commit = match git(&["rev-parse", "HEAD"]) {
        Some(head) => {
            let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
                .is_none_or(|status| !status.is_empty());
            if dirty {
                format!("{head}-dirty")
            } else {
                head
            }
        }
        None => "unknown".to_string(),
    };
    println!("cargo:rustc-env=INGOT_GIT_COMMIT={commit}");
}
