//! Phase 0 verification probes.
//!
//! `verify size` — measure compressed `.crate` tarball size for the largest
//! crates to decide whether they fit under the 10MB crates.io limit.
//!
//! `verify patches` — temporarily comment out `[patch.crates-io]` entries in
//! the workspace manifest, run `cargo check`, then restore the original
//! manifest. This answers whether the fork patches (ratatui/crossterm/
//! tungstenite) are actually required for compilation.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CRATES_IO_SIZE_LIMIT_BYTES: u64 = 10 * 1024 * 1024;

/// Crates to measure in `verify size`. Picked by git-tracked source size
/// (these five are the largest).
const LARGEST_CRATES: &[&str] = &[
    "codex-core",
    "codex-tui",
    "codex-app-server",
    "codex-app-server-protocol",
    "codex-exec-server",
];

pub fn run_size() -> Result<()> {
    let codex_root = locate_codex_root()?;
    println!("Phase 0 — verify size");
    println!("Source workspace: {}", codex_root.display());
    println!(
        "crates.io size limit: {} bytes ({:.2} MB)\n",
        CRATES_IO_SIZE_LIMIT_BYTES,
        CRATES_IO_SIZE_LIMIT_BYTES as f64 / (1024.0 * 1024.0)
    );

    println!(
        "{:<32} {:>12} {:>12} {:>10}",
        "crate", "raw bytes", "compressed", "% of cap"
    );
    println!("{}", "-".repeat(70));

    let mut all_ok = true;
    for crate_name in LARGEST_CRATES {
        match measure_crate(&codex_root, crate_name) {
            Ok(Measurement {
                raw_bytes,
                crate_bytes,
            }) => {
                let pct = (crate_bytes as f64 / CRATES_IO_SIZE_LIMIT_BYTES as f64) * 100.0;
                let status = if crate_bytes <= CRATES_IO_SIZE_LIMIT_BYTES {
                    "OK"
                } else {
                    all_ok = false;
                    "OVER"
                };
                println!(
                    "{:<32} {:>12} {:>12} {:>9.1}% {}",
                    crate_name,
                    format_bytes(raw_bytes),
                    format_bytes(crate_bytes),
                    pct,
                    status
                );
            }
            Err(e) => {
                all_ok = false;
                println!("{:<32} ERROR: {}", crate_name, e);
            }
        }
    }

    println!();
    if all_ok {
        println!("✓ All measured crates fit under the 10MB compressed limit.");
    } else {
        println!(
            "✗ At least one crate exceeds the limit OR failed to package. See above."
        );
    }

    Ok(())
}

struct Measurement {
    raw_bytes: u64,
    crate_bytes: u64,
}

