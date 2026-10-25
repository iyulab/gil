//! The similar record layer's vote, shared by suggestions and by the replays thresholds are chosen on, so the score a
//! threshold is chosen on is the score it is compared with.

use crate::lexical_memory::Match;

/// How many of `nearest` vote when `votes` may: the fewer of the two.
pub fn voters(nearest: &[Match], votes: usize) -> usize {
    nearest.len().min(votes)
}

/// The value the nearest `votes` records vote for, each with its similarity (a negative one counts as none): the
/// nearest record that voted for it, and the vote's margin — the winner's weight less the runner-up's, over all the
/// weight cast; 0 when none was. A tie goes to the value whose nearest voter is nearer. With a single voter the score is
/// its similarity. `None` when there are no records.
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

/// Experimental. What a record that settled several values is remembered by in the similar record memory: its elements
/// joined by this, which no element may hold.
pub const ELEMENT_SEPARATOR: char = '\u{1f}';

/// Experimental. The elements of a settled set as the similar record memory keeps them: once each, in scalar order,
/// leaving out an empty one and one holding [`ELEMENT_SEPARATOR`], which the memory could not tell apart.
pub fn elements_of(set: Option<&Vec<String>>) -> Vec<String> {
    let mut elements: Vec<String> = set
        .into_iter()
        .flatten()
        .filter(|e| !e.is_empty() && !e.contains(ELEMENT_SEPARATOR))
        .cloned()
        .collect();
    elements.sort();
    elements.dedup();
    elements
}

/// Experimental. The elements joined for the similar record memory; `None` for a set with none.
pub fn join_elements(elements: &[String]) -> Option<String> {
    (!elements.is_empty()).then(|| elements.join(&ELEMENT_SEPARATOR.to_string()))
}

/// Experimental. Each element the nearest `votes` records hold, with its share: the similarity of the records holding it
/// (a negative one counts as none) over the similarity of all the records that voted. Each element stands on its own — a
/// record holding several votes for each of them in full. Most shared first, then in scalar order; empty when there are
/// no records or none is at all similar.
pub fn element_shares(nearest: &[Match], votes: usize) -> Vec<(String, f64)> {
    let mut total = 0.0_f64;
    let mut tally: std::collections::BTreeMap<&str, f64> = std::collections::BTreeMap::new();
    for m in nearest.iter().take(votes) {
        let weight = m.similarity.max(0.0);
        total += weight;
        for element in m.answer.split(ELEMENT_SEPARATOR).filter(|e| !e.is_empty()) {
            *tally.entry(element).or_default() += weight;
        }
    }

    if total <= 0.0 {
        return Vec::new();
    }

    let mut shares: Vec<(String, f64)> = tally
        .into_iter()
        .map(|(element, held)| (element.to_owned(), held / total))
        .collect();
    // Stable over the scalar order the tally keeps.
    shares.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    shares
}
