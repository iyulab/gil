//! Chooses a judged field's key threshold by replaying settled documents: each document, in settlement order, is asked
//! about with the values that had arrived before the field, against a field memory holding the documents settled before
//! it, then remembered. Precision is fitted as a non-decreasing function of the score, and the threshold is the lowest
//! score down to which every fitted band of answers meets the target precision.
//!
//! Every sum here is taken in the order the reference implementation takes it — the same operations on the same operands
//! — so the numbers come out the same to the bit. Not yet supported by this implementation: values a field must not
//! miss, typed thresholds and the similar document layer.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::field_memory::FieldMemory;
use crate::form::{Document, Form, Role, SettlementKind};

/// How many of the latest documents replayed [`ThresholdChoice::promise`] is taken over — every document in settlement
/// order counts, whether or not it was asked about.
pub const RECENT_DOCUMENTS: usize = 5_000;

/// A threshold chosen for one memory layer of a field, and how it did on the replay it was chosen on. A checked value
/// ([`SettlementKind::Check`]) counts as one ÷ its draw probability lookups, so `precision` and `answer_rate` are
/// weighted, while `answered` and `lookups` count the lookups made.
#[derive(Clone, Debug, PartialEq)]
pub struct ThresholdChoice {
    /// The lowest score down to which every band of answers met the target precision.
    pub threshold: f64,
    /// The share of answers at or above the threshold that matched the settled value.
    pub precision: f64,
    /// The share of lookups answered at or above the threshold.
    pub answer_rate: f64,
    /// How many lookups were answered.
    pub answered: usize,
    /// How many lookups the replay made.
    pub lookups: usize,
    /// The answers with their weights.
    pub answered_weight: f64,
    /// How many independent answers the weighted precision is worth: the squared sum of the weights over the sum of
    /// their squares.
    pub effective_answered: f64,
    /// For the similar document layer only; always `None` for the key layer.
    pub similarity_floor: Option<f64>,
    /// The precision to expect of the next answers: the precision of the answers at or above the threshold among the
    /// latest [`RECENT_DOCUMENTS`] documents replayed (or `precision` when those are worth fewer than the minimum number
    /// of answers), lowered to the bottom of its Wilson score interval at one standard error.
    pub promise: f64,
    /// How many independent answers `promise` is worth.
    pub promise_answered: f64,
}

/// How the key layer did under one key: the lookups whose best value this key backed most strongly, and how the chosen
/// threshold treated them.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyState {
    /// The key, as a suggestion's evidence names it — `field: value`.
    pub key: String,
    /// How many lookups this key backed the best value of.
    pub lookups: usize,
    /// How many of them the chosen threshold answered; 0 when none was chosen.
    pub answered: usize,
    /// The share of those answers that matched, weighted; `None` when none was answered.
    pub precision: Option<f64>,
    /// Whether the chosen threshold answered this key's latest lookup.
    pub answering: bool,
    /// The answers with their weights.
    pub answered_weight: f64,
    /// How many independent answers `precision` is worth.
    pub effective_answered: f64,
}

/// What replaying settled documents found for one memory layer of a field.
#[derive(Clone, Debug, PartialEq)]
pub struct ThresholdReplay {
    /// The lowest threshold down to which every band of answers met the target precision; `None` when none does.
    pub chosen: Option<ThresholdChoice>,
    /// The highest threshold that gathers at least the minimum number of answers; `None` when no score does.
    pub most_precise: Option<ThresholdChoice>,
    /// How many lookups the replay made.
    pub lookups: usize,
    /// How many of them found a candidate at any score.
    pub candidates: usize,
    /// For the key layer of a field that takes one value, the state under each key that backed a best value, most
    /// lookups first, then by key in scalar order.
    pub keys: Option<Vec<KeyState>>,
}

/// One answer the layer would give at some threshold: its score, whether it matched, how many decisions it counts as,
/// the place of its document in settlement order (from 1, counting every document), the key that backed it most
/// strongly, and whether a value the field must not miss held the key layer back from it.
#[derive(Clone, Debug)]
struct Answer {
    score: f64,
    correct: bool,
    weight: f64,
    order: usize,
    key: Option<String>,
    caught: bool,
}

/// A band of answers the fit pooled: its lowest score, how many answers, their weight, the weight of the right ones, and
/// the sum of the squared weights.
#[derive(Clone, Copy, Debug)]
struct Block {
    lowest: f64,
    count: usize,
    weight: f64,
    correct: f64,
    squares: f64,
}

impl Block {
    fn precision(&self) -> f64 {
        self.correct / self.weight
    }
}