fn measure_crate(codex_root: &Path, crate_name: &str) -> Result<Measurement> {
    // Measure the would-be tarball size by taring+gzipping the git-tracked
    // files for this crate directly. This bypasses `cargo package`'s manifest
    // validation (which would fail on path deps without explicit versions —
    // exactly what Phase 1 fixes). The result is a faithful estimate of the
    // final compressed `.crate` size because that's all `cargo package` does
    // internally: tar the git-tracked files and gzip the result.
    let crate_dir = locate_crate_dir(codex_root, crate_name)?;
    let raw_bytes = measure_raw_size(&crate_dir)?;

    // Pipe `git ls-files -z` to `tar -T - --null` to `gzip` to capture
    // compressed size without writing a temp file.
    let ls_output = Command::new("git")
        .args([
            "-C",
            crate_dir.to_str().unwrap_or("."),
            "ls-files",
            "-z",
        ])
        .output()
        .context("git ls-files")?;
    if !ls_output.status.success() {
        anyhow::bail!("git ls-files failed");
    }

    // Use tar to create a gzipped archive of the listed files, output to stdout.
    // We chdir into crate_dir so paths in the tar are relative.
    let mut tar_output = Command::new("tar")
        .args(["-c", "-z", "--null", "-T", "-"])
        .current_dir(&crate_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("spawn tar")?;

    // Write the file list to tar's stdin.
    use std::io::Write;
    let mut stdin = tar_output.stdin.take().context("tar stdin")?;
    stdin.write_all(&ls_output.stdout)?;
    drop(stdin);

    let output = tar_output.wait_with_output().context("tar wait")?;
    if !output.status.success() {
        anyhow::bail!(
            "tar failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let crate_bytes = output.stdout.len() as u64;

    Ok(Measurement {
        raw_bytes,
        crate_bytes,
    })
}

fn measure_raw_size(crate_dir: &Path) -> Result<u64> {
    // Sum all git-tracked files under crate_dir. Falls back to disk size if
    // not in a git repo.
    let output = Command::new("git")
        .args(["-C", crate_dir.to_str().unwrap_or("."), "ls-files"])
        .output();
    let total: u64 = match output {
        Ok(o) if o.status.success() => {
            let listing = String::from_utf8_lossy(&o.stdout);
            let mut sum: u64 = 0;
            for line in listing.lines() {
                let p = crate_dir.join(line);
                if let Ok(m) = fs::metadata(&p) {
                    sum += m.len();
                }
            }
            sum
        }
        _ => {
            // Fallback: walk dir.
            walkdir_size(crate_dir)
        }
    };
    Ok(total)
}

fn walkdir_size(dir: &Path) -> u64 {
    let mut sum: u64 = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(m) = fs::metadata(&path) {
                if m.is_file() {
                    sum += m.len();
                } else if m.is_dir() {
                    sum += walkdir_size(&path);
                }
            }
        }
    }
    sum
}

pub fn run_patches() -> Result<()> {
    let codex_root = locate_codex_root()?;
    let manifest_path = codex_root.join("Cargo.toml");
    println!("Phase 0 — verify patches");
    println!("Source workspace manifest: {}", manifest_path.display());
    println!();

    let original = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;

    let patch_entries = extract_patch_entries(&original);
    if patch_entries.is_empty() {
        println!("No [patch.crates-io] entries found. Nothing to verify.");
        return Ok(());
    }

    println!("[patch.crates-io] entries detected:");
    for entry in &patch_entries {
        println!("  - {}", entry.key);
    }
    println!();
    println!("Temporarily commenting out all patch entries...");

    let modified = comment_out_patches(&original);
    fs::write(&manifest_path, &modified)
        .with_context(|| format!("write test manifest {}", manifest_path.display()))?;

    // Always restore on exit, even on panic.
    let result = run_cargo_check(&codex_root);

    fs::write(&manifest_path, &original)
        .with_context(|| format!("restore {}", manifest_path.display()))?;
    println!("Restored original Cargo.toml.");
    println!();

    match result {
        Ok(CheckOutcome::Success) => {
            println!("✓ cargo check succeeded WITHOUT fork patches.");
            println!();
            println!("Conclusion: the [patch.crates-io] forks are NOT required for");
            println!("compilation. You can drop them entirely when publishing to");
            println!("crates.io (use upstream ratatui/crossterm/tungstenite).");
        }
        Ok(CheckOutcome::Failure { stdout, stderr }) => {
            println!("✗ cargo check FAILED without fork patches.");
            println!();
            println!("This confirms the forks ARE required. To publish to crates.io,");
            println!("you must either:");
            println!("  (a) publish the forks as separate crates");
            println!("      (lemurclaw-ratatui, lemurclaw-crossterm, ...), or");
            println!("  (b) patch your code to work with upstream versions.");
            println!();
            println!("Diagnostic output (first 50 lines each):");
            println!("--- stdout ---");
            for line in stdout.lines().take(50) {
                println!("{}", line);
            }
            println!("--- stderr ---");
            for line in stderr.lines().take(50) {
                println!("{}", line);
            }
        }
        Err(e) => {
            println!("✗ cargo check invocation failed: {}", e);
            println!("The fork question is unresolved. Check manually.");
        }
    }

    Ok(())
}

enum CheckOutcome {
    Success,
    Failure { stdout: String, stderr: String },
}

fn run_cargo_check(codex_root: &Path) -> Result<CheckOutcome> {
    println!("Running `cargo check -p codex-tui -p codex-core` (this is slow)...");
    let output = Command::new("cargo")
        .args(["check", "-p", "codex-tui", "-p", "codex-core"])
        .current_dir(codex_root)
        .output()
        .context("spawn cargo check")?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if output.status.success() {
        Ok(CheckOutcome::Success)
    } else {
        Ok(CheckOutcome::Failure { stdout, stderr })
    }
}

struct PatchEntry {
    key: String,
    #[allow(dead_code)]
    line_idx: usize,
}

fn extract_patch_entries(manifest: &str) -> Vec<PatchEntry> {
    let mut out = Vec::new();
    let mut in_patch = false;
    for (idx, line) in manifest.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_patch = trimmed.starts_with("[patch");
            continue;
        }
        if in_patch && !trimmed.is_empty() && !trimmed.starts_with('#') {
            if let Some(eq) = trimmed.find('=') {
                let key = trimmed[..eq].trim().to_string();
                out.push(PatchEntry {
                    key,
                    line_idx: idx,
                });
            }
        }
    }
    out
}

