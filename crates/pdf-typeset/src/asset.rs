//! Shared image-asset handling for the document importers.
//!
//! Both the HTML importer ([`crate::html`]) and the Markdown importer
//! ([`crate::markup`]) fetch referenced images through an [`AssetResolver`] and
//! register them as named in-memory `Typst` files. [`AssetRegistry`] centralizes
//! the fetch, stable naming, and de-duplication so the two importers can't drift.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash as _, Hasher as _};

use crate::AssetResolver;

/// Collects the named in-memory files an import registers — fetched images and,
/// for HTML, math SVGs — fetching and de-duplicating images through an
/// [`AssetResolver`].
pub(crate) struct AssetRegistry<'r> {
    resolver: &'r dyn AssetResolver,
    /// `<img src>` / Markdown destination URL → the `Typst` file name it was
    /// registered under, so a repeated image is fetched once.
    names: HashMap<String, String>,
    assets: Vec<(String, Vec<u8>)>,
    /// Images resolved and registered.
    pub ok: usize,
    /// Images whose source could not be fetched (kept for QA / the UI).
    pub failed: usize,
}

impl<'r> AssetRegistry<'r> {
    pub fn new(resolver: &'r dyn AssetResolver) -> Self {
        Self {
            resolver,
            names: HashMap::new(),
            assets: Vec::new(),
            ok: 0,
            failed: 0,
        }
    }

    /// Resolve an image `src` to its registered `Typst` file name, fetching it
    /// once (deduped by `src`). `None` — and a bumped [`Self::failed`] — when the
    /// source can't be fetched.
    pub fn image(&mut self, src: &str) -> Option<String> {
        if let Some(name) = self.names.get(src) {
            return Some(name.clone());
        }
        let Some(bytes) = self.resolver.fetch(src) else {
            self.failed += 1;
            return None;
        };
        let name = format!("img-{:016x}{}", hash(src), ext_of(src));
        self.assets.push((name.clone(), bytes));
        self.names.insert(src.to_string(), name.clone());
        self.ok += 1;
        Some(name)
    }

    /// Register a pre-built named file (e.g. a math SVG the caller already holds).
    pub fn push(&mut self, name: String, bytes: Vec<u8>) {
        self.assets.push((name, bytes));
    }

    /// The registered files, consuming the registry.
    pub fn into_assets(self) -> Vec<(String, Vec<u8>)> {
        self.assets
    }
}

/// File extension from a URL/path (lowercased, incl. the dot), defaulting to
/// `.png` when none is present.
pub(crate) fn ext_of(src: &str) -> String {
    let path = src.split(['?', '#']).next().unwrap_or(src);
    match path.rsplit('/').next().and_then(|f| f.rsplit_once('.')) {
        Some((_, ext)) if ext.len() <= 5 && ext.chars().all(char::is_alphanumeric) => {
            format!(".{}", ext.to_ascii_lowercase())
        }
        _ => ".png".to_string(),
    }
}

fn hash(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}
