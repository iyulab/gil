//! Chooses a judged field's key threshold by replaying settled documents: each document, in settlement order, is asked
//! about with the values that had arrived before the field, against a field memory holding the documents settled before
//! it, then remembered. Precision is fitted as a non-decreasing function of the score, and the threshold is the lowest
//! score down to which every fitted band of answers meets the target precision.
//!
//! Every sum here is taken in the order the reference implementation takes it — the same operations on the same operands
//! — so the numbers come out the same to the bit. Not yet supported by this implementation: values a field must not
//! miss, typed thresholds and the similar document layer.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use crate::field_memory::{FieldMemory, First};
use crate::form::{Document, Form, Role, SettlementKind};
use crate::lexical_memory::LexicalMemory;
use crate::{similar_vote, text};

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

/// The replays of a field's two memory layers, in the order a suggestion consults them.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerThresholds {
    /// For the key threshold, replayed on every lookup.
    pub key: ThresholdReplay,
    /// For the memory threshold, replayed on the lookups the chosen key threshold would not have answered.
    pub memory: ThresholdReplay,
}

/// What the similar document layer made of one lookup: the vote's score, whether its value matched, the similarity of
/// the nearest document voting for it, and whether more than one document voted.
#[derive(Clone, Copy, Debug)]
struct Vote {
    score: f64,
    correct: bool,
    nearest: f64,
    voted: bool,
}

/// One document of a layers replay: whether the key layer was asked and what its best value was, whether the similar
/// document layer was asked and how it voted, the document's weight and its place in settlement order.
#[derive(Clone, Debug)]
struct Step {
    key_looked: bool,
    key: Option<First>,
    looked: bool,
    vote: Option<Vote>,
    weight: f64,
    order: usize,
}

/// The share of a threshold's answers whose nearest voter may fall below its similarity floor.
const FLOOR_SHARE: f64 = 0.01;

/// Chooses `field`'s key threshold and memory threshold together by replaying `documents` through both memories, which
/// must be empty and configured as the ones in use. Each document, in settlement order, is asked about before it is put
/// in: the key layer as [`select_key_threshold`] asks it, and the similar document layer with the evidence of the values
/// that had arrived before the field, voting among the field's `similar_document_votes` nearest documents. The document
/// memory keeps one document per case — evidence that reads the same after normalization — the latest settled. The key
/// threshold is chosen on every lookup; the memory threshold on the lookups the chosen key threshold would not have
/// answered, and with more than one voter each memory choice carries its similarity floor.
///
/// Values outside a field's candidate domain are not yet left out by this implementation: every document found votes.
///
/// # Panics
/// When `target_precision` is outside (0, 1], `minimum_answered` is 0, the field is not judged, or it takes several
/// values — the similar document layer does not suggest such a field.
#[allow(clippy::too_many_arguments)]
pub fn select_layers(
    field_memory: &mut FieldMemory,
    memory: &mut LexicalMemory,
    form: &Form,
    field: &str,
    documents: &[Document],
    target_precision: f64,
    minimum_answered: usize,
    counting: Option<&[SettlementKind]>,
) -> LayerThresholds {
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
    assert!(
        !definition.multiple,
        "'{field}' takes several values; the similar document layer does not suggest it"
    );

    let (steps, latest) = replay(field_memory, memory, form, field, documents, counting);

    let keyed: Vec<&Step> = steps.iter().filter(|s| s.key_looked).collect();
    let answers: Vec<Answer> = keyed
        .iter()
        .filter_map(|s| {
            s.key.as_ref().map(|first| Answer {
                score: first.score,
                correct: first.matches,
                weight: s.weight,
                order: s.order,
                key: Some(first.key.clone()),
                caught: first.caught,
            })
        })
        .collect();
    let uncaught: Vec<Answer> = answers.iter().filter(|a| !a.caught).cloned().collect();
    let key = with_keys(
        fit_recent(
            &uncaught,
            keyed.len(),
            keyed.iter().fold(0.0, |sum, s| sum + s.weight),
            latest,
            target_precision,
            minimum_answered,
        ),
        &answers,
    );

    let chosen = key.chosen.as_ref().map(|c| c.threshold);
    let left: Vec<&Step> = steps
        .iter()
        .filter(|s| {
            s.looked
                && !chosen.is_some_and(|t| {
                    s.key
                        .as_ref()
                        .is_some_and(|first| !first.caught && first.score >= t)
                })
        })
        .collect();
    let memory = similarity(
        &left,
        latest,
        definition.similar_document_votes,
        target_precision,
        minimum_answered,
    );
    LayerThresholds { key, memory }
}

