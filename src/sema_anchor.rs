//! Sema anchor layer - offline Swahili -> English semantic skeletons.
//!
//! Wraps the `sema` crate (Nama-ResearchLab). Small models are strongest in
//! English; this module resolves each Swahili word to a lemma, part of
//! speech and English gloss (fully offline) so the LLM can reason on an
//! English skeleton instead of guessing at morphology it has never seen.

use sema::lexicon::segment_words;
use sema::{Lexicon, Skeleton};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Loads and applies the distilled lexicon.
pub struct Anchor {
    lex: Arc<Lexicon>,
}

/// Result of anchoring one user message.
pub struct AnchoredInput {
    pub anchors: Vec<Skeleton>,
    pub unresolved: Vec<String>,
}

/// When every gloss of an entry is Wiktionary form-of boilerplate
/// ("Applicative form of -fika: to arrive at"), surface the meaningful part.
/// Strip Wiktionary boilerplate aggressively. For a 0.5B model, the
/// gloss needs to be a clean English word, not a grammar lecture.
fn tidy_gloss(gloss: &str) -> String {
    let t = gloss.trim();
    // "Applicative form of -fika: to arrive at" → "to arrive at"
    if let Some(pos) = t.find(':') {
        let rest = t[pos + 1..].trim();
        if !rest.is_empty() {
            return rest.to_string();
        }
    }
    // Remove class_((...)) annotations
    let mut clean = t.to_string();
    while let Some(start) = clean.find("class_((") {
        if let Some(end) = clean[start..].find("))") {
            clean = format!("{}{}", &clean[..start], &clean[start+end+2..]);
        } else { break; }
    }
    let clean = clean.trim().to_string();
    // If it still contains grammatical descriptions, it's not a usable gloss
    let garbage = ["inflected form of", "infinitive of", "alternative form of",
        "first-person", "second-person", "present affirmative of",
        "positive degree present of", "plural of", "inflection of"];
    for g in &garbage {
        if clean.to_lowercase().contains(g) { return String::new(); }
    }
    // Dash-prefixed roots and very short leftovers are not useful
    if clean.starts_with('-') || clean.len() <= 1 { return String::new(); }
    clean
}

/// A single word tagged with its grammatical role.
#[derive(Debug, Clone)]
pub struct TaggedWord {
    pub surface: String,
    pub pos: String,
    pub gloss: String,
    pub role: &'static str,
}

/// Role-tagged parse of a sentence.
#[derive(Debug)]
pub struct SentenceTag {
    pub words: Vec<TaggedWord>,
    pub unresolved: Vec<String>,
}

impl SentenceTag {
    pub fn has_roles(&self) -> bool {
        !self.words.is_empty()
    }

    pub fn to_summary(&self) -> String {
        self.words.iter().map(|w| {
            format!("{}={}[{}]", w.surface, w.gloss, w.role)
        }).collect::<Vec<_>>().join(" ")
    }
}

