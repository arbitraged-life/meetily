// Speaker diarization module — identifies who said what using ONNX embedding models
// Uses a segmentation + embedding + clustering pipeline similar to pyannote

pub mod commands;
pub mod embeddings;
pub mod clustering;
pub mod speaker_db;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A speaker identified in a meeting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Speaker {
    pub id: String,
    pub label: String, // User-assigned name or "Speaker 1"
    pub embedding: Vec<f32>, // Average embedding vector (256-dim for wespeaker resnet34-LM)
    pub sample_count: u32, // Number of segments used to compute avg embedding
}

/// A transcript segment with speaker attribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiarizedSegment {
    pub text: String,
    pub speaker_id: String,
    pub speaker_label: String,
    pub start_time: f64,
    pub end_time: f64,
    pub confidence: f32,
    pub embedding: Vec<f32>,
}

/// Configuration for the diarization engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiarizationConfig {
    /// Whether diarization is enabled
    pub enabled: bool,
    /// Min segment duration for embedding extraction (seconds)
    pub min_segment_duration: f64,
    /// Cosine similarity threshold for same-speaker (0.0 - 1.0)
    pub similarity_threshold: f32,
    /// Max number of speakers to detect (0 = unlimited)
    pub max_speakers: usize,
    /// Path to the ONNX embedding model
    pub model_path: Option<PathBuf>,
}

impl Default for DiarizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_segment_duration: 1.0,
            similarity_threshold: 0.75,
            max_speakers: 0,
            model_path: None,
        }
    }
}

/// Runtime state for the diarization engine
pub struct DiarizationState {
    pub config: DiarizationConfig,
    /// Known speakers from this meeting
    pub speakers: Vec<Speaker>,
    /// Persistent speaker database (cross-meeting)
    pub known_speakers: Vec<Speaker>,
    /// Segments awaiting attribution
    pub pending_segments: Vec<DiarizedSegment>,
    /// Embedding extractor (None if model not available)
    pub embedder: Option<embeddings::EmbeddingExtractor>,
}

impl DiarizationState {
    pub fn new(config: DiarizationConfig) -> Self {
        Self {
            config,
            speakers: Vec::new(),
            known_speakers: Vec::new(),
            pending_segments: Vec::new(),
            embedder: None,
        }
    }
}

/// Get the models directory for diarization models
pub fn models_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("com.meetily.ai")
        .join("models")
        .join("diarization")
}

/// Check if the embedding model is available
pub fn is_model_available() -> bool {
    let model_path = models_dir().join("wespeaker_en_voxceleb_resnet34.onnx");
    model_path.exists()
}
