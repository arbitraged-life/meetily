// Speaker embedding extraction using ONNX Runtime (WeSpeaker ResNet34)
// Input: audio segment (f32 samples, 16kHz mono)
// Output: 256-dim speaker embedding vector

use log::info;
use ort::inputs;
use ort::session::Session;
use ort::value::TensorRef;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

/// WeSpeaker-based speaker embedding extractor
pub struct EmbeddingExtractor {
    session: Arc<Mutex<Session>>,
    sample_rate: u32,
}

impl EmbeddingExtractor {
    /// Load the ONNX model
    pub fn new(model_path: &Path) -> Result<Self, String> {
        let session = Session::builder()
            .map_err(|e| format!("Failed to create ONNX session builder: {}", e))?
            .commit_from_file(model_path)
            .map_err(|e| format!("Failed to load model {}: {}", model_path.display(), e))?;

        info!("🎤 Speaker embedding model loaded from {}", model_path.display());

        Ok(Self {
            session: Arc::new(Mutex::new(session)),
            sample_rate: 16000,
        })
    }

    /// Extract speaker embedding from audio samples
    /// Audio must be 16kHz mono f32
    pub async fn extract_embedding(&self, audio: &[f32]) -> Result<Vec<f32>, String> {
        if audio.len() < (self.sample_rate as usize / 2) {
            return Err("Audio too short for embedding (need at least 0.5s)".to_string());
        }

        let mut session = self.session.lock().await;

        // WeSpeaker expects [batch, samples] shape
        let audio_len = audio.len();
        let input_array = ndarray::Array2::from_shape_vec((1, audio_len), audio.to_vec())
            .map_err(|e| format!("Failed to create input array: {}", e))?;

        let input_inputs = inputs![
            "input" => TensorRef::from_array_view(input_array.view())
                .map_err(|e| format!("TensorRef error: {}", e))?,
        ];

        let outputs = session
            .run(input_inputs)
            .map_err(|e| format!("Inference error: {}", e))?;

        // Output is [1, embedding_dim] — extract as Vec<f32>
        let (_, output) = outputs
            .iter()
            .next()
            .ok_or("No output from model")?;

        let embedding: Vec<f32> = output
            .try_extract_array::<f32>()
            .map_err(|e| format!("Failed to extract embedding: {}", e))?
            .iter()
            .cloned()
            .collect();

        // L2 normalize the embedding
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            Ok(embedding.iter().map(|x| x / norm).collect())
        } else {
            Ok(embedding)
        }
    }

    /// Compute cosine similarity between two embeddings
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a > 0.0 && norm_b > 0.0 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }
}
