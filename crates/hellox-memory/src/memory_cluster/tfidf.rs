use std::collections::HashMap;

pub(super) fn build_vectors(docs: &[&HashMap<String, u32>]) -> Vec<HashMap<String, f32>> {
    if docs.is_empty() {
        return Vec::new();
    }

    let doc_count = docs.len() as f32;
    let mut df: HashMap<String, usize> = HashMap::new();
    for doc in docs {
        for token in doc.keys() {
            *df.entry(token.clone()).or_insert(0) += 1;
        }
    }

    let mut vectors = Vec::with_capacity(docs.len());
    for doc in docs {
        let mut weights: HashMap<String, f32> = HashMap::with_capacity(doc.len());
        let mut sum_sq = 0.0_f32;

        for (token, count) in *doc {
            let df_value = df.get(token).copied().unwrap_or(0) as f32;
            let idf = ((1.0 + doc_count) / (1.0 + df_value)).ln() + 1.0;
            let tf = *count as f32;
            let weight = tf * idf;
            if weight == 0.0 {
                continue;
            }
            sum_sq += weight * weight;
            weights.insert(token.clone(), weight);
        }

        let norm = sum_sq.sqrt();
        if norm > 0.0 {
            for weight in weights.values_mut() {
                *weight /= norm;
            }
        }

        vectors.push(weights);
    }

    vectors
}

pub(super) fn cosine_similarity(left: &HashMap<String, f32>, right: &HashMap<String, f32>) -> f32 {
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }

    let (small, large) = if left.len() <= right.len() {
        (left, right)
    } else {
        (right, left)
    };

    let mut dot = 0.0_f32;
    for (token, weight) in small {
        if let Some(other) = large.get(token) {
            dot += weight * other;
        }
    }
    dot
}
