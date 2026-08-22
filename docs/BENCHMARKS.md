# Sema x Jericho - Benchmark Findings

Test set: **100 Swahili sentences** (`testdata/swahili_sentences.csv`,
columns: ID, Swahili sentence, English translation, grammar focus). The set
spans noun-class agreement (1-18), tense/aspect (-na-, -li-, -ta-, -me-,
-me--sha-, -nge-, -ngali-), possessives, locatives and interrogatives.

Reproduction commands at the bottom. Numbers measured on the dev machine
(Windows 11, Rust 1.97.1, Ollama with qwen2.5:0.5b), Aug 2026.

---

## 1. Anchor coverage (Sema layer)

`cargo run --release --example bench_csv -- testdata/swahili_sentences.csv`
(run from the Sema repo, same lexicon as shipped in `jericho/data/`)

| Metric | Value |
|---|---|
| Content words | 333 |
| Resolved | 288 (**86.5%**) |
| Sentences with >= 1 anchor | **100 / 100** |
| Sentences fully covered | 58 |
| Zero-hit sentences | **0** |

Resolution paths:

| Path | Words | Share of resolved |
|---|---|---|
| Exact lemma hit | 243 | 84% |
| Forms index | 0 | 0% |
| Affix strip (morphology) | 45 | 16% |

The affix path is doing real work: `wanacheza -> cheza`, `umefikia ->
fikia`, `tumeshapata -> pata` resolve only through morphology-aware
stripping. The forms index contributing 0 suggests inflected-form data in
the distill step is sparse relative to what affix rules already cover.

### Where coverage drops (upstream TODO material)

The weakest sentences are exactly the advanced conditional/negative-polar
constructions:

- #49 `Vitabu vilivyopotea vimepatikana.` (33%) - relative-form verb
  `-pot-` + subject concords not in lexicon/affix table.
- #92 `Kama ungalijua usingekwenda.` (33%) - compound conditional
  `-ngali-`, negated `-singe-`.
- ~8 sentences at 50%: hypothetical `-nge-` forms (#93), negative-polar
  `huku-`, and adjective concord variants.

**Actionable**: richer Swahili verb extensions (relative `-po-/-ko-/-mo-`,
conditionals) in the affix TOML would likely push coverage >92%.

## 2. Anchor latency & memory

Per-sentence cost (segment + resolve all words, release build):

| p50 | p95 | max |
|---|---|---|
| 3 us | 7 us | 32 us |

Throughput context: the entire 100-sentence corpus anchors in < 0.5 ms of
CPU time. The anchor pass is effectively free next to any LLM call.
Resident memory of the benchmark process peaks at ~20 MB - that *is* the
lexicon (19.7k entries + forms index + affix table) fully loaded.

## 3. RAG retrieval: raw TF-IDF vs Sema-lemmatized

Method (`cargo test --release swahili_bench -- --ignored --nocapture`):
100 corpus chunks; 10 queries that are morphological variants of corpus
sentences (different subject prefix, pluralized objects, added adverbs).
A human sees query/target as the same meaning; raw TF-IDF cannot because
no surface token overlaps.

| Metric | Raw tokenizer | Sema lemmatized |
|---|---|---|
| Top-1 hit rate | 7 / 10 | **10 / 10** |
| MRR | 0.78 | **1.00** |
| Corpus fit time | 7 ms | 4 ms |

Highlights:

- `nimeshapata chakula` found nothing relevant raw (best: an unrelated
  play sentence); lemmatized retrieves `Tumeshapata chakula.`
- `miti hii ni mirefu` <-> `mti huyu ni mrefu` (plural pair across noun
  classes): perfect 1.00 similarity only after concord+stem resolution.
- Lemmatization did not hurt a single query and even cut fit time slightly
  (fewer distinct terms -> smaller vocabulary).

Caveat: 10 handcrafted queries over 100 one-line docs. Directional, not
statistically deep - but the failure mode it fixes (agglutination vs
lexical matching) scales with corpus size, so gains should grow.

## 4. End-to-end A/B through Ollama (qwen2.5:0.5b)

Same system prompt, temperature 0.3, num_predict 90. RAW = sentence alone;
SEMA = Jericho's anchored prompt block + original message.

| Sentence | RAW behavior | SEMA behavior |
|---|---|---|
| #1 Mimi ni mwalimu. | nonsense ("ni 2023") | engages content, attempts translation |
| #14 Ufunguo upo wapi? | refuses for context | echoes + has key/where anchors available |
| #21 Ninasoma kitabu sasa. | "not a standard word" refusal | produces "Ninasoma read the book now" - glosses leak through correctly |
| #24 Tumeshapata chakula. | refuses | engages, correct food/get anchors |
| #49 Vitabu vilivyopotea... | hallucinates word salad (Estonian-ish!) | attempts translation using partial anchors |
| #92 Kama ungalijua... | refuses | engages despite low coverage |

Pattern: anchoring converts refusals/gibberish into content-bearing
responses. Latency also improved (RAW rambled to 7 s on #1; SEMA stayed
1-2 s). Honest limit: a 0.5B model still cannot compose full fluent
answers from skeletons - it parrots glosses. The skeleton becomes decisive
at qwen2.5:3b/7b-class sizes where English competence exists but Swahili
tokenization still does not.

## 5. Resource expenditure log (measured during these runs)

Process-level peak RSS / CPU, sampled live during execution:

| Component | Peak RAM | CPU | Notes |
|---|---|---|---|
| Sema bench_csv.exe | 19.6 MB | 94 ms | ~all of it = loaded lexicon |
| swahili_bench test binary | 20.7 MB | 47 ms | anchor pass + 2x TF-IDF fits + retrieval, wall 0.08-0.14 s |
| Jericho GUI (idle, anchored mode on) | 122.7 MB | ~31 ms per refresh window | 26 threads; lexicon resident inside app process |
| llama-server (Ollama runner, qwen2.5:0.5b) | **413.7 MB** | dominant consumer | model weights mmap'd during inference |
| ollama server daemon | 23 MB | 3.9 s over burst | API routing only |
| ollama app tray | 6.6 MB | - | - |

Inference wall times (4-call anchored burst): ~7 s total, ~1.8 s/call.

Disk & build costs:

| Item | Size |
|---|---|
| swahili.distilled.jsonl | 1.7 MB (shipped in repo/app) |
| bench_csv.exe | 0.62 MB |
| jericho target/ (build artifacts) | ~2.9 GB |
| sema target/ | ~0.27 GB |
| Full release build (jericho, cold incl. deps) | 6m 45s |
| Incremental test build (release) | 31 s |
| sema example build | 6 s |

Context: co-tenants on this machine during measurement were Chrome (~520 MB),
the coding agent itself (~470 MB) and Defender (~220 MB). Total stack
(Jericho GUI + lexicon + 0.5B model) fits comfortably in ~550 MB working set,
consistent with the 4 GB budget in the README.

## Reproducing

```sh
# Sema side (in Nama-ResearchLab/Sema checkout)
cargo run --release --example acceptance                       # built-in harness
cargo run --release --example bench_csv -- path/to/test.csv    # CSV harness

# Jericho side
cargo test --release swahili_bench -- --ignored --nocapture    # anchor + RAG bench
```

A/B transcripts were produced by POSTing `/api/chat` with the exact prompt
Jericho builds in `spawn_chat()`; the JSONL blocks came from
`bench_csv --json`, guaranteeing byte-identical anchoring between offline
bench and live chat.
