use serde::{Deserialize, Serialize};
use sema::Lexicon;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;
use regex::Regex;

// ============================================================
// RAG - RETRIEVAL AUGMENTED GENERATION
// Chunking, embedding (local), vector store, retrieval pipeline
// ============================================================

/// A single chunk of text extracted from a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub id: String,
    pub source_file: String,
    pub chunk_index: usize,
    pub content: String,
    pub token_estimate: usize,
    pub metadata: ChunkMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub file_extension: String,
    pub file_size_bytes: u64,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub language: Option<String>,
}

/// A document that has been ingested into the RAG store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestedDocument {
    pub path: String,
    pub chunks_count: usize,
    pub total_tokens: usize,
    pub ingested_at: String,
    pub file_hash: String,
}

/// Search result from vector similarity query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagResult {
    pub chunk: DocumentChunk,
    pub similarity: f32,
    pub rank: usize,
}

/// RAG pipeline state and statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagStats {
    pub total_documents: usize,
    pub total_chunks: usize,
    pub total_tokens: usize,
    pub last_ingest_time_ms: f64,
    pub memory_usage_mb: f64,
}

// ---- Embedding Engine (local TF-IDF style, no external deps) ----

/// Simple but effective local embedding using TF-IDF + dimensionality reduction
/// No external model needed - works entirely on CPU
pub struct LocalEmbedder {
    /// Vocabulary: word -> index
    vocab: HashMap<String, usize>,
    /// IDF weights per vocabulary term
    idf: Vec<f32>,
    /// Dimension of output vectors
    dim: usize,
    /// Optional Sema lexicon: lemmatize tokens so Swahili inflected forms
    /// (umefikia, wamefika...) collapse to their lemma (fikia) before TF-IDF
    lemmatizer: Option<Arc<Lexicon>>,
}

impl LocalEmbedder {
    pub fn new(dim: usize) -> Self {
        Self {
            vocab: HashMap::new(),
            idf: Vec::new(),
            dim,
            lemmatizer: None,
        }
    }

    /// Attach a Sema lexicon for morphology-aware tokenization.
    pub fn set_lemmatizer(&mut self, lex: Option<Arc<Lexicon>>) {
        self.lemmatizer = lex;
    }

    /// Build vocabulary and IDF from a corpus of text chunks
    pub fn fit(&mut self, corpus: &[String]) {
        let mut doc_freq: HashMap<String, usize> = HashMap::new();
        let total_docs = corpus.len() as f32;

        // Tokenize and count document frequency
        for doc in corpus {
            let words = self.tokenize_text(doc);
            let unique: std::collections::HashSet<_> = words.into_iter().collect();
            for word in unique {
                *doc_freq.entry(word).or_insert(0) += 1;
            }
        }

        // Build vocabulary
        let mut vocab: Vec<(String, usize)> = doc_freq
            .iter()
            .map(|(w, &df)| (w.clone(), df))
            .collect();
        vocab.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by frequency
        vocab.truncate(self.dim * 10); // Keep top N words

        self.vocab.clear();
        for (i, (word, _)) in vocab.iter().enumerate() {
            self.vocab.insert(word.clone(), i);
        }

        // Compute IDF
        self.idf = vocab
            .iter()
            .map(|(_, df)| (total_docs / *df as f32).ln() + 1.0)
            .collect();
    }

    /// Embed a text into a fixed-dimension vector using TF-IDF + hash projection
    pub fn embed(&self, text: &str) -> Vec<f32> {
        let words = self.tokenize_text(text);
        let mut tf: HashMap<usize, f32> = HashMap::new();
        let total = words.len() as f32;

        for word in &words {
            if let Some(&idx) = self.vocab.get(word) {
                *tf.entry(idx).or_insert(0.0) += 1.0;
            }
        }

        // Build full TF-IDF vector in vocab space
        let mut vec: Vec<f32> = vec![0.0; self.vocab.len()];
        for (&idx, &count) in &tf {
            if idx < self.idf.len() {
                vec[idx] = (count / total) * self.idf[idx];
            }
        }

        // Project down to target dimension using random projection (deterministic hash)
        self.hash_project(&vec)
    }