impl Anchor {
    /// Load a distilled JSONL lexicon (Swahili affix table is embedded in sema).
    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let lex = Lexicon::load(path)?;
        Ok(Self {
            lex: Arc::new(lex),
        })
    }

    /// Shared handle for other subsystems (RAG lemmatization).
    pub fn lexicon(&self) -> Arc<Lexicon> {
        Arc::clone(&self.lex)
    }

    pub fn lemma_count(&self) -> usize {
        self.lex.len()
    }

    /// Role-tag a sentence: segment, resolve, assign S/V/O/M by POS.
    pub fn tag_sentence(&self, text: &str) -> SentenceTag {
        let mut words = Vec::new();
        let mut unresolved = Vec::new();
        let mut seen_verb = false;

        for tok in segment_words(text) {
            if !tok.chars().any(|c| c.is_alphabetic()) {
                continue;
            }
            match self.lex.skeleton_for(&tok) {
                Some(sk) => {
                    let gloss = tidy_gloss(&sk.gloss);
                    let role = match sk.pos.as_str() {
                        "verb" => { seen_verb = true; "V" }
                        "noun" | "name" | "pron" => {
                            if !seen_verb { "S" } else { "O" }
                        }
                        "adj" | "adv" => "M",
                        _ => "M",
                    };
                    words.push(TaggedWord {
                        surface: sk.surface,
                        pos: sk.pos,
                        gloss,
                        role,
                    });
                }
                None => unresolved.push(tok),
            }
        }

        SentenceTag { words, unresolved }
    }

    /// Resolve every word of `text` against the lexicon.
    pub fn anchor_text(&self, text: &str) -> AnchoredInput {
        let mut anchors = Vec::new();
        let mut unresolved = Vec::new();
        for tok in segment_words(text) {
            // Skip standalone punctuation tokens produced by the segmenter.
            if tok.chars().all(|c| !c.is_alphabetic()) {
                continue;
            }
            match self.lex.skeleton_for(&tok) {
                Some(sk) => anchors.push(sk),
                None => unresolved.push(tok),
            }
        }
        AnchoredInput { anchors, unresolved }
    }

    /// Legacy block-format anchor for backward compat / testing.
    pub fn prompt_block(&self, text: &str) -> String {
        let anchored = self.anchor_text(text);
        if anchored.anchors.is_empty() {
            return String::new();
        }
        let mut out = String::new();
        for sk in &anchored.anchors {
            out.push_str(&format!(
                "{} -> {} ({}): '{}'",
                sk.surface,
                sk.lemma,
                sk.pos,
                tidy_gloss(&sk.gloss)
            ));
            if let Some(r) = &sk.root {
                out.push_str(&format!(" root={r}"));
            }
            out.push('\n');
        }
        if !anchored.unresolved.is_empty() {
            out.push_str(&format!("unresolved: {}\n", anchored.unresolved.join(", ")));
        }
        out.push_str("\nREPLY TO THE USER IN ENGLISH. Do not analyze or translate.");
        out
    }
}

/// Try the configured path first, then common repo layouts.
pub fn load_anchor(configured: &str) -> std::io::Result<Anchor> {
    let mut tried = Vec::new();
    let mut candidates = vec![PathBuf::from(configured)];
    candidates.push(PathBuf::from("data/swahili.distilled.jsonl"));
    candidates.push(PathBuf::from("../sema/data/swahili.distilled.jsonl"));
    candidates.push(PathBuf::from("./swahili.distilled.jsonl"));
    for cand in candidates {
        match Anchor::load(&cand) {
            Ok(a) => return Ok(a),
            Err(e) => tried.push(format!("{} ({e})", cand.display())),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("lexicon not found; tried: {}", tried.join("; ")),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mini_lex() -> Anchor {
        let tmp = std::env::temp_dir().join("jericho_sema_test.jsonl");
        std::fs::write(
            &tmp,
            concat!(
                r#"{"w":"fikia","p":"verb","g":["Applicative form of -fika: to arrive at"],"r":"-fika"}"#,
                "\n",
                r#"{"w":"wapi","p":"adv","g":["where"]}"#
            ),
        )
        .unwrap();
        Anchor::load(&tmp).unwrap()
    }

    #[test]
    fn prompt_block_resolves_affixes() {
        let a = mini_lex();
        let block = a.prompt_block("umefikia wapi zzinga?");
        assert!(block.contains("fikia (verb)"), "block:\n{block}");
        assert!(block.contains("to arrive at"), "block:\n{block}");
        assert!(block.contains("root=-fika"), "block:\n{block}");
        assert!(block.contains("unresolved: zzinga"), "block:\n{block}");
        assert!(!block.contains('?'), "punctuation must not leak in: {block}");
        assert!(block.contains("REPLY"), "must have directive: {block}");
    }

    #[test]
    fn tidy_gloss_strips_class_boilerplate() {
        assert_eq!(tidy_gloss("Applicative form of -fika: to arrive at"), "to arrive at");
        assert_eq!(tidy_gloss("ji class_((V)) inflected form of -angu"), "");
        assert_eq!(tidy_gloss("n class_((IX)) inflected form of -a"), "");
        assert_eq!(tidy_gloss("plural of kitabu"), "");
        assert_eq!(tidy_gloss("infinitive of -fika"), "");
        assert_eq!(tidy_gloss("please"), "please");
    }

    #[test]
    fn english_passthrough_is_empty() {
        let a = mini_lex();
        assert!(a.prompt_block("hello there friend").is_empty());
    }
}
