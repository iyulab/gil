//! The similar document layer's vote, shared by suggestions and by the replays thresholds are chosen on, so the score a
//! threshold is chosen on is the score it is compared with.

use crate::lexical_memory::Match;

/// How many of `nearest` vote when `votes` may: the fewer of the two.
pub fn voters(nearest: &[Match], votes: usize) -> usize {
    nearest.len().min(votes)
}

/// The value the nearest `votes` documents vote for, each with its similarity (a negative one counts as none): the
/// nearest document that voted for it, and the vote's margin — the winner's weight less the runner-up's, over all the
/// weight cast; 0 when none was. A tie goes to the value whose nearest voter is nearer. With a single voter the score is
/// its similarity. `None` when there are no documents.
pub fn decide(nearest: &[Match], votes: usize) -> Option<(&Match, f64)> {
    let first = nearest.first()?;
    if votes == 1 || nearest.len() == 1 {
        return Some((first, first.similarity));
    }

    // Values enter the tally in order of their nearest voter.
    let mut tally: Vec<(&Match, f64)> = Vec::new();
    for m in nearest.iter().take(votes) {
        let weight = m.similarity.max(0.0);
        match tally.iter_mut().find(|t| t.0.answer == m.answer) {
            Some(entry) => entry.1 += weight,
            None => tally.push((m, weight)),
        }
    }

    // Stable, so a tie keeps the nearer one first; the total is summed in this order.
    tally.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let total = tally.iter().fold(0.0, |sum, t| sum + t.1);
    let runner_up = tally.get(1).map_or(0.0, |t| t.1);
    let score = if total > 0.0 {
        (tally[0].1 - runner_up) / total
    } else {
        0.0
    };
    Some((tally[0].0, score))
}
