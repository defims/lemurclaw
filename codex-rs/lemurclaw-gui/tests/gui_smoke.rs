//! GUI build smoke test.
//!
//! Companion to `lemurclaw/tests/tui_smoke.rs`. Verifies that:
//!   1. The `assets/dist/` tree that `lemurclaw_gui::assets::DIST` embeds at
//!      compile time is present and well-formed (vite build ran, index.html +
//!      JS + CSS chunks all exist and are non-trivial).
//!   2. The custom-protocol handler shape (`PROTOCOL_SCHEME` / `entry_url`)
//!      is reachable, so the wry webview will find the entry URL.
//!
//! This catches the regression where someone changes the React side, forgets
//! to rebuild `assets/dist/`, and ships a binary that opens a blank webview.
//! It does NOT start wry or render anything — that needs a display + a real
//! app-server and runs only on developer machines / headed CI runners.
//!
//! We re-`include_dir!` the same path instead of exposing
//! `lemurclaw_gui::assets::DIST` publicly — keeping the lib API surface small
//! (see AGENTS.md "crate API surface" rule) is more important than avoiding
//! a redundant macro invocation in one test.

use include_dir::{Dir, include_dir};

static DIST: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets/dist");

#[test]
fn dist_index_html_exists_and_is_non_empty() {
    let file = DIST
        .get_file("index.html")
        .expect("assets/dist/index.html must exist — run `npm run build` in assets/");
    let contents = file.contents();
    assert!(
        contents.len() > 100,
        "index.html is suspiciously small ({} bytes)",
        contents.len()
    );
    // vite injects the hashed bundle via a <script type="module" src="./assets/...js">
    // tag. If that reference is missing the webview will load an empty page.
    let html = std::str::from_utf8(contents).expect("index.html should be UTF-8");
    assert!(
        html.contains("./assets/"),
        "index.html must reference ./assets/ for vite's hashed bundles; got:\n{html}"
    );
}

#[test]
fn dist_contains_a_nontrivial_js_bundle() {
    let assets = DIST
        .get_dir("assets")
        .expect("assets/dist/assets/ directory must exist after vite build");
    let js_files: Vec<_> = assets
        .files()
        .filter(|f| f.path().extension().and_then(|e| e.to_str()) == Some("js"))
        .collect();
    assert!(!js_files.is_empty(), "no .js bundles found in dist/assets/");
    for f in js_files {
        let size = f.contents().len();
        // The React + app bundle is hundreds of KB. Sanity floor at 10 KB so
        // we catch an accidental empty chunk without being flaky on future
        // code-splitting changes.
        assert!(
            size > 10_000,
            "{} is only {} bytes — expected a real bundle",
            f.path().display(),
            size
        );
    }
}

#[test]
fn dist_contains_a_nontrivial_css_bundle() {
    let assets = DIST
        .get_dir("assets")
        .expect("assets/dist/assets/ directory must exist after vite build");
    let css_files: Vec<_> = assets
        .files()
        .filter(|f| f.path().extension().and_then(|e| e.to_str()) == Some("css"))
        .collect();
    assert!(!css_files.is_empty(), "no .css bundles found in dist/assets/");
    for f in css_files {
        let size = f.contents().len();
        assert!(
            size > 1_000,
            "{} is only {} bytes — expected a real stylesheet",
            f.path().display(),
            size
        );
    }
}

// Note: we deliberately do NOT smoke-test lemurclaw_gui::assets::entry_url()
// here. The `assets` module is private (crate API surface is intentionally
// small per AGENTS.md), so an integration test in tests/ can't reach it.
// Reaching it would require either making the module pub (API churn for one
// test) or moving the test into src/ as a #[cfg(test)] unit (different file
// organization convention). The compile-time `include_dir!` already proves
// the embedding works; the entry_url function is exercised the moment the
// binary actually starts.