/// Replays the documents in settlement order, asking each memory about each document before putting it in — the
/// document memory keeping one document per case.
fn replay(
    field_memory: &mut FieldMemory,
    memory: &mut LexicalMemory,
    form: &Form,
    field: &str,
    documents: &[Document],
    counting: Option<&[SettlementKind]>,
) -> (Vec<Step>, usize) {
    let task = format!("{}/{field}", form.name);
    let votes = form.field(field).similar_document_votes;
    let mut ordered: Vec<Document> = documents.iter().map(Document::remembered).collect();
    ordered.sort_by(|a, b| a.settled_at.cmp(&b.settled_at).then(a.id.cmp(&b.id)));

    let mut steps = Vec::new();
    let mut remembered = 0_usize;
    let mut cases: HashMap<String, String> = HashMap::new(); // case → the document representing it
    let mut case_of: HashMap<String, String> = HashMap::new(); // document → the case it represents
    let mut order = 0;
    for document in &ordered {
        order += 1;
        let Some(settled) = document.values.get(field) else {
            continue;
        };

        // A document settled again is not its own evidence.
        field_memory.remove(&form.name, &document.id);
        if let Some(own) = case_of.remove(&document.id) {
            memory.forget(&task, &document.id);
            cases.remove(&own);
            remembered -= 1;
        }

        // Asked with what the field's last suggestion saw; remembered, below, with everything.
        let before = arrived(form, document, field, &document.values);
        let before_sets = arrived(form, document, field, &document.sets);
        let key_looked = field_memory.count(&form.name) > 0;
        let first = if key_looked {
            field_memory.first(form, field, &before, settled, &before_sets)
        } else {
            None
        };
        let evidence = form.evidence(field, &document.values, &document.sets);
        let vote = if remembered > 0 {
            let found = memory.nearest(&task, &form.evidence(field, &before, &before_sets), votes);
            similar_vote::decide(&found, votes).map(|(winner, score)| Vote {
                score,
                correct: winner.answer == *settled,
                nearest: winner.similarity,
                voted: similar_vote::voters(&found, votes) > 1,
            })
        } else {
            None
        };

        // A settlement not counted is replayed into memory like any other, but not asked about.
        let weight = weight(document, field, counting);
        let counted = weight > 0.0;
        steps.push(Step {
            key_looked: key_looked && counted,
            key: first,
            looked: remembered > 0 && counted,
            vote,
            weight,
            order,
        });
        field_memory.put(form, document);

        let case = text::normalize(&evidence);
        if let Some(earlier) = cases.get(&case) {
            memory.forget(&task, earlier);
            case_of.remove(earlier);
            remembered -= 1;
        }

        cases.insert(case.clone(), document.id.clone());
        case_of.insert(document.id.clone(), case);
        memory.remember(&task, &document.id, &evidence, settled);
        remembered += 1;
    }

    (steps, order)
}

/// The similarity choice over the given lookups: every one counts, answered or not. With more than one voter each
/// threshold carries the similarity floor of the answers it rests on.
fn similarity(
    looked: &[&Step],
    latest: usize,
    votes: usize,
    target_precision: f64,
    minimum_answered: usize,
) -> ThresholdReplay {
    let voting: Vec<(&Step, Vote)> = looked
        .iter()
        .filter_map(|s| s.vote.map(|v| (*s, v)))
        .collect();
    let answers: Vec<Answer> = voting
        .iter()
        .map(|(s, v)| Answer {
            score: v.score,
            correct: v.correct,
            weight: s.weight,
            order: s.order,
            key: None,
            caught: false,
        })
        .collect();
    let replay = fit_recent(
        &answers,
        looked.len(),
        looked.iter().fold(0.0, |sum, s| sum + s.weight),
        latest,
        target_precision,
        minimum_answered,
    );
    if votes == 1 {
        return replay;
    }

    // Only answers decided by a vote say how alike its voters were; a lone voter's score is its similarity already.
    let voted: Vec<(f64, f64)> = voting
        .iter()
        .filter(|(_, v)| v.voted)
        .map(|(_, v)| (v.score, v.nearest))
        .collect();
    let floored = |choice: Option<ThresholdChoice>| {
        choice.map(|c| ThresholdChoice {
            similarity_floor: similarity_floor(&voted, c.threshold),
            ..c
        })
    };
    ThresholdReplay {
        chosen: floored(replay.chosen),
        most_precise: floored(replay.most_precise),
        ..replay
    }
}