    /// Cosine similarity between two vectors
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let mut dot = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;
        for (x, y) in a.iter().zip(b.iter()) {
            dot += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }
        let denom = norm_a.sqrt() * norm_b.sqrt();
        if denom > 0.0 {
            dot / denom
        } else {
            0.0
        }
    }

    /// Project high-dimensional TF-IDF vector down to `self.dim` using seeded hash
    fn hash_project(&self, vec: &[f32]) -> Vec<f32> {
        let mut result = vec![0.0f32; self.dim];
        for (i, &val) in vec.iter().enumerate() {
            if val == 0.0 {
                continue;
            }
            // Use index as seed for deterministic projection
            let hash = Self::simple_hash(i);
            let bucket = (hash % self.dim as u32) as usize;
            let sign = if hash % 2 == 0 { 1.0 } else { -1.0 };
            result[bucket] += val * sign;
        }
        // L2 normalize
        let norm: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in result.iter_mut() {
                *x /= norm;
            }
        }
        result
    }

    fn tokenize(text: &str) -> Vec<String> {
        let re = Regex::new(r"[a-zA-Z0-9_]+").unwrap();
        re.find_iter(&text.to_lowercase())
            .map(|m| m.as_str().to_string())
            .filter(|w| w.len() > 1)
            .collect()
    }

    /// Tokenize, then lemmatize each token when a Sema lexicon is attached.
    /// `umefikia` -> `fikia`, so agglutinative Swahili no longer fragments
    /// TF-IDF term statistics.
    fn tokenize_text(&self, text: &str) -> Vec<String> {
        match &self.lemmatizer {
            Some(lex) => Self::tokenize(text)
                .into_iter()
                .map(|w| lex.lookup(&w).map(|(e, _)| e.w.clone()).unwrap_or(w))
                .collect(),
            None => Self::tokenize(text),
        }
    }

    fn simple_hash(input: usize) -> u32 {
        let mut h = input as u32;
        h = h.wrapping_mul(0x45d9f3b);
        h ^= h >> 16;
        h = h.wrapping_mul(0x45d9f3b);
        h ^= h >> 16;
        h
    }
}

// ---- Vector Store ----

/// In-memory vector store with persistence
pub struct VectorStore {
    chunks: Vec<DocumentChunk>,
    embeddings: Vec<Vec<f32>>,
    embedder: LocalEmbedder,
}

impl VectorStore {
    pub fn new(dim: usize) -> Self {
        Self {
            chunks: Vec::new(),
            embeddings: Vec::new(),
            embedder: LocalEmbedder::new(dim),
        }
    }

    /// Attach/detach the Sema lemmatizer (re-fit happens on next ingest).
    pub fn set_lemmatizer(&mut self, lex: Option<Arc<Lexicon>>) {
        self.embedder.set_lemmatizer(lex);
    }

    /// Add a chunk with its pre-computed embedding
    pub fn add(&mut self, chunk: DocumentChunk, embedding: Vec<f32>) {
        self.chunks.push(chunk);
        self.embeddings.push(embedding);
    }

    /// Rebuild the TF-IDF vocabulary from all stored chunks
    pub fn rebuild_index(&mut self) {
        let corpus: Vec<String> = self.chunks.iter().map(|c| c.content.clone()).collect();
        self.embedder.fit(&corpus);
        self.embeddings.clear();
        for chunk in &self.chunks {
            let emb = self.embedder.embed(&chunk.content);
            self.embeddings.push(emb);
        }
    }

    /// Search for most similar chunks to a query
    pub fn search(&self, query: &str, top_k: usize) -> Vec<RagResult> {
        let query_emb = self.embedder.embed(query);

        let mut results: Vec<(usize, f32)> = self
            .embeddings
            .iter()
            .enumerate()
            .map(|(i, emb)| (i, LocalEmbedder::cosine_similarity(&query_emb, emb)))
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        results
            .into_iter()
            .take(top_k)
            .enumerate()
            .map(|(rank, (idx, sim))| RagResult {
                chunk: self.chunks[idx].clone(),
                similarity: sim,
                rank,
            })
            .collect()
    }

    pub fn stats(&self) -> RagStats {
        let total_tokens: usize = self.chunks.iter().map(|c| c.token_estimate).sum();
        let mem = std::mem::size_of_val(self.chunks.as_slice())
            + std::mem::size_of_val(self.embeddings.as_slice());
        RagStats {
            total_documents: 0, // computed elsewhere
            total_chunks: self.chunks.len(),
            total_tokens,
            last_ingest_time_ms: 0.0,
            memory_usage_mb: mem as f64 / 1_048_576.0,
        }
    }
}

// ---- Document Chunker ----

