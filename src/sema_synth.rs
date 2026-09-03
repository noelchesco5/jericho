//! Sema-Tena render layer - structured intent -> grammatical Swahili verb.
//!
//! This is the BACK-PASS that pairs with `sema_anchor`'s forward pass:
//! after the model reasons in English, its structured fields are compiled
//! into an exact Swahili surface verb by a pure data-driven engine (all
//! morphology lives in Sema-Tena's sw.toml, zero hardcoded language rules
//! in the assembler). Replaces guesswork rendering with guaranteed forms.
//!
//! Note: the in-app forward/backward round-trip (decompose then
//! re-synthesize) waits on the forward decomposer landing in the org Sema
//! repository. Until then this module renders from explicit parts, which
//! is all the chat path needs.

use sema_tena::synth::{Engine, Intent, SynthesisResult};

/// Negated-TAM allomorph surfaces the forward pass emits -> canonical tag.
fn negated_tam(surface: &str) -> Option<&'static str> {
    match surface {
        "ku" => Some("PAST"),
        "ja" => Some("PERF"),
        _ => None,
    }
}

/// The back-pass synthesizer. Morphology is embedded; no lexicon file needed.
pub struct Synth {
    engine: Engine,
}

/// Outcome of a forward/backward round-trip check. The forward half runs
/// wherever the decomposer lives (CLI harness today, in-app once the org
/// Sema repository ships its linearizer module); this struct is the shared
/// shape for reporting it.
#[derive(Debug, Clone)]
pub struct Roundtrip {
    pub verb: String,
    pub produced: String,
    pub exact: bool,
    pub subject_class: Option<String>,
    pub object_class: Option<String>,
}

impl Synth {
    /// Build over the embedded Swahili morphology (always available).
    pub fn embedded() -> Self {
        Self {
            engine: Engine::swahili(),
        }
    }

    /// Build over an explicit morphology file (e.g. another language table).
    pub fn from_path(path: &std::path::Path) -> std::io::Result<Self> {
        let m = sema_tena::synth::Morphology::from_path(path)?;
        Ok(Self {
            engine: Engine::from_morphology(m),
        })
    }

    /// Low-level compile of a structured intent.
    pub fn synthesize(&self, intent: &Intent) -> SynthesisResult {
        self.engine.synthesize(intent)
    }

    /// Convenience: one fully-specified verb.
    #[allow(clippy::too_many_arguments)]
    pub fn verb(
        &self,
        subject: Option<&str>,
        tense: Option<&str>,
        negation: bool,
        object: Option<&str>,
        root: &str,
        derivations: &[&str],
        mood: &str,
    ) -> SynthesisResult {
        self.synthesize(&Intent {
            subject: subject.map(str::to_string),
            tense: tense.map(str::to_string),
            negation,
            object: object.map(str::to_string),
            root: root.to_string(),
            derivations: derivations.iter().map(|s| s.to_string()).collect(),
            mood: mood.to_string(),
            particles: Vec::new(),
        })
    }

    /// Build an intent from explicit forward-analysis parts (subject and
    /// tense surfaces, polarity, object infix, root). Fused negative
    /// subject surfaces resolve through the engine's own data table; the
    /// negated-TAM allomorphs (ku/ja) map to canonical tags; everything
    /// else passes through to the engine's normalizers. Unsegmented
    /// material (derivations, subjunctive si-) rides inside the root and
    /// round-trips verbatim.
    pub fn intent_from_parts(
        &self,
        subject: Option<&str>,
        tense: Option<&str>,
        negated: bool,
        object: Option<&str>,
        root: &str,
    ) -> Intent {
        let subject = subject.map(|s| {
            self.engine
                .morphology()
                .subjects_neg
                .get(s)
                .cloned()
                .unwrap_or_else(|| s.to_string())
        });
        let tense = tense.map(|t| {
            if negated {
                negated_tam(t).unwrap_or(t).to_string()
            } else {
                t.to_string()
            }
        });
        Intent {
            subject,
            tense,
            negation: negated,
            object: object.map(str::to_string),
            root: root.to_string(),
            derivations: Vec::new(),
            mood: "a".to_string(),
            particles: Vec::new(),
        }
    }

    /// Re-synthesize a verb from forward-analysis parts and compare exactly
    /// against the original surface. Feed it parts from the forward
    /// decomposer (CLI `sema morph` today; in-app module once available).
    pub fn roundtrip_parts(
        &self,
        verb: &str,
        subject: Option<&str>,
        tense: Option<&str>,
        negated: bool,
        object: Option<&str>,
        root: &str,
    ) -> Roundtrip {
        let intent = self.intent_from_parts(subject, tense, negated, object, root);
        let out = self.synthesize(&intent);
        let produced = out.surface.clone();
        Roundtrip {
            verb: verb.to_string(),
            exact: produced.trim() == verb.trim(),
            produced,
            subject_class: out.subject_class,
            object_class: out.object_class,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth() -> Synth {
        Synth::embedded()
    }

    #[test]
    fn exact_forms_come_from_data_only() {
        let s = synth();
        assert_eq!(
            s.verb(Some("1SG"), Some("PAST"), true, None, "fanya", &[], "a")
                .surface,
            "sikufanya"
        );
        assert_eq!(
            s.verb(Some("3SG"), Some("PRES"), false, None, "la", &[], "a")
                .surface,
            "anakula"
        );
        assert_eq!(
            s.verb(Some("1SG"), Some("PRES"), false, None, "soma", &["causative"], "a")
                .surface,
            "ninasomesha"
        );
        assert_eq!(
            s.verb(Some("KI"), Some("PAST"), false, None, "vunja", &[], "a")
                .surface,
            "kilivunja"
        );
    }

    #[test]
    fn concord_metadata_rides_along() {
        let s = synth();
        let r = s.verb(Some("KI"), Some("PAST"), false, None, "vunja", &[], "a");
        assert_eq!(r.subject_class.as_deref(), Some("7 (ki/vi)"));
        let r = s.verb(Some("1SG"), Some("PAST"), true, None, "fanya", &[], "a");
        assert_eq!(r.subject_class, None);
    }

    #[test]
    fn roundtrip_from_parts_is_exact() {
        // Parts as the forward decomposer reports them
        // (subject/tense surfaces, polarity, object infix, root).
        let s = synth();
        let cases = [
            ("sikufanya", Some("ni"), Some("ku"), true, None, "fanya"),
            ("anakula", Some("a"), Some("na"), false, Some("ku"), "la"),
            ("hawatakujibu", Some("wa"), Some("ta"), true, Some("ku"), "jibu"),
            ("hajibu", Some("a"), None, true, None, "jibu"),
            ("kilivunja", Some("KI"), Some("li"), false, None, "vunja"),
            ("ninasomesha", Some("ni"), Some("na"), false, None, "somesha"),
            ("usijali", Some("u"), None, false, None, "sijali"),
        ];
        for (verb, subj, tense, neg, obj, root) in cases {
            let r = s.roundtrip_parts(verb, subj, tense, neg, obj, root);
            assert!(r.exact, "round-trip failed for {verb}: {:?}", r.produced);
        }
    }
}
