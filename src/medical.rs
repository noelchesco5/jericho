//! Medical symptom matcher + bilingual template renderer.
//!
//! When Sema anchors reveal medical keywords, we bypass the LLM entirely
//! and render a safe bilingual response from pre-defined templates.
//! This is the "render" layer of Sema's anchor→reason→render pipeline.

use std::collections::HashMap;

pub struct MedicalMatcher {
    symptoms: Vec<Symptom>,
    body_parts: Vec<BodyPart>,
    /// Dynamic symptom for body-part + pain verb combos (owned, no borrow issues)
    body_pain_fallback: Symptom,
}

struct Symptom {
    swahili: Vec<&'static str>,
    english: &'static str,
    severity: Severity,
    advice_sw: &'static str,
    advice_en: &'static str,
}

struct BodyPart {
    swahili: &'static str,
    english: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Severity {
    Mild,
    Moderate,
    Serious,
    Emergency,
}

pub struct MedicalResponse {
    pub matched_symptoms: Vec<String>,
    pub matched_body_parts: Vec<String>,
    pub severity: Severity,
    pub reply_sw: String,
    pub reply_en: String,
}

impl MedicalMatcher {
    pub fn new() -> Self {
        Self {
            symptoms: vec![
                Symptom {
                    swahili: vec!["homa", "joto", "joto kali"],
                    english: "fever",
                    severity: Severity::Moderate,
                    advice_sw: "Unaweza kuwa na homu. Kunywa maji mengi, kupumzika, na tembelea daktari ikiwa homu inaendelea zaidi ya siku moja.",
                    advice_en: "You may have a fever. Drink plenty of fluids, rest, and see a doctor if the fever persists for more than one day.",
                },
                Symptom {
                    swahili: vec!["kikohozi", "kohoa", "kukohoa"],
                    english: "cough",
                    severity: Severity::Mild,
                    advice_sw: "Kikohozi ni dalili ya maradhi ya kawaida. Kunywa maji ya moto na asali, pumzika, na tembelea daktari ikiwa kikohozi kinaendelea zaidi ya wiki moja.",
                    advice_en: "Cough is a common symptom. Drink warm water with honey, rest, and see a doctor if the cough persists for more than one week.",
                },
                Symptom {
                    swahili: vec!["maumivu ya kichwa", "kichwa kinaniuma", "maumivu yaichwa"],
                    english: "headache",
                    severity: Severity::Mild,
                    advice_sw: "Maumivu ya kichwa ni ya kawaida. Pumzika katika eneo la giza, kunywa maji, na tumia dawa za maumivu ikiwa ni lazima. Tembelea daktari ikiwa maumivu ni makali sana.",
                    advice_en: "Headache is common. Rest in a dark room, drink water, and use pain relievers if necessary. See a doctor if the pain is severe.",
                },
                Symptom {
                    swahili: vec!["maumivu ya tumbo", "tumbo linaniuma", "maumivu ya chumbani"],
                    english: "stomach pain",
                    severity: Severity::Moderate,
                    advice_sw: "Maumivu ya tumbo yanaweza kuwa dalili ya matatizo mbalimbali. Kunywa maji, epuka chakula chenye mafuta, na tembelea daktari ikiwa maumivu ni makali au yaendelea.",
                    advice_en: "Stomach pain can indicate various issues. Drink water, avoid greasy food, and see a doctor if the pain is severe or persistent.",
                },
                Symptom {
                    swahili: vec!["maumivu ya kifua", "kifua kinaniuma"],
                    english: "chest pain",
                    severity: Severity::Emergency,
                    advice_sw: "MAUMIVU YA KIFUA NI DALILI YA DHATI. Piga nambari ya dharura MARA MOJA. Usisubiri.",
                    advice_en: "CHEST PAIN IS A SERIOUS SYMPTOM. Call emergency number IMMEDIATELY. Do not wait.",
                },
                Symptom {
                    swahili: vec!["kupumua vibaya", "kupumua kwa shida", "hamshindwi kupumua"],
                    english: "difficulty breathing",
                    severity: Severity::Emergency,
                    advice_sw: "KUPUMUA KWA SHIDA NI DALILI YA DHATI. Piga nambari ya dharura MARA MOJA. Jitayarisha kukaa chini na kichwa juu kidogo.",
                    advice_en: "DIFFICULTY BREATHING IS A SERIOUS SYMPTOM. Call emergency number IMMEDIATELY. Prepare to sit up with head slightly elevated.",
                },
                Symptom {
                    swahili: vec!["maumivu ya mgongo", "mgongo unaniuma"],
                    english: "back pain",
                    severity: Severity::Mild,
                    advice_sw: "Maumivu ya mgongo ni ya kawaida. Pumzika, epuka kubeba uzito, na tumia baridi/kwanza kwa siku ya kwanza. Tembelea daktari ikiwa maumivu yaendelea.",
                    advice_en: "Back pain is common. Rest, avoid heavy lifting, and use ice/heat for the first day. See a doctor if pain persists.",
                },
                Symptom {
                    swahili: vec!["maumivu ya mguu", "mguu unaniuma"],
                    english: "leg pain",
                    severity: Severity::Mild,
                    advice_sw: "Maumivu ya mguu yanaweza kuwa dalili ya uchovu au majeraha. Pumzika, weka juu ya miguu, na tembelea daktari ikiwa maumivu ni makali.",
                    advice_en: "Leg pain can be from fatigue or injury. Rest, elevate legs, and see a doctor if pain is severe.",
                },
                Symptom {
                    swahili: vec!["jicho linaniuma", "maumivu ya jicho", "jicho ni nyekundu"],
                    english: "eye pain",
                    severity: Severity::Moderate,
                    advice_sw: "Maumivu ya jicho yanahitaji utunzaji maalum. Usiguse jicho, fanya mapumziko ya macho, na tembelea daktari wa macho haraka iwezekanavyo.",
                    advice_en: "Eye pain needs special care. Don't touch the eye, rest your eyes, and see an eye doctor as soon as possible.",
                },
                Symptom {
                    swahili: vec!["maumivu ya sikio", "sikio linaniuma", "sikio linaumwa"],
                    english: "ear pain",
                    severity: Severity::Moderate,
                    advice_sw: "Maumivu ya sikio ni ya kawaida hasa kwa watoto. Tumia joto la joto kwa sikio, na tembelea daktari ikiwa maumivu ni makali au kuna tovuti.",
                    advice_en: "Ear pain is common especially in children. Apply warm compress to ear, and see a doctor if pain is severe or there is discharge.",
                },
                Symptom {
                    swahili: vec!["kuhara", "kipindupindu", "kupungua tumbo"],
                    english: "diarrhea",
                    severity: Severity::Moderate,
                    advice_sw: "Kuhara kunaweza kupeleka upungufu wa maji mwilini. Kunywa ORS au maji mengi, epuka vyakula vya mafuta, na tembelea daktari ikiwa kuhara kinaendelea zaidi ya siku 2.",
                    advice_en: "Diarrhea can cause dehydration. Drink ORS or plenty of fluids, avoid greasy food, and see a doctor if it lasts more than 2 days.",
                },
                Symptom {
                    swahili: vec!["tapeli", "kutapika", "kutapika damu"],
                    english: "vomiting",
                    severity: Severity::Moderate,
                    advice_sw: "Kutapika kwingi kunaweza kupeleka upungufu wa maji. Kunywa maji madogo madogo, epuka chakula kwa muda, na tembelea daktari ikiwa kuna damu.",
                    advice_en: "Frequent vomiting can cause dehydration. Drink small amounts of water, avoid food for a while, and see a doctor if there is blood.",
                },
                Symptom {
                    swahili: vec!["choo kigumu", "cho kigumu", "constipation", "kukosa choo"],
                    english: "constipation",
                    severity: Severity::Mild,
                    advice_sw: "Choo kigumu ni tatizo la kawaida. Kunywa maji mengi, kula vyakula vyenye nyuzi, na fanya mazoezi ya mwili. Tembelea daktari ikiwa inaendelea.",
                    advice_en: "Constipation is common. Drink plenty of water, eat fiber-rich foods, and exercise. See a doctor if it persists.",
                },
                Symptom {
                    swahili: vec!["maumivu ya mifupa", "mifupa inaniuma", "mfupa umevunjika"],
                    english: "bone pain / possible fracture",
                    severity: Severity::Serious,
                    advice_sw: "Maumivu makali ya mifupa yanaweza kuwa dalili ya mfupa uliovunjika. Usisonge sehemu iliyojeruhiwa, imobilize, na piga nambari ya dharura au tembelea hospitali.",
                    advice_en: "Severe bone pain may indicate a fracture. Don't move the injured area, immobilize, and call emergency or go to hospital.",
                },
                Symptom {
                    swahili: vec!["gozi nyekundu", "changa nyekundu", "alama kwenye ngozi"],
                    english: "skin rash",
                    severity: Severity::Mild,
                    advice_sw: "Changa la ngozi linaweza kuwa dalili ya mzio au maambukizi. Epuka kuogoa, tumia Cream ya mzio, na tembelea daktari ikiwa linaenea au kuwa mbaya.",
                    advice_en: "Skin rash may indicate allergy or infection. Don't scratch, use allergy cream, and see a doctor if it spreads or worsens.",
                },
                Symptom {
                    swahili: vec!["maumivu ya meno", "jino linaniuma", "maumivu ya jino"],
                    english: "toothache",
                    severity: Severity::Mild,
                    advice_sw: "Maumivu ya meno ni ya kawaida. Osha mdomo kwa chumvi na maji ya moto, tumia dawa za maumivu, na tembelea daktari wa meno haraka iwezekanavyo.",
                    advice_en: "Toothache is common. Rinse mouth with salt and warm water, use pain relievers, and see a dentist as soon as possible.",
                },
                Symptom {
                    swahili: vec!["maumivu ya koo", "koo linaniuma", "koo kavu"],
                    english: "sore throat",
                    severity: Severity::Mild,
                    advice_sw: "Koo la maumivu ni dalili ya maradhi ya kawaida. Kunywa maji ya moto na asali, fanya mapumziko, na tembelea daktari ikiwa maumivu yaendelea zaidi ya wiki.",
                    advice_en: "Sore throat is a common symptom. Drink warm water with honey, rest, and see a doctor if pain persists for more than a week.",
                },
                Symptom {
                    swahili: vec!["maumivu ya pua", "pua inaniuma", "kutokwa na maambukizi", "maambukizi ya pua", "maambukizi ya sinusi"],
                    english: "nose pain / sinus",
                    severity: Severity::Mild,
                    advice_sw: "Maumivu ya pua yanaweza kuwa dalili ya maambukizi ya sinusi. Kunywa maji ya moto, vuta mvuke, na tembelea daktari ikiwa dalili zinaendelea.",
                    advice_en: "Nose pain may indicate sinus infection. Drink hot water, inhale steam, and see a doctor if symptoms persist.",
                },
                Symptom {
                    swahili: vec!["usingizi mbaya", "usingizi kidogo", "kujiuguza"],
                    english: "insomnia / poor sleep",
                    severity: Severity::Mild,
                    advice_sw: "Usingizi mbaya unaweza kuathiri afya yako. Epuka kahawa jioni, fanya mazoezi, na tembelea daktari ikiwa tatizo linaendelea.",
                    advice_en: "Poor sleep can affect your health. Avoid coffee in the evening, exercise, and see a doctor if the problem persists.",
                },
                Symptom {
                    swahili: vec!["uvimbe", "kuvimba", "miguu imevimba"],
                    english: "swelling",
                    severity: Severity::Moderate,
                    advice_sw: "Uvimbe unaweza kuwa dalili ya maambukizi au matatizo ya moyo. Weka juu ya sehemu iliyovimba, na tembelea daktari haraka iwezekanavyo.",
                    advice_en: "Swelling may indicate infection or heart problems. Elevate the swollen area, and see a doctor as soon as possible.",
                },
            ],
            body_parts: vec![
                BodyPart { swahili: "kichwa", english: "head" },
                BodyPart { swahili: "macho", english: "eyes" },
                BodyPart { swahili: "masikio", english: "ears" },
                BodyPart { swahili: "pua", english: "nose" },
                BodyPart { swahili: "mdomo", english: "mouth" },
                BodyPart { swahili: "koo", english: "throat" },
                BodyPart { swahili: "kifua", english: "chest" },
                BodyPart { swahili: "tumbo", english: "stomach" },
                BodyPart { swahili: "mgongo", english: "back" },
                BodyPart { swahili: "mguu", english: "leg" },
                BodyPart { swahili: "mkono", english: "arm" },
                BodyPart { swahili: "jino", english: "tooth/teeth" },
                BodyPart { swahili: "ngozi", english: "skin" },
                BodyPart { swahili: "mfupa", english: "bone" },
            ],
            body_pain_fallback: Symptom {
                swahili: vec![],
                english: "body-part pain",
                severity: Severity::Mild,
                advice_sw: "Maumivu ya sehemu hii ni ya kawaida. Pumzika, epuka shughuli nzito, na tembelea daktari ikiwa maumivu yaendelea.",
                advice_en: "Pain in this area is common. Rest, avoid heavy activity, and see a doctor if pain persists.",
            },
        }
    }