/// Of the answers decided by a vote at or above `threshold`, the nearest voter's similarity that only [`FLOOR_SHARE`]
/// of them fall below; `None` when there are none.
fn similarity_floor(answers: &[(f64, f64)], threshold: f64) -> Option<f64> {
    let mut nearest: Vec<f64> = answers
        .iter()
        .filter(|(score, _)| *score >= threshold)
        .map(|(_, nearest)| *nearest)
        .collect();
    if nearest.is_empty() {
        return None;
    }

    nearest.sort_by(f64::total_cmp);
    Some(nearest[(FLOOR_SHARE * (nearest.len() - 1) as f64) as usize])
}

/// One set of fields tried for a judged field's key fields, and how its key layer replayed.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyFieldsTrial {
    /// The fields that keyed the judged field in this replay; empty for every field it rests on.
    pub key_fields: Vec<String>,
    /// The key layer's replay with them, as [`select_key_threshold`] makes it.
    pub replay: ThresholdReplay,
}

/// The fields that should key a judged field in the settled field memory, chosen by replaying its key layer, and the
/// replays the choice was made from.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyFieldsChoice {
    /// The fields to name, in the order they were added; `None` when no narrower set gained more than every field the
    /// field rests on — leave the key fields unset.
    pub key_fields: Option<Vec<String>>,
    /// The key layer's replay with the choice.
    pub chosen: ThresholdReplay,
    /// The key layer's replay keyed by every field the field rests on.
    pub all_fields: ThresholdReplay,
    /// Every set tried: each field it rests on alone, in the form's order, then each set the greedy additions tried.
    pub trials: Vec<KeyFieldsTrial>,
}

/// The form with `field` keyed by `key_fields` (every field it rests on when `None`).
fn keyed_by(form: &Form, field: &str, key_fields: Option<Vec<String>>) -> Form {
    let mut form = form.clone();
    for f in form.fields.iter_mut().filter(|f| f.name == field) {
        f.key_fields = key_fields.clone();
    }
    form
}

/// Chooses the fields that should key `field` by the gain their chosen key threshold gives over the target precision —
/// the precision above the target times the answers' weight. Replays the key layer as [`select_key_threshold`] does with
/// each field the field rests on alone, ranks them by gain (a tie keeps the form's order), then adds them in that order
/// while each addition gains more. The result names that set only if it gains more than every such field together.
/// A fresh default field memory serves each replay.
///
/// Counting typed text as well is not yet supported by this implementation.
///
/// # Panics
/// As [`select_key_threshold`].
pub fn select_key_fields(
    form: &Form,
    field: &str,
    documents: &[Document],
    target_precision: f64,
    minimum_answered: usize,
    counting: Option<&[SettlementKind]>,
) -> KeyFieldsChoice {
    let replay = |key_fields: Option<Vec<String>>| KeyFieldsTrial {
        replay: select_key_threshold(
            &mut FieldMemory::default(),
            &keyed_by(form, field, key_fields.clone()),
            field,
            documents,
            target_precision,
            minimum_answered,
            counting,
        ),
        key_fields: key_fields.unwrap_or_default(),
    };
    let gains = |trial: &KeyFieldsTrial| {
        trial.replay.chosen.as_ref().map_or(0.0, |c| {
            (c.precision - target_precision) * c.answered_weight
        })
    };

    let mut trials: Vec<KeyFieldsTrial> = Vec::new();
    let all = replay(None);
    let evidence: Vec<String> = form
        .fields
        .iter()
        .filter(|f| form.supports(&f.name, field))
        .map(|f| f.name.clone())
        .collect();
    let mut ranked: Vec<KeyFieldsTrial> = evidence
        .iter()
        .map(|name| {
            let trial = replay(Some(vec![name.clone()]));
            trials.push(trial.clone());
            trial
        })
        .collect();
    ranked.sort_by(|a, b| gains(b).partial_cmp(&gains(a)).unwrap_or(Ordering::Equal));

    let every_field = |trials: Vec<KeyFieldsTrial>| KeyFieldsChoice {
        key_fields: None,
        chosen: all.replay.clone(),
        all_fields: all.replay.clone(),
        trials,
    };
    let Some(mut best) = ranked.first().cloned() else {
        return every_field(trials);
    };
    if gains(&best) == 0.0 {
        return every_field(trials);
    }

    for next in &ranked[1..] {
        if best.key_fields.len() + 1 == evidence.len() {
            break; // every field it rests on: the replay without key fields
        }

        let mut fields = best.key_fields.clone();
        fields.push(next.key_fields[0].clone());
        let trial = replay(Some(fields));
        trials.push(trial.clone());
        if gains(&trial) <= gains(&best) {
            break;
        }

        best = trial;
    }

    if gains(&best) > gains(&all) {
        KeyFieldsChoice {
            key_fields: Some(best.key_fields),
            chosen: best.replay,
            all_fields: all.replay,
            trials,
        }
    } else {
        every_field(trials)
    }
}

