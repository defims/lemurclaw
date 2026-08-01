// Build script for lemurclaw-webui: builds the shared React frontend under
// `assets/` into `assets/dist/` when Node is available, so the bundled
// webview (gui) and webui server have a UI to serve. Emits
// `cargo:rerun-if-changed` so source edits trigger a rebuild.
//
// The build is best-effort: if Node is missing or `npm` fails, we emit a
// `cargo:warning` and let the crate still compile. Both frontends will then
// serve a missing page (visible only at runtime), which is acceptable for
// `cargo check`/`cargo build` on machines without Node installed. CI and
// release builds are expected to have Node.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // CARGO_MANIFEST_DIR is always set by Cargo when compiling this crate's
    // build script; it points at `lemurclaw-webui/`. If it's somehow missing
    // (shouldn't happen under cargo), emit a warning and bail rather than
    // panic, since build scripts that abort fail the whole build.
    let manifest_dir = match env::var("CARGO_MANIFEST_DIR") {
        Ok(d) => d,
        Err(_) => {
            println!(
                "cargo:warning=lemurclaw-webui: CARGO_MANIFEST_DIR not set; skipping frontend build"
            );
            return;
        }
    };
    let manifest = PathBuf::from(manifest_dir);
    let assets = manifest.join("assets");
    let dist = assets.join("dist");

    println!(
        "cargo:rerun-if-changed={}",
        assets.join("package.json").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        assets.join("index.html").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        assets.join("vite.config.ts").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        assets.join("tsconfig.json").display()
    );
    // Source tree (src/ + types/) — coarse-grained: any change under it reruns.
    println!("cargo:rerun-if-changed={}", assets.join("src").display());

    if dist.join("index.html").exists() {
        // Already built (e.g. checked-in dist or prior local build); don't
        // rebuild on every cargo invocation.
        return;
    }

    // Detect Node via `npm --version`. If npm is unavailable, warn and bail.
    let npm_version = Command::new("npm").arg("--version").output();
    let npm_available = match &npm_version {
        Ok(out) => out.status.success(),
        Err(_) => false,
    };
    if !npm_available {
        println!(
            "cargo:warning=lemurclaw-webui: npm not found; skipping frontend build (assets/dist missing, UI will be blank)"
        );
        return;
    }

    let install_ok = Command::new("npm")
        .args(["install"])
        .current_dir(&assets)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !install_ok {
        println!(
            "cargo:warning=lemurclaw-webui: `npm install` failed in assets/; frontend not built (UI will be blank)"
        );
        return;
    }

    let build_ok = Command::new("npm")
        .args(["run", "build"])
        .current_dir(&assets)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !build_ok {
        println!(
            "cargo:warning=lemurclaw-webui: `npm run build` failed in assets/; frontend not built (UI will be blank)"
        );
        return;
    }

    // Sanity-check the build actually produced the entry HTML.
    if !dist.join("index.html").exists() {
        println!(
            "cargo:warning=lemurclaw-webui: `npm run build` succeeded but assets/dist/index.html is missing"
        );
    }
}
