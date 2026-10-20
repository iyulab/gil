//! An attribute's coarse level as the two memory layers back it, shared by suggestions and by the replays its thresholds are
//! chosen on.

use crate::lexical_memory::Match;
use crate::schema::CoarseLevel;

/// The class the values under the record's keys back most: each value's score — never below 0 — added to its
/// class's, the classes in order of their best-ranked value. The score is that sum, on the key layer's scale, and the
/// evidence the key backing its best value. A tie goes to the class of the better-ranked value. `None` when no value is
/// under the keys.
pub fn keys(keyed: &[(String, f64, String)], level: &CoarseLevel) -> Option<(String, f64, String)> {
    let mut groups: Vec<(String, f64, String)> = Vec::new();
    for (value, score, evidence) in keyed {
        let named = level.of(value);
        match groups.iter_mut().find(|g| g.0 == named) {
            Some(group) => group.1 += score.max(0.0),
            None => groups.push((named, score.max(0.0), evidence.clone())),
        }
    }

    // The first of the highest: a tie keeps the better-ranked.
    groups
        .into_iter()
        .reduce(|best, g| if g.1 > best.1 { g } else { best })
}

/// The class the nearest `votes` records vote for, as [`crate::similar_vote::decide`] votes for a value: each with its
/// similarity, the score the vote's margin over classes, or with a single voter its similarity. The match is the nearest
/// record that voted for the class — its similarity is what the level's similarity floor is compared with when more
/// than one voted (the flag). `None` when there are no records.
pub fn vote(
    nearest: &[Match],
    votes: usize,
    level: &CoarseLevel,
) -> Option<(String, f64, Match, bool)> {
    let first = nearest.first()?;
    let voters = &nearest[..nearest.len().min(votes)];
    if voters.len() == 1 {
        return Some((
            level.of(&first.answer),
            first.similarity,
            first.clone(),
            false,
        ));
    }

    // Classes enter the tally in order of their nearest voter.
    let mut tally: Vec<(String, &Match, f64)> = Vec::new();
    for m in voters {
        let named = level.of(&m.answer);
        let weight = m.similarity.max(0.0);
        match tally.iter_mut().find(|t| t.0 == named) {
            Some(entry) => entry.2 += weight,
            None => tally.push((named, m, weight)),
        }
    }

    // Stable, so a tie keeps the nearer one first; the total is summed in this order.
    tally.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    let total = tally.iter().fold(0.0, |sum, t| sum + t.2);
    let runner_up = tally.get(1).map_or(0.0, |t| t.2);
    let score = if total > 0.0 {
        (tally[0].2 - runner_up) / total
    } else {
        0.0
    };
    let (class, nearest, _) = tally.swap_remove(0);
    Some((class, score, nearest.clone(), true))
}