/// Chooses `field`'s key threshold by replaying `documents` through `memory`, which must be empty and configured as the
/// one in use. Documents are replayed in settlement order (then by id in scalar order), each without its earlier version
/// as evidence; a value the application filled is neither asked about nor remembered. `counting` names the kinds of
/// settlement whose lookups are counted — `None` counts every settlement, including those not known.
///
/// A field that takes several values trusts each value on its own score, so every value offered under the keys counts,
/// right when it is one of the settled values not yet chosen. A person picks such values one after another, and those
/// chosen are evidence for the rest, so each document is asked about once with none of its values chosen, then again
/// after each but the last is chosen, in [`pick_order`]. `lookups` then counts the values sought, and the replay carries
/// no state under each key.
///
/// # Panics
/// When `target_precision` is outside (0, 1], `minimum_answered` is 0, or the field is not judged.
pub fn select_key_threshold(
    memory: &mut FieldMemory,
    form: &Form,
    field: &str,
    documents: &[Document],
    target_precision: f64,
    minimum_answered: usize,
    counting: Option<&[SettlementKind]>,
) -> ThresholdReplay {
    assert!(
        target_precision > 0.0 && target_precision <= 1.0,
        "target_precision must be in (0, 1]"
    );
    assert!(minimum_answered >= 1, "minimum_answered must be at least 1");
    let definition = form.field(field);
    assert!(
        definition.role == Role::Judged,
        "'{field}' is not a judged field"
    );
    let multiple = definition.multiple;

    let mut ordered: Vec<Document> = documents.iter().map(Document::remembered).collect();
    ordered.sort_by(|a, b| a.settled_at.cmp(&b.settled_at).then(a.id.cmp(&b.id)));

    let mut matches: Vec<Answer> = Vec::new();
    let (mut lookups, mut lookup_weight, mut order) = (0_usize, 0.0_f64, 0_usize);
    for document in &ordered {
        order += 1;
        let settled_set = if multiple {
            pick_order(document, field)
        } else {
            Vec::new()
        };
        let settled = document.values.get(field);
        let nothing_settled = if multiple {
            settled_set.is_empty()
        } else {
            settled.is_none()
        };
        if nothing_settled {
            continue;
        }

        memory.remove(&form.name, &document.id); // a document settled again is not its own evidence
        let weight = weight(document, field, counting);
        if memory.count(&form.name) > 0 && weight > 0.0 {
            let before = arrived(form, document, field, &document.values);
            let before_sets = arrived(form, document, field, &document.sets);
            if let Some(settled) = settled.filter(|_| !multiple) {
                lookups += 1;
                lookup_weight += weight;
                if let Some(first) = memory.first(form, field, &before, settled, &before_sets) {
                    matches.push(Answer {
                        score: first.score,
                        correct: first.matches,
                        weight,
                        order,
                        key: Some(first.key),
                        caught: first.caught,
                    });
                }
            } else {
                for chosen in 0..settled_set.len() {
                    let mut sets = before_sets.clone();
                    sets.insert(field.to_owned(), settled_set[..chosen].to_vec());
                    let mut sought: Vec<&String> = settled_set[chosen..].iter().collect();
                    sought.sort();
                    sought.dedup();
                    lookups += sought.len();
                    lookup_weight += sought.len() as f64 * weight;
                    for (value, score) in memory.scored(form, field, &before, &sets) {
                        matches.push(Answer {
                            score,
                            correct: sought.contains(&&value),
                            weight,
                            order,
                            key: None,
                            caught: false,
                        });
                    }
                }
            }
        }

        memory.put(form, document);
    }

    let uncaught: Vec<Answer> = matches.iter().filter(|m| !m.caught).cloned().collect();
    let replay = fit_recent(
        &uncaught,
        lookups,
        lookup_weight,
        order,
        target_precision,
        minimum_answered,
    );
    if multiple {
        replay
    } else {
        with_keys(replay, &matches)
    }
}

/// A set field's settled values, once each, in the order a replay supposes they were picked: by the 64-bit FNV-1a hash
/// of the UTF-8 of the document id, U+001F and the value, then in scalar order — so it depends neither on the order the
/// values are listed in nor on their scalar order.
pub fn pick_order(document: &Document, field: &str) -> Vec<String> {
    let mut values: Vec<&String> = Vec::new();
    for value in document.sets.get(field).into_iter().flatten() {
        if !values.contains(&value) {
            values.push(value);
        }
    }
    let mut keyed: Vec<(u64, &String)> = values
        .into_iter()
        .map(|v| (fnv1a(&format!("{}\u{1f}{v}", document.id)), v))
        .collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)));
    keyed.into_iter().map(|(_, v)| v.clone()).collect()
}

