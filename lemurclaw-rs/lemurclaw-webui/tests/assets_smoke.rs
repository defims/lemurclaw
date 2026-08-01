//! Shared frontend build smoke test.
//!
//! Verifies that the `assets/dist/` tree embedded at compile time by
//! `lemurclaw_webui::assets` is present and well-formed: vite build ran,
//! `index.html` + JS + CSS chunks all exist and are non-trivial.
//!
//! This catches the regression where someone changes the React side, forgets
//! to rebuild `assets/dist/`, and ships a binary that opens a blank webview
//! (gui) or serves a blank page (webui). It does NOT start any server or
//! render anything — that needs a display + a real app-server and runs only
//! on developer machines / headed CI runners.
//!
//! Uses the public `lemurclaw_webui::assets` accessors (`dist_file`,
//! `dist_subdir_files`) rather than re-`include_dir!`-ing the path, so the
//! `include_dir::Dir` type stays crate-private (AGENTS.md "crate API surface"
//! rule). The compile-time `include_dir!` inside `assets.rs` already proves
//! the embedding works at build time; these tests assert the *contents* are
//! sane.

use lemurclaw_webui::assets;

#[test]
fn dist_index_html_exists_and_is_non_empty() {
    let contents = assets::dist_file("index.html").expect(
        "assets/dist/index.html must exist — run `npm run build` in lemurclaw-webui/assets/",
    );
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
    let files = assets::dist_subdir_files("assets");
    let js_files: Vec<_> = files
        .iter()
        .filter(|(name, _)| name.ends_with(".js"))
        .collect();
    assert!(!js_files.is_empty(), "no .js bundles found in dist/assets/");
    for (name, bytes) in js_files {
        // The React + app bundle is hundreds of KB. Sanity floor at 10 KB so
        // we catch an accidental empty chunk without being flaky on future
        // code-splitting changes.
        assert!(
            bytes.len() > 10_000,
            "{name} is only {} bytes — expected a real bundle",
            bytes.len()
        );
    }
}

#[test]
fn dist_contains_a_nontrivial_css_bundle() {
    let files = assets::dist_subdir_files("assets");
    let css_files: Vec<_> = files
        .iter()
        .filter(|(name, _)| name.ends_with(".css"))
        .collect();
    assert!(
        !css_files.is_empty(),
        "no .css bundles found in dist/assets/"
    );
    for (name, bytes) in css_files {
        assert!(
            bytes.len() > 1_000,
            "{name} is only {} bytes — expected a real stylesheet",
            bytes.len()
        );
    }
}
