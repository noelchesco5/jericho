//! Render layer: map English model output back to Swahili via Sema's gloss table.
//!
//! After the model reasons in English, this module finds Swahili equivalents
//! for key content words and produces a bilingual response. The model never
//! back-translates prose — Sema handles the rendering.

use sema::Lexicon;
use std::collections::HashMap;

/// Reverse index: English gloss (lowercase) -> Vec of (Swahili surface form, POS).
pub struct RenderIndex {
    map: HashMap<String, Vec<(String, String)>>,
}

impl RenderIndex {
    /// Build from a word list + lexicon. For each Swahili word, resolve it
    /// and record its English gloss as a key pointing back to the Swahili form.
    pub fn from_words(words: &[&str], lex: &Lexicon) -> Self {
        let mut map: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for &w in words {
            if let Some(sk) = lex.skeleton_for(w) {
                let gloss = clean_gloss(&sk.gloss);
                if !gloss.is_empty() {
                    map.entry(gloss.to_lowercase())
                        .or_default()
                        .push((sk.surface, sk.pos));
                }
            }
        }
        Self { map }
    }

    /// Find Swahili equivalents for an English word.
    pub fn lookup(&self, english: &str) -> Option<&[(String, String)]> {
        self.map.get(&english.to_lowercase()).map(|v| v.as_slice())
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Map English model output to bilingual by finding Swahili equivalents
/// for content words. Returns (Swahili skeleton, English original).
pub fn render_to_swahili(model_output: &str, index: &RenderIndex) -> (String, String) {
    let mut swahili_words = Vec::new();
    let mut english_words: Vec<&str> = Vec::new();

    for word in model_output.split_whitespace() {
        let clean: String = word.chars().filter(|c| c.is_alphabetic() || *c == '-' || *c == '\'').collect();
        if clean.len() < 3 {
            english_words.push(word);
            continue;
        }
        let lower = clean.to_lowercase();
        match index.lookup(&lower) {
            Some(entries) if !entries.is_empty() => {
                let (sw, pos) = &entries[0];
                swahili_words.push(format!("{}({})", sw, pos));
                english_words.push(word);
            }
            _ => {
                english_words.push(word);
            }
        }
    }

    let sw = swahili_words.join(" ");
    let en = english_words.join(" ");
    (sw, en)
}

fn clean_gloss(gloss: &str) -> String {
    let t = gloss.trim();
    if let Some(pos) = t.find(':') {
        let rest = t[pos + 1..].trim();
        if !rest.is_empty() { return rest.to_string(); }
    }
    let mut clean = t.to_string();
    while let Some(start) = clean.find("class_((") {
        if let Some(end) = clean[start..].find("))") {
            clean = format!("{}{}", &clean[..start], &clean[start+end+2..]);
        } else { break; }
    }
    let clean = clean.trim().to_string();
    for g in &["inflected form of", "infinitive of", "alternative form of",
        "first-person", "second-person", "present affirmative of",
        "positive degree present of", "plural of", "inflection of"] {
        if clean.to_lowercase().contains(g) { return String::new(); }
    }
    if clean.starts_with('-') || clean.len() <= 1 { String::new() } else { clean }
}

/// Common Swahili content words to index for rendering.
pub const SWAHILI_WORDS: &[&str] = &[
    "habari","jina","mwalimu","mtoto","nyumba","kitabu","maji","chakula",
    "safari","daktari","hospitali","homa","maumivu","kikohozi","kifua",
    "tumbo","jicho","sikio","koo","meno","mguu","mkono","mgongo","ngozi",
    "kikapu","meza","kiti","mlango","jua","mvua","tembo","ndege",
    "fedha","kazi","shule","soko","barabara","gari","mpunga","mbegu",
    "basi","dawa","elimu","usafiri","kilimo","mpango","nchi",
    "asubuhi","jioni","usiku","sasa","baada","kabla",
    "kuimba","kuchorea","kucheza","kusoma","kufundisha","kununua",
    "kupanda","kuvuna","kupika","kulia","kulala","kuamka",
    "kuzuri","mbaya","kubwa","ndogo","refu","fupi","jema","baya",
    "moto","baridi","pana","embamba","nzito","nyepesi",
];
