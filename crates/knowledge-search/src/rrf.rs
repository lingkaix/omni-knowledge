//! Reciprocal rank fusion over already ordered file lists.

/// Fuse 1-based distinct file ranks. Each list votes once per source id.
/// Tied scores sort by source id. Raw scores are not compared across lists.
pub fn fuse_ranks(lists: &[Vec<String>]) -> Vec<(String, f32)> {
    let mut scores: Vec<(String, f32)> = Vec::new();
    for list in lists {
        let mut seen = Vec::new();
        for id in list {
            if seen.iter().any(|existing: &String| existing == id) {
                continue;
            }
            seen.push(id.clone());
            let rank = seen.len() as f32;
            if let Some(entry) = scores.iter_mut().find(|(existing, _)| existing == id) {
                entry.1 += 1.0 / (60.0 + rank);
            } else {
                scores.push((id.clone(), 1.0 / (60.0 + rank)));
            }
        }
    }
    scores.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
    scores
}
