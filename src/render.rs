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

/// Produce bilingual output: English response with Swahili gloss annotations.
///
/// For each content word in the English response, find its Swahili equivalent
/// and add it as a parenthetical annotation. The result is a bilingual
/// response where the user can see both English and Swahili.
pub fn render_bilingual(model_output: &str, index: &RenderIndex) -> String {
    let mut annotated = Vec::new();

    for word in model_output.split_whitespace() {
        let clean: String = word.chars().filter(|c| c.is_alphabetic() || *c == '-' || *c == '\'').collect();
        if clean.len() < 3 {
            annotated.push(word.to_string());
            continue;
        }
        let lower = clean.to_lowercase();
        match index.lookup(&lower) {
            Some(entries) if !entries.is_empty() => {
                let (sw, pos) = &entries[0];
                // Only annotate content words (nouns, verbs, adjectives), not function words
                if matches!(pos.as_str(), "noun" | "verb" | "adj" | "adv") {
                    annotated.push(format!("{}({})", word, sw));
                } else {
                    annotated.push(word.to_string());
                }
            }
            _ => {
                annotated.push(word.to_string());
            }
        }
    }

    annotated.join(" ")
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

/// Curated English→Swahili gloss table for constrained domains.
/// Manually verified mappings (lexicon glosses are too noisy for auto-reverse).
pub fn curated_gloss() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        // Medical
        ("fever", "homa", "noun"),
        ("pain", "maumivu", "noun"),
        ("headache", "maumivu ya kichwa", "noun"),
        ("cough", "kikohozi", "noun"),
        ("chest", "kifua", "noun"),
        ("stomach", "tumbo", "noun"),
        ("eye", "jicho", "noun"),
        ("ear", "sikio", "noun"),
        ("throat", "koo", "noun"),
        ("teeth", "meno", "noun"),
        ("leg", "mguu", "noun"),
        ("arm", "mkono", "noun"),
        ("back", "mgongo", "noun"),
        ("skin", "ngozi", "noun"),
        ("doctor", "daktari", "noun"),
        ("hospital", "hospitali", "noun"),
        ("medicine", "dawa", "noun"),
        ("diarrhea", "kuhara", "noun"),
        ("vomiting", "kutapika", "noun"),
        ("breathing", "kupumua", "verb"),
        ("swelling", "uvimbe", "noun"),
        ("rash", "gozi nyekundu", "noun"),
        ("insomnia", "usingizi mbaya", "noun"),
        ("fracture", "mfupa umevunjika", "noun"),
        // Education
        ("teacher", "mwalimu", "noun"),
        ("student", "mwanafunzi", "noun"),
        ("children", "watoto", "noun"),
        ("school", "shule", "noun"),
        ("book", "kitabu", "noun"),
        ("education", "elimu", "noun"),
        ("exams", "mitihani", "noun"),
        ("library", "maktaba", "noun"),
        ("science", "sayansi", "noun"),
        ("mathematics", "hesabu", "noun"),
        ("study", "kusoma", "verb"),
        ("teach", "kufundisha", "verb"),
        ("learn", "kujifunza", "verb"),
        // Agriculture
        ("farmer", "mkulima", "noun"),
        ("rice", "mpunga", "noun"),
        ("seed", "mbegu", "noun"),
        ("agriculture", "kilimo", "noun"),
        ("water", "maji", "noun"),
        ("crops", "mazao", "noun"),
        ("cattle", "ng'ombe", "noun"),
        ("grass", "nyasi", "noun"),
        ("fertilizer", "mbolea", "noun"),
        ("rain", "mvua", "noun"),
        ("farm", "shamba", "noun"),
        ("irrigation", "umwagiliaji", "noun"),
        ("poultry", "kuku", "noun"),
        ("vegetables", "mboga", "noun"),
        ("harvest", "kuvuna", "verb"),
        ("plant", "kupanda", "verb"),
        ("cultivate", "kulima", "verb"),
        // Common
        ("house", "nyumba", "noun"),
        ("market", "soko", "noun"),
        ("road", "barabara", "noun"),
        ("car", "gari", "noun"),
        ("sun", "jua", "noun"),
        ("money", "fedha", "noun"),
        ("work", "kazi", "noun"),
        ("big", "kubwa", "adj"),
        ("small", "ndogo", "adj"),
        ("long", "refu", "adj"),
        ("short", "fupi", "adj"),
        ("good", "nzuri", "adj"),
        ("bad", "mbaya", "adj"),
        ("hot", "moto", "adj"),
        ("cold", "baridi", "adj"),
        ("important", "muhimu", "adj"),
        ("safe", "salama", "adj"),
        ("dangerous", "hatari", "adj"),
        ("future", "mustakabali", "noun"),
    ]
}

/// Build render index from curated gloss table.
pub fn build_render_index() -> RenderIndex {
    let mut map: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for (en, sw, pos) in curated_gloss() {
        map.entry(en.to_lowercase())
            .or_default()
            .push((sw.to_string(), pos.to_string()));
    }
    RenderIndex { map }
}

/// Render English model output with Swahili gloss annotations.
pub fn render_bilingual(model_output: &str, index: &RenderIndex) -> String {
    let mut annotated = Vec::new();
    for word in model_output.split_whitespace() {
        let clean: String = word.chars().filter(|c| c.is_alphabetic() || *c == '-' || *c == '\'').collect();
        if clean.len() < 3 {
            annotated.push(word.to_string());
            continue;
        }
        let lower = clean.to_lowercase();
        match index.lookup(&lower) {
            Some(entries) if !entries.is_empty() => {
                let (sw, pos) = &entries[0];
                if matches!(pos.as_str(), "noun" | "verb" | "adj" | "adv") {
                    annotated.push(format!("{}({})", word, sw));
                } else {
                    annotated.push(word.to_string());
                }
            }
            _ => annotated.push(word.to_string()),
        }
    }
    annotated.join(" ")
}