    /// Match anchored Swahili words against known medical terms.
    /// Returns None if no medical content detected (let LLM handle it).
    pub fn match_medical(&self, swahili_text: &str) -> Option<MedicalResponse> {
        let lower = swahili_text.to_lowercase();
        let mut matched_symptoms: Vec<&Symptom> = Vec::new();
        let mut matched_parts: Vec<&BodyPart> = Vec::new();

        for symptom in &self.symptoms {
            for keyword in &symptom.swahili {
                if lower.contains(keyword) {
                    matched_symptoms.push(symptom);
                    break;
                }
            }
        }

        for part in &self.body_parts {
            if lower.contains(part.swahili) {
                matched_parts.push(part);
            }
        }

        // If a body part appears with pain/swelling verbs, treat as medical
        let pain_verbs = ["naumwa", "linaniuma", "inaniuma", "unaniuma",
            "linauma", "inauma", "unauma", "limevimba", "imevimba",
            "umevimba", "nauma", "niumwe", "lumwa", "iumwa"];
        let mut used_fallback = false;
        if matched_symptoms.is_empty() && !matched_parts.is_empty() {
            for verb in &pain_verbs {
                if lower.contains(verb) {
                    matched_symptoms.push(&self.body_pain_fallback);
                    used_fallback = true;
                    break;
                }
            }
        }

        if matched_symptoms.is_empty() {
            return None;
        }

        // Determine highest severity
        let severity = matched_symptoms.iter().map(|s| s.severity).max_by_key(|s| *s as u8).unwrap();

        // Build bilingual response
        let mut reply_sw = String::new();
        let mut reply_en = String::new();

        if severity == Severity::Emergency {
            reply_sw.push_str("⚠️ DALILI ZA DHATI ⚠️\n\n");
            reply_en.push_str("⚠️ SERIOUS SYMPTOMS ⚠️\n\n");
        }

        // List matched symptoms
        reply_sw.push_str("Dalili zilizopatikana / Detected symptoms:\n");
        reply_en.push_str("Detected symptoms:\n");
        for (i, symptom) in matched_symptoms.iter().enumerate() {
            if i > 0 {
                reply_sw.push_str(", ");
                reply_en.push_str(", ");
            }
            reply_sw.push_str(&format!("{} ({})", symptom.swahili[0], symptom.english));
            reply_en.push_str(symptom.english);
        }
        if !matched_parts.is_empty() {
            reply_sw.push_str("\nSehemu zilizoathiriwa / Affected areas: ");
            reply_en.push_str("\nAffected areas: ");
            for (i, part) in matched_parts.iter().enumerate() {
                if i > 0 {
                    reply_sw.push_str(", ");
                    reply_en.push_str(", ");
                }
                reply_sw.push_str(&format!("{} ({})", part.swahili, part.english));
                reply_en.push_str(part.english);
            }
        }

        reply_sw.push_str("\n\n");
        reply_en.push_str("\n\n");

        // Add advice from each matched symptom
        for symptom in &matched_symptoms {
            reply_sw.push_str(&format!("• {}\n", symptom.advice_sw));
            reply_en.push_str(&format!("• {}\n", symptom.advice_en));
        }

        // Add disclaimer
        if severity != Severity::Emergency {
            reply_sw.push_str("\nUnganisha: Huduma hii si daktari. Tembelea daktari kwa ushauri wa kitaalamu.");
            reply_en.push_str("\nNote: This is not medical advice. Please see a doctor for professional guidance.");
        }

        Some(MedicalResponse {
            matched_symptoms: matched_symptoms.iter().map(|s| s.english.to_string()).collect(),
            matched_body_parts: matched_parts.iter().map(|p| p.english.to_string()).collect(),
            severity,
            reply_sw,
            reply_en,
        })
    }
}

impl std::fmt::Display for MedicalResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n\n---\n\n{}", self.reply_sw, self.reply_en)
    }
}
