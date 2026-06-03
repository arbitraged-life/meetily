// Speaker clustering — assigns segments to speakers using embeddings

use super::Speaker;
use super::embeddings::EmbeddingExtractor;
use log::info;
use uuid::Uuid;

/// Assign a segment to an existing speaker or create a new one
pub fn assign_speaker(
    embedding: &[f32],
    speakers: &mut Vec<Speaker>,
    threshold: f32,
) -> (String, String) {
    // Find best matching speaker
    let mut best_sim = 0.0f32;
    let mut best_idx = None;

    for (idx, speaker) in speakers.iter().enumerate() {
        let sim = EmbeddingExtractor::cosine_similarity(embedding, &speaker.embedding);
        if sim > best_sim {
            best_sim = sim;
            best_idx = Some(idx);
        }
    }

    if best_sim >= threshold {
        let idx = best_idx.unwrap();
        // Update the speaker's average embedding (running average)
        let speaker = &mut speakers[idx];
        let n = speaker.sample_count as f32;
        let new_n = n + 1.0;
        speaker.embedding = speaker
            .embedding
            .iter()
            .zip(embedding.iter())
            .map(|(old, new)| (old * n + new) / new_n)
            .collect();
        speaker.sample_count += 1;

        // Re-normalize
        let norm: f32 = speaker.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            speaker.embedding = speaker.embedding.iter().map(|x| x / norm).collect();
        }

        (speaker.id.clone(), speaker.label.clone())
    } else {
        // New speaker
        let speaker_num = speakers.len() + 1;
        let new_speaker = Speaker {
            id: Uuid::new_v4().to_string(),
            label: format!("Speaker {}", speaker_num),
            embedding: embedding.to_vec(),
            sample_count: 1,
        };
        let id = new_speaker.id.clone();
        let label = new_speaker.label.clone();
        speakers.push(new_speaker);
        info!("🆕 New speaker detected: {} (total: {})", label, speakers.len());
        (id, label)
    }
}

/// Merge two speakers (when user identifies them as same person)
pub fn merge_speakers(speakers: &mut Vec<Speaker>, keep_id: &str, merge_id: &str) -> bool {
    let merge_idx = speakers.iter().position(|s| s.id == merge_id);
    let keep_idx = speakers.iter().position(|s| s.id == keep_id);

    if let (Some(keep_i), Some(merge_i)) = (keep_idx, merge_idx) {
        let merge_embedding = speakers[merge_i].embedding.clone();
        let merge_count = speakers[merge_i].sample_count;

        let keeper = &mut speakers[keep_i];
        let total = keeper.sample_count + merge_count;
        let kn = keeper.sample_count as f32;
        let mn = merge_count as f32;

        keeper.embedding = keeper
            .embedding
            .iter()
            .zip(merge_embedding.iter())
            .map(|(k, m)| (k * kn + m * mn) / (kn + mn))
            .collect();
        keeper.sample_count = total;

        // Normalize
        let norm: f32 = keeper.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            keeper.embedding = keeper.embedding.iter().map(|x| x / norm).collect();
        }

        speakers.remove(merge_i);
        true
    } else {
        false
    }
}
