# Sema Integration - Implementation Notes

How [Sema](https://github.com/Nama-ResearchLab/Sema) is embedded in Project
Jericho: what runs where, which code owns each step, and how to configure or
extend it. Benchmark results live in [BENCHMARKS.md](BENCHMARKS.md).

## Why

Small local models (qwen2.5:0.5b and friends) are English-trained. Swahili
input arrives as morphology-heavy surface forms (`umefikia`, `wanacheza`)
that such models tokenize into noise. Sema is a fully offline resolver:
surface word -> lemma + part-of-speech + English gloss + derivational root,
from a 1.8 MB distilled Wiktionary lexicon (19,717 Swahili lemmas). No
network, no embedding model, no translation API.

Jericho uses it in two places:

```
                anchor                 reason
Swahili input ----------> EN skeleton --------> Ollama model ---> response
             (sema_anchor.rs)      (unchanged chat flow)

RAG query ------------------> TF-IDF over lemmatized corpus (src/rag/)
```

## Components

### 1. `src/sema_anchor.rs` - the anchor layer

Wraps the `sema` crate:

| Item | Purpose |
|------|---------|
| `Anchor::load(path)` | loads a `Lexicon` from distilled JSONL |
| `Anchor::anchor_text(text)` | segments + resolves each word into a `Skeleton` (surface, lemma, pos, gloss, root) |
| `Anchor::prompt_block(text)` | renders the LLM-facing block; empty string when nothing resolves, so English passes through untouched |
| `Anchor::lexicon()` | shared `Arc<Lexicon>` handed to the RAG embedder |
| `load_anchor(configured)` | tries configured path then common fallbacks |

Block format prepended to chat messages:

```
[SEMANTIC ANCHORS - the user's Swahili words resolved offline to English]
umefikia -> fikia (verb): 'to arrive at' root=-fika
wapi -> wapi (adv): 'where'
unresolved: zzinga
Use these anchors to understand the original message below. Reply helpfully.

---
User message (original): umefikia wapi?
```

Design notes:

- Wiktionary form-of boilerplate ("Applicative form of -fika: ...") is tidied
  to its meaningful tail by `tidy_gloss()`.
- Punctuation is skipped; unknown alphabetic words go to an `unresolved:` line.
- Zero resolutions (English input) -> block omitted entirely.

### 2. Chat hook - `src/app.rs`

- `anchor: Option<Anchor>` field; loaded at startup when
  `[sema] enabled = true` (`new()`), status posted as a system chat message in
  `initialize()`.
- In `spawn_chat()` the outgoing user message becomes
  `format!("{block}\n---\nUser message (original): {input}")` when anchoring
  produced content, otherwise the raw input.
- Config changes apply live in the dirty-config handler: toggling off unloads
  the lexicon *and* detaches the RAG lemmatizer.

### 3. RAG lemmatization - `src/rag/mod.rs`

Swahili agglutinates: `umefikia`, `wamefika`, `tumeifikia` share the root
`fika` but are different tokens. Raw TF-IDF treats them as unrelated words,
so retrieval on morphologically varied queries collapses.

- `LocalEmbedder` gained an optional `lemmatizer: Option<Arc<Lexicon>>`.
- `tokenize_text()` maps every token through `Lexicon::lookup()` before
  counting term frequencies - applied identically to ingested documents,
  index rebuilds and queries, so both sides of the cosine comparison live in
  lemma space.
- Attached via `RagPipeline::set_lemmatizer(Some(anchor.lexicon()))` when
  `[sema] lemmatize_rag = true`. Detaching restores stock behavior.

### 4. Configuration - `src/config.rs`

```toml
[sema]
enabled = true                                  # anchor pass on chat input
lexicon_path = "data/swahili.distilled.jsonl"   # fallbacks: ../sema/data/..., ./swahili.distilled.jsonl
lemmatize_rag = true                            # morphology-aware TF-IDF
```

GUI: CONFIG > RAG tab > "SEMA (SWAHILI ANCHORING)" section
(`src/gui/config_panel.rs`). Changes take effect after SAVE CONFIG.

### 5. Data & licensing - `data/`

- `data/swahili.distilled.jsonl` - 19,717 lemmas, 1.7 MB, shipped with the app.
- Derived from Wiktionary via kaikki.org, licensed **CC BY-SA 4.0**
  (code stays MIT). Attribution obligations: see `data/NOTICE.md`.
  If you ship this file in a product you must keep the credit + share-alike.
- Regenerate from raw dumps with `sema-distill` (upstream repo).

## Dependency wiring

```toml
# Cargo.toml
sema = { git = "https://github.com/noelchesco5/Sema" }
```

Points at the fork; upstream lives at Nama-ResearchLab/Sema. Both MIT.
Upgrading = `cargo update -p sema`.

## Extending

- **Better prompt**: current block is deliberately terse for tiny-context
  models. For >=3B models consider adding sentence-role hints once upstream
  ships them.
- **Render layer** (roadmap): answer templates keyed on anchors so responses
  can come back in Swahili without lossy back-translation.
- **New languages**: one data file + one affix TOML upstream; then add a
  language field to `[sema]` and load the matching lexicon.