/// Split a document into overlapping chunks for RAG ingestion
pub fn chunk_document(
    content: &str,
    source_path: &str,
    chunk_size: usize,
    chunk_overlap: usize,
) -> Vec<DocumentChunk> {
    let _lines: Vec<&str> = content.lines().collect();
    let mut chunks = Vec::new();
    let words: Vec<&str> = content.split_whitespace().collect();

    if words.is_empty() {
        return chunks;
    }

    let mut start = 0;
    let mut chunk_idx = 0;

    while start < words.len() {
        let end = (start + chunk_size).min(words.len());
        let chunk_words = &words[start..end];
        let chunk_text = chunk_words.join(" ");

        // Find approximate line range
        let char_offset: usize = words[..start].iter().map(|w| w.len() + 1).sum();
        let line_start = content[..char_offset.min(content.len())]
            .lines()
            .count();
        let chunk_chars: usize = chunk_words.iter().map(|w| w.len() + 1).sum();
        let line_end = content[..(char_offset + chunk_chars).min(content.len())]
            .lines()
            .count();

        let ext = Path::new(source_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        chunks.push(DocumentChunk {
            id: Uuid::new_v4().to_string(),
            source_file: source_path.to_string(),
            chunk_index: chunk_idx,
            content: chunk_text.clone(),
            token_estimate: chunk_words.len(), // rough: 1 word ~= 1 token
            metadata: ChunkMetadata {
                file_extension: ext,
                file_size_bytes: content.len() as u64,
                line_start: Some(line_start),
                line_end: Some(line_end),
                language: None,
            },
        });

        chunk_idx += 1;
        start += chunk_size - chunk_overlap;
        if start + chunk_overlap >= words.len() {
            break;
        }
    }

    chunks
}

// ---- RAG Pipeline (full orchestration) ----

pub struct RagPipeline {
    pub store: VectorStore,
    pub documents: Vec<IngestedDocument>,
    pub config: RagConfig,
}

#[derive(Debug, Clone)]
pub struct RagConfig {
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub top_k: usize,
    pub similarity_threshold: f32,
    pub embedding_dim: usize,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            chunk_size: 500,
            chunk_overlap: 50,
            top_k: 5,
            similarity_threshold: 0.3,
            embedding_dim: 128,
        }
    }
}

impl RagPipeline {
    pub fn new(config: RagConfig) -> Self {
        Self {
            store: VectorStore::new(config.embedding_dim),
            documents: Vec::new(),
            config,
        }
    }

    /// Attach/detach the Sema lemmatizer used by the embedder.
    pub fn set_lemmatizer(&mut self, lex: Option<Arc<Lexicon>>) {
        self.store.set_lemmatizer(lex);
    }

    /// Ingest a text file into the vector store
    pub fn ingest_file(&mut self, path: &Path) -> Result<IngestedDocument, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        let file_str = path.to_string_lossy().to_string();

        // Compute simple hash for dedup
        let hash = {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            format!("{:x}", hasher.finish())
        };

        // Check for duplicate
        if self.documents.iter().any(|d| d.path == file_str && d.file_hash == hash) {
            return Err(format!("Document already ingested: {}", file_str));
        }

        let chunks = chunk_document(
            &content,
            &file_str,
            self.config.chunk_size,
            self.config.chunk_overlap,
        );

        let total_tokens: usize = chunks.iter().map(|c| c.token_estimate).sum();
        let chunks_count = chunks.len();

        // Add to store
        for chunk in chunks {
            let emb = self.store.embedder.embed(&chunk.content);
            self.store.add(chunk, emb);
        }

        // Rebuild index after ingestion
        self.store.rebuild_index();

        let doc = IngestedDocument {
            path: file_str,
            chunks_count,
            total_tokens,
            ingested_at: chrono::Utc::now().to_rfc3339(),
            file_hash: hash,
        };

        self.documents.push(doc.clone());
        Ok(doc)
    }

    /// Ingest all supported files from a directory
    pub fn ingest_directory(
        &mut self,
        dir: &Path,
        extensions: &[String],
    ) -> Result<Vec<IngestedDocument>, String> {
        let mut results = Vec::new();

        if !dir.exists() {
            return Ok(results);
        }

        let entries = std::fs::read_dir(dir)
            .map_err(|e| format!("Failed to read dir {}: {}", dir.display(), e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();
                let ext_with_dot = format!(".{}", ext);
                if extensions.contains(&ext_with_dot) || extensions.contains(&ext) {
                    match self.ingest_file(&path) {
                        Ok(doc) => results.push(doc),
                        Err(e) => tracing::warn!("Skipping {}: {}", path.display(), e),
                    }
                }
            }
        }

        Ok(results)
    }

    /// Query the RAG store and return formatted context for the LLM
    pub fn query(&self, question: &str) -> (String, Vec<RagResult>) {
        let results = self.store.search(question, self.config.top_k);

        let filtered: Vec<&RagResult> = results
            .iter()
            .filter(|r| r.similarity >= self.config.similarity_threshold)
            .collect();

        if filtered.is_empty() {
            return (String::new(), results);
        }

        let mut context = String::from("Relevant context from local documents:\n\n");
        for (i, result) in filtered.iter().enumerate() {
            context.push_str(&format!(
                "[{}] (similarity: {:.2}, source: {})\n{}\n\n",
                i + 1,
                result.similarity,
                result.chunk.source_file,
                result.chunk.content,
            ));
        }

        (context, results)
    }

    pub fn stats(&self) -> RagStats {
        let mut s = self.store.stats();
        s.total_documents = self.documents.len();
        s
    }
}
