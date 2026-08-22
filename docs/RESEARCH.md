# Sema Research: Structured Anchoring for Low-Resource Languages

## Hypothesis

Sema's POS-tagged anchoring can improve small model output quality on
Swahili inputs by providing structured grammatical context rather than
raw gloss injection.

## Architecture

```
Swahili input
    │
    ▼
┌─────────────────────────────┐
│  ANCHOR (Sema)              │
│  segment → resolve → tag    │
│  output: structured roles   │
└─────────────┬───────────────┘
              │
    ┌─────────┴─────────┐
    │                   │
    ▼                   ▼
┌──────────────┐  ┌──────────────┐
│  RAW ANCHOR  │  │  ROLE-TAGGED │
│  word=gloss  │  │  S=noun V=verb│
│  in user msg │  │  structured  │
└──────┬───────┘  └──────┬───────┘
       │                 │
       ▼                 ▼
┌─────────────────────────────┐
│  REASON (LLM)               │
│  model processes context    │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  RENDER (Sema)              │
│  english output → bilingual │
│  via gloss reverse-index    │
└─────────────────────────────┘
```

## Conditions to test

1. **Control**: raw Swahili → 1.5B model → English output
2. **Raw anchor**: Swahili + glosses in user message → 1.5B → English
3. **Role-tagged**: Swahili + structured POS parse → 1.5B → English
4. **Render layer**: role-tagged + model output mapped back to Swahili

## Metrics

- Response accuracy (does the model understand the input?)
- Bilingual quality (is the Swahili render natural?)
- Latency (anchor + model + render)
- Token usage (does structured input reduce token waste?)

## What we built

- `role_tagger.rs`: POS-based role tagging using Sema's lexicon
- `render.rs`: gloss reverse-index for bilingual output
- `sema_anchor.rs`: core anchor pass (unchanged)

## What we removed

- `medical.rs`: keyword matcher that bypassed the LLM (not Sema)
- `system_prompt_addon`: gloss injection into system prompt
- Medical GUI controls (not relevant to research)

## Results so far

- Role tagging: 2.1us/sentence (negligible latency)
- Anchor pass: 3.7us/sentence
- Render index: 0.0ms one-time build
- Role coverage: subjects/verbs/objects identified by POS tags
- POS distribution: noun(3214), verb(713), adj(387), name(281), adv(118)

## Next steps

1. Run controlled A/B through Ollama with4 conditions
2. Measure accuracy across 50 sentences
3. Test render layer: does bilingual output improve user comprehension?
4. Document findings in docs/RESEARCH.md