/// A stable 64-bit FNV-1a hash of the text's UTF-8 bytes.
fn fnv1a(text: &str) -> u64 {
    text.bytes()
        .fold(14_695_981_039_346_656_037_u64, |hash, b| {
            (hash ^ u64::from(b)).wrapping_mul(1_099_511_628_211)
        })
}

/// How many decisions the field's settlement in the document counts as: none when its kind is not counted (every kind is
/// when none are named), one ÷ the draw probability for a check, one otherwise.
fn weight(document: &Document, field: &str, counting: Option<&[SettlementKind]>) -> f64 {
    let kind = document.settlements.get(field).copied();
    if let Some(counting) = counting
        && !kind.is_some_and(|k| counting.contains(&k))
    {
        return 0.0;
    }

    match (kind, document.probabilities.get(field)) {
        (Some(SettlementKind::Check), Some(probability)) => 1.0 / probability,
        _ => 1.0,
    }
}

/// The document's values that had arrived before `field` was settled — what its last suggestion was made from. With no
/// arrival order, the observed values count as there from the start and the judged ones as settled in the form's order;
/// with one, a field it leaves out counts as there from the start.
fn arrived<T: Clone>(
    form: &Form,
    document: &Document,
    field: &str,
    values: &BTreeMap<String, T>,
) -> BTreeMap<String, T> {
    let rank = |name: &str| -> i64 {
        if let Some(arrival) = &document.arrival {
            return match arrival.iter().position(|n| n == name) {
                Some(at) => at as i64,
                None if name == field => i64::MAX,
                None => -1,
            };
        }

        // A value of no field of the form is no evidence anyway.
        match form.fields.iter().position(|f| f.name == name) {
            Some(index) if form.fields[index].role != Role::Observed => index as i64,
            _ => -1,
        }
    };

    let own = rank(field);
    values
        .iter()
        .filter(|(name, _)| name.as_str() != field && rank(name) < own)
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/// [`fit`], then each choice's promise over the answers among the latest [`RECENT_DOCUMENTS`] documents of `latest`.
fn fit_recent(
    matches: &[Answer],
    lookups: usize,
    lookup_weight: f64,
    latest: usize,
    target_precision: f64,
    minimum_answered: usize,
) -> ThresholdReplay {
    let replay = fit(
        matches,
        lookups,
        lookup_weight,
        target_precision,
        minimum_answered,
    );
    let recent: Vec<&Answer> = matches
        .iter()
        .filter(|m| m.order as i64 > latest as i64 - RECENT_DOCUMENTS as i64)
        .collect();
    let promised = |choice: Option<ThresholdChoice>| -> Option<ThresholdChoice> {
        let choice = choice?;
        let (mut weight, mut correct, mut squares) = (0.0_f64, 0.0_f64, 0.0_f64);
        for m in recent.iter().filter(|m| m.score >= choice.threshold) {
            weight += m.weight;
            correct += if m.correct { m.weight } else { 0.0 };
            squares += m.weight * m.weight;
        }

        let effective = if weight > 0.0 {
            weight * weight / squares
        } else {
            0.0
        };
        Some(if effective >= minimum_answered as f64 {
            ThresholdChoice {
                promise: lower(correct / weight, effective),
                promise_answered: effective,
                ..choice
            }
        } else {
            ThresholdChoice {
                promise: lower(choice.precision, choice.effective_answered),
                promise_answered: choice.effective_answered,
                ..choice
            }
        })
    };

    ThresholdReplay {
        chosen: promised(replay.chosen),
        most_precise: promised(replay.most_precise),
        ..replay
    }
}

/// The lower end of the Wilson score interval at one standard error for a precision measured on `answered` independent
/// answers.
fn lower(precision: f64, answered: f64) -> f64 {
    if answered <= 0.0 {
        return precision;
    }

    let centre = precision + (1.0 / (2.0 * answered));
    let margin =
        ((precision * (1.0 - precision) / answered) + (1.0 / (4.0 * answered * answered))).sqrt();
    (centre - margin) / (1.0 + (1.0 / answered))
}

/// The state under each key that backed a best value: its lookups, and what `replay`'s chosen threshold made of them. A
/// caught lookup is the key's, but never answered.
fn with_keys(replay: ThresholdReplay, answers: &[Answer]) -> ThresholdReplay {
    let threshold = replay.chosen.as_ref().map(|c| c.threshold);
    let answers_it = |a: &Answer| !a.caught && threshold.is_some_and(|t| a.score >= t);

    // Grouped by key in the order the keys first come, each group's answers in replay order.
    let mut groups: Vec<(&str, Vec<&Answer>)> = Vec::new();
    for answer in answers {
        let Some(key) = answer.key.as_deref() else {
            continue;
        };
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, group)) => group.push(answer),
            None => groups.push((key, vec![answer])),
        }
    }

    let mut keys: Vec<KeyState> = groups
        .into_iter()
        .map(|(key, group)| {
            let answered: Vec<&Answer> = group.iter().copied().filter(|a| answers_it(a)).collect();
            let weight = answered.iter().fold(0.0, |sum, a| sum + a.weight);
            let squares = answered
                .iter()
                .fold(0.0, |sum, a| sum + a.weight * a.weight);
            let correct = answered
                .iter()
                .filter(|a| a.correct)
                .fold(0.0, |sum, a| sum + a.weight);
            // The latest lookup: the first with the greatest order.
            let latest = group
                .iter()
                .copied()
                .reduce(|best, a| if a.order > best.order { a } else { best })
                .expect("a group has an answer");
            KeyState {
                key: key.to_owned(),
                lookups: group.len(),
                answered: answered.len(),
                precision: (weight > 0.0).then(|| correct / weight),
                answering: answers_it(latest),
                answered_weight: weight,
                effective_answered: if weight > 0.0 {
                    weight * weight / squares
                } else {
                    0.0
                },
            }
        })
        .collect();
    keys.sort_by(|a, b| b.lookups.cmp(&a.lookups).then(a.key.cmp(&b.key)));
    ThresholdReplay {
        keys: Some(keys),
        ..replay
    }
}