/// Chooses, for each field that keys `field`, how readily its keys are believed after only a few settlements. Replays
/// the documents in the order the field memory weighs them; before each is remembered, every key its values give the
/// field that was seen before predicts the field's value as a Chinese restaurant process does — a value of weight w
/// under a key of weighted total W by w ÷ (W + α), a value not yet seen under the key by α ÷ (W + α) — and each key
/// field takes the α under which its keys' predictions are likeliest, with two anchoring predictions from a key seen
/// once. The keys are those the form gives the field, from the values known when its suggestion would have been made;
/// the field's own concentrations are ignored. `memory` must be empty and configured as the one in use.
///
/// Returns a concentration for each key field whose keys were seen again; empty for a field that takes several values.
///
/// # Panics
/// When the field is not judged.
pub fn select_key_concentrations(
    memory: &mut FieldMemory,
    form: &Form,
    field: &str,
    documents: &[Document],
) -> BTreeMap<String, f64> {
    assert!(
        form.field(field).role == Role::Judged,
        "'{field}' is not a judged field"
    );
    let mut concentrations = BTreeMap::new();
    if form.field(field).multiple {
        return concentrations;
    }

    let mut form = form.clone();
    for f in form.fields.iter_mut().filter(|f| f.name == field) {
        f.key_concentrations.clear();
    }
    let mut ordered: Vec<Document> = documents.iter().map(Document::remembered).collect();
    ordered.sort_by(|a, b| a.settled_at.cmp(&b.settled_at).then(a.id.cmp(&b.id)));

    // key field → (predictions of a new value, the totals predicted from), in document order.
    let mut predictions: BTreeMap<String, (u64, Vec<f64>)> = BTreeMap::new();
    for document in &ordered {
        let Some(settled) = document.values.get(field) else {
            continue;
        };

        memory.remove(&form.name, &document.id); // a document settled again is not its own evidence
        let before = arrived(&form, document, field, &document.values);
        for (key_field, total, weight) in memory.key_totals(&form, field, &before, settled) {
            let (unseen, totals) = predictions.entry(key_field).or_default();
            totals.push(total);
            if weight == 0.0 {
                *unseen += 1;
            }
        }

        memory.put(&form, document);
    }

    for (key_field, (unseen, totals)) in predictions {
        concentrations.insert(key_field, likeliest(unseen, &totals));
    }

    concentrations
}

/// The concentration α under which `unseen` new values among predictions from keys of weighted totals `totals`, with the
/// two anchoring predictions from a key seen once, are likeliest: the root of (unseen + 1) ÷ α = Σ 1 ÷ (W + α) +
/// 2 ÷ (1 + α), found by halving on a log scale, 200 times.
fn likeliest(unseen: u64, totals: &[f64]) -> f64 {
    let (mut low, mut high) = (1e-9_f64, 1e9_f64);
    for _ in 0..200 {
        let mid = (low * high).sqrt();
        let mut sum = 2.0 / (1.0 + mid);
        for total in totals {
            sum += 1.0 / (total + mid);
        }

        if (unseen + 1) as f64 > mid * sum {
            low = mid;
        } else {
            high = mid;
        }
    }

    (low * high).sqrt()
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
