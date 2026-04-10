use std::collections::HashSet;

pub(super) fn similarity(left: &HashSet<String>, right: &HashSet<String>) -> f32 {
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

    let mut intersection = 0usize;
    for token in small {
        if large.contains(token) {
            intersection += 1;
        }
    }
    let union = left.len() + right.len() - intersection;
    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}