/// Fits precision as a non-decreasing function of the score (isotonic regression by pooling adjacent violators), then
/// walks the fitted blocks from the highest score down while each block's own precision meets the target. The threshold
/// is the lowest score of the last such block; the most precise is the first block, from the top, whose answers above it
/// are worth `minimum_answered`.
fn fit(
    matches: &[Answer],
    lookups: usize,
    lookup_weight: f64,
    target_precision: f64,
    minimum_answered: usize,
) -> ThresholdReplay {
    // Answers of one score form a group, in replay order; groups go from the lowest score up.
    let mut sorted: Vec<&Answer> = matches.iter().collect();
    sorted.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(Ordering::Equal));

    let mut blocks: Vec<Block> = Vec::new();
    for group in sorted.chunk_by(|a, b| a.score == b.score) {
        let mut block = Block {
            lowest: group[0].score,
            count: group.len(),
            weight: group.iter().fold(0.0, |sum, m| sum + m.weight),
            correct: group
                .iter()
                .filter(|m| m.correct)
                .fold(0.0, |sum, m| sum + m.weight),
            squares: group.iter().fold(0.0, |sum, m| sum + m.weight * m.weight),
        };
        while let Some(below) = blocks.last().copied()
            && below.precision() >= block.precision()
        {
            blocks.pop();
            block = Block {
                lowest: below.lowest,
                count: below.count + block.count,
                weight: below.weight + block.weight,
                correct: below.correct + block.correct,
                squares: below.squares + block.squares,
            };
        }

        blocks.push(block);
    }

    let mut chosen: Option<ThresholdChoice> = None;
    let mut most_precise: Option<ThresholdChoice> = None;
    let mut meets = true;
    let (mut count, mut weight, mut correct, mut squares) = (0_usize, 0.0_f64, 0.0_f64, 0.0_f64);
    for block in blocks.iter().rev() {
        if !meets && most_precise.is_some() {
            break;
        }

        count += block.count;
        weight += block.weight;
        correct += block.correct;
        squares += block.squares;
        meets &= block.precision() >= target_precision;
        let effective = weight * weight / squares;
        if effective >= minimum_answered as f64 {
            let threshold = ThresholdChoice {
                threshold: block.lowest,
                precision: correct / weight,
                answer_rate: weight / lookup_weight,
                answered: count,
                lookups,
                answered_weight: weight,
                effective_answered: effective,
                similarity_floor: None,
                promise: correct / weight,
                promise_answered: count as f64,
            };
            if most_precise.is_none() {
                most_precise = Some(threshold.clone());
            }
            if meets {
                chosen = Some(threshold);
            }
        }
    }

    ThresholdReplay {
        chosen,
        most_precise,
        lookups,
        candidates: matches.len(),
        keys: None,
    }
}

#[cfg(test)]
mod tests {
    use super::fnv1a;

    #[test]
    fn fnv1a_matches_the_published_vectors() {
        assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a("a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a("foobar"), 0x85944171f73967e8);
    }
}
