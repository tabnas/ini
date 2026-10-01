// The translation parts: what the manifest says and what the crate
// embeds are the same files.
//
// A packaged crate holds nothing outside `rs/`, so the crate embeds its
// own copies, `rs/translate/manifest.json` of `tabnas.plugin.json` and
// `rs/translate/render.alc` of the render the manifest names, as
// `manifest_text()` and `render_text()`. The copies are the only texts a
// host sees, so they must be the files: this holds the embedded manifest
// to the repository's, and the render the manifest names, read from the
// repository, to the embedded one. Change the file at the root and copy
// it into `rs/translate/`; this fails until both are the same.
//
// The render is not run here: that takes alchemy, which this crate does
// not depend on. The round trip, every fixture read, written through the
// render and read back, runs in the host's suite.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The repository root: this crate sits in `rs/`, directly under it.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rs/ sits directly under the repository root")
        .to_path_buf()
}

fn manifest() -> Value {
    serde_json::from_str(tabnas_ini::manifest_text()).expect("the manifest is JSON")
}

fn translate() -> Value {
    manifest()
        .get("translate")
        .cloned()
        .expect("the manifest carries a translate object")
}

#[test]
fn the_manifest_the_crate_embeds_is_the_repositorys() {
    let on_disk = fs::read_to_string(repo_root().join("tabnas.plugin.json"))
        .expect("the repository has its manifest");
    assert_eq!(
        on_disk,
        tabnas_ini::manifest_text(),
        "rs/translate/manifest.json is not tabnas.plugin.json: copy the manifest into rs/translate"
    );
}

#[test]
fn the_render_the_manifest_names_is_the_one_the_crate_embeds() {
    let translate = translate();
    let path = translate["render"]
        .as_str()
        .expect("translate.render names a file");
    let on_disk = fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|e| panic!("translate.render names {path}, which cannot be read: {e}"));
    assert_eq!(
        on_disk,
        tabnas_ini::render_text(),
        "translate.render names {path}, and rs/translate/render.alc, which render_text() \
         embeds, is another text: copy the render into rs/translate"
    );
}

/// INI is read as a tree and written from one. Its events carry the
/// tree already, so there is no lift, and no accessor for one.
#[test]
fn ini_reads_and_writes_a_tree_with_no_lift() {
    let translate = translate();
    assert_eq!(translate["reads"], "tree");
    assert_eq!(translate["writes"], "tree");
    assert_eq!(translate.get("lift"), None);
}

/// A host keys its registry of parts by the manifest's `languageId`.
#[test]
fn the_manifest_names_the_language() {
    assert_eq!(manifest()["languageId"], "ini");
}

/// The host prints the loss lines verbatim, so each is a sentence.
#[test]
fn the_loss_is_a_list_of_sentences() {
    let translate = translate();
    let loss = translate["loss"]
        .as_array()
        .expect("translate.loss is a list");
    assert!(!loss.is_empty());
    for line in loss {
        let line = line.as_str().expect("each loss line is a string");
        assert!(
            line.starts_with(char::is_uppercase) && line.ends_with('.'),
            "{line:?} is not a sentence"
        );
    }
}

/// A host links the render with its own program and other formats'
/// parts, so every definition is named for INI, the entry point is
/// `ini-render`, and the file defines no `export` of its own.
#[test]
fn the_render_is_a_library_named_for_ini() {
    let names: Vec<&str> = tabnas_ini::render_text()
        .lines()
        .filter_map(|line| line.strip_prefix("def "))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect();
    assert!(names.contains(&"ini-render"), "{names:?}");
    for name in &names {
        assert!(name.starts_with("ini-"), "{name} is not named for INI");
    }
}