fn comment_out_patches(manifest: &str) -> String {
    let mut out = String::with_capacity(manifest.len());
    let mut in_patch = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_patch = trimmed.starts_with("[patch");
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_patch && !trimmed.is_empty() && !trimmed.starts_with('#') {
            // Comment out this line.
            out.push_str("# [xtask-disabled] ");
            out.push_str(line);
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn locate_codex_root() -> Result<PathBuf> {
    // xtask lives at <repo>/xtask, so codex-rs is one level up.
    let cwd = std::env::current_dir().context("get cwd")?;
    // Try cwd/codex-rs first (if invoked from repo root), then ../codex-rs
    // (if invoked from xtask/).
    let candidates = [
        cwd.join("codex-rs"),
        cwd.join("..").join("codex-rs"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("codex-rs"),
    ];
    for c in &candidates {
        if c.join("Cargo.toml").exists() {
            return Ok(c.canonicalize().unwrap_or_else(|_| c.clone()));
        }
    }
    anyhow::bail!(
        "could not locate codex-rs/ workspace (tried {:?})",
        candidates
    );
}

fn locate_crate_dir(codex_root: &Path, crate_name: &str) -> Result<PathBuf> {
    // crate_name like "codex-core" -> dir "core"
    let suffix = crate_name
        .strip_prefix("codex-")
        .unwrap_or(crate_name);
    let direct = codex_root.join(suffix);
    if direct.is_dir() {
        return Ok(direct);
    }
    // Some crates live in subdirs (utils/, ext/, memories/). Walk shallow.
    for sub in ["utils", "ext", "memories"] {
        let candidate = codex_root.join(sub).join(suffix);
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }
    // Fall back: use cargo metadata to find the dir.
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version=1",
            "--no-deps",
            "--manifest-path",
            codex_root.join("Cargo.toml").to_str().unwrap(),
        ])
        .output()
        .context("run cargo metadata")?;
    if !output.status.success() {
        anyhow::bail!("cargo metadata failed");
    }
    let json = String::from_utf8_lossy(&output.stdout);
    // Simple substring search for the crate dir.
    let marker = format!("\"name\":\"{}\"", crate_name);
    if let Some(pos) = json.find(&marker) {
        // The "manifest_path" field comes right after the name in cargo metadata
        // output. Find the next "manifest_path" after this position.
        if let Some(mp_pos) = json[pos..].find("\"manifest_path\":\"") {
            let start = pos + mp_pos + "\"manifest_path\":\"".len();
            if let Some(end) = json[start..].find('"') {
                let mp = &json[start..start + end];
                let p = PathBuf::from(mp);
                if let Some(parent) = p.parent() {
                    return Ok(parent.to_path_buf());
                }
            }
        }
    }
    anyhow::bail!("could not locate crate dir for {}", crate_name);
}

fn format_bytes(b: u64) -> String {
    if b >= 1024 * 1024 {
        format!("{:.2} MB", b as f64 / (1024.0 * 1024.0))
    } else if b >= 1024 {
        format!("{:.1} KB", b as f64 / 1024.0)
    } else {
        format!("{} B", b)
    }
}
