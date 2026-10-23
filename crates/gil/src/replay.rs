//! Chooses a decided attribute's key threshold by replaying settled records: each record, in settlement order, is asked
//! about with the values that had arrived before the attribute, against a key memory holding the records settled before
//! it, then remembered. Precision is fitted as a non-decreasing function of the score, and the threshold is the lowest
//! score down to which every fitted band of answers meets the target precision.
//!
//! Every sum here is taken in a fixed order — the same operations on the same operands — so the numbers come out the
//! same to the bit on every run and every platform.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use crate::decision_record::{CoarseLook, DecisionRecord, KeyLook, SimilarLook, TypedLook};
use crate::key_memory::{First, KeyMemory, Lookup};
use crate::lexical_memory::LexicalMemory;
use crate::schema::{AttributeRole, Record, Schema, SettlementKind};
use crate::{coarse_groups, similar_vote, text};

/// How many of the latest records replayed [`ThresholdChoice::promise`] is taken over — every record in settlement
/// order counts, whether or not it was asked about.
pub const RECENT_RECORDS: usize = 5_000;

/// How many standard errors below the measured precision [`ThresholdRule::Pooled`] holds the target at. More than the one
/// [`ThresholdChoice::promise`] is lowered by: at one, thin attributes answered on a few hundred lookups fell below the
/// target in replays the rule had not been chosen on; at this many none did.
pub const POOLED_STANDARD_ERRORS: f64 = 1.645;

/// How many of the attribute's latest lookups [`ThresholdRule::Pooled`] chooses on, at the least.
pub const POOLED_RECENT_LOOKUPS: usize = 5_000;

/// How many answers, under the attribute's thresholds as they stand, [`ThresholdRule::Pooled`] reaches back for when its
/// latest lookups hold fewer — an attribute answered rarely has too few in them to rest a pooled precision on.
pub const POOLED_RECENT_ANSWERS: usize = 1_000;

/// How many key scores [`ThresholdRule::Pooled`] tries as thresholds, at most, besides none.
const POOLED_KEY_CANDIDATES: usize = 200;

/// How [`select_layers_by_rule`] chooses an attribute's two thresholds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ThresholdRule {
    /// Each layer on its own: the lowest threshold down to which every band of its answers meets the target precision.
    /// No band of weak answers is admitted on the strength of good ones above it, so the attribute answers less than it
    /// could while keeping its promise. The rule of [`select_layers`].
    #[default]
    EveryBand,
    /// The two thresholds together, as the pair that answers the most of the attribute's latest lookups while the
    /// precision of all those answers taken together stays at or above the target at the low end of its Wilson score
    /// interval at [`POOLED_STANDARD_ERRORS`] standard errors — a promise about the answers on average, not about every
    /// band of them. The lookups are the attribute's latest [`POOLED_RECENT_LOOKUPS`], reaching further back while the
    /// attribute's thresholds as they stand would have answered fewer than [`POOLED_RECENT_ANSWERS`] of them. In eight
    /// forward replays on seven public collections, each on records the rule was not tuned on, it kept a 0.8 target
    /// every time and answered 1.10–1.80 times as often as [`ThresholdRule::EveryBand`], 1.55 in the middle. Some bands
    /// of its answers fall below the target where others are above it: choose it when the target is a share of all
    /// answers, not a floor under each kind of answer.
    Pooled,
}

/// What a threshold is chosen for — the target precision, the fewest answers it may rest on, and which settlements count.
#[derive(Clone, Copy, Debug)]
pub struct Choice<'a> {
    /// The precision every band of answers down to the threshold must meet, in (0, 1].
    ///
    /// It follows from what a wrong answer costs: where a wrong answer costs ρ times a value entered with no
    /// suggestion, answering pays only above a precision of `1 - 1/ρ` (0.9 when a wrong value costs ten entries) —
    /// a lower target lets a threshold keep its promise and still cost more than asking.
    pub target_precision: f64,
    /// The fewest independent answers a threshold may rest on, at least 1.
    pub minimum_answered: usize,
    /// The kinds of settlement whose lookups are counted — `None` counts every settlement, including those not known.
    pub counting: Option<&'a [SettlementKind]>,
}

impl Choice<'_> {
    /// Asserts that a threshold can be chosen for this.
    ///
    /// # Panics
    /// When `target_precision` is outside (0, 1] or `minimum_answered` is 0.
    fn check(&self) {
        assert!(
            self.target_precision > 0.0 && self.target_precision <= 1.0,
            "target_precision must be in (0, 1]"
        );
        assert!(
            self.minimum_answered >= 1,
            "minimum_answered must be at least 1"
        );
    }
}

/// A threshold chosen for one memory layer of an attribute, and how it did on the replay it was chosen on. A checked value
/// ([`SettlementKind::Check`]) counts as one ÷ its draw probability lookups, so `precision` and `answer_rate` are
/// weighted, while `answered` and `lookups` count the lookups made.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// For the similar record layer only; always `None` for the key layer.
    pub similarity_floor: Option<f64>,
    /// The precision to expect of the next answers: the precision of the answers at or above the threshold among the
    /// latest [`RECENT_RECORDS`] records replayed (or `precision` when those are worth fewer than the minimum number
    /// of answers), lowered to the bottom of its Wilson score interval at one standard error.
    pub promise: f64,
    /// How many independent answers `promise` is worth.
    pub promise_answered: f64,
}

/// How the key layer did under one key: the lookups whose best value this key backed most strongly, and how the chosen
/// threshold treated them.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyState {
    /// The key, as a suggestion's evidence names it — `attribute: value`.
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

/// What replaying settled records found for one memory layer of an attribute.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ThresholdReplay {
    /// The lowest threshold down to which every band of answers met the target precision; `None` when none does.
    pub chosen: Option<ThresholdChoice>,
    /// The highest threshold that gathers at least the minimum number of answers; `None` when no score does.
    pub most_precise: Option<ThresholdChoice>,
    /// How many lookups the replay made.
    pub lookups: usize,
    /// How many of them found a candidate at any score.
    pub candidates: usize,
    /// For the key layer of an attribute that takes one value, the state under each key that backed a best value, most
    /// lookups first, then by key in scalar order.
    pub keys: Option<Vec<KeyState>>,
}

/// One answer the layer would give at some threshold: its score, whether it matched, how many decisions it counts as,
/// the place of its record in settlement order (from 1, counting every record), the key that backed it most
/// strongly, and whether a value the attribute must not miss held the key layer back from it.
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

/// Chooses `attribute`'s key threshold by replaying `records` through `memory`, which must be empty and configured as the
/// one in use. Records are replayed in settlement order (then by id in scalar order), each without its earlier version
/// as evidence; a value the application filled is neither asked about nor remembered. `counting` names the kinds of
/// settlement whose lookups are counted — `None` counts every settlement, including those not known.
///
/// An attribute that takes several values trusts each value on its own score, so every value offered under the keys counts,
/// right when it is one of the settled values not yet chosen. A person picks such values one after another, and those
/// chosen are evidence for the rest, so each record is asked about once with none of its values chosen, then again
/// after each but the last is chosen, in [`pick_order`]. `lookups` then counts the values sought, and the replay carries
/// no state under each key.
///
/// # Panics
/// When `target_precision` is outside (0, 1], `minimum_answered` is 0, or the attribute is not decided.
pub fn select_key_threshold(
    memory: &mut KeyMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    choice: Choice,
) -> ThresholdReplay {
    choice.check();
    let definition = schema.attribute(attribute);
    assert!(
        definition.role == AttributeRole::Decided,
        "'{attribute}' is not a decided attribute"
    );
    let multiple = definition.multiple;

    let mut ordered: Vec<Record> = records.iter().map(Record::remembered).collect();
    ordered.sort_by(|a, b| {
        a.settled_at
            .cmp(&b.settled_at)
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut matches: Vec<Answer> = Vec::new();
    let (mut lookups, mut lookup_weight, mut order) = (0_usize, 0.0_f64, 0_usize);
    for record in &ordered {
        order += 1;
        let settled_set = if multiple {
            pick_order(record, attribute)
        } else {
            Vec::new()
        };
        let settled = record.values.get(attribute);
        let nothing_settled = if multiple {
            settled_set.is_empty()
        } else {
            settled.is_none()
        };
        if nothing_settled {
            continue;
        }

        memory.remove(schema.name(), &record.id); // a record settled again is not its own evidence
        let weight = weight(record, attribute, choice.counting);
        if memory.count(schema.name()) > 0 && weight > 0.0 {
            let before = arrived(schema, record, attribute, &record.values);
            let before_sets = arrived(schema, record, attribute, &record.sets);
            let asked = Lookup::new(schema, attribute, &before).sets(&before_sets);
            if let Some(settled) = settled.filter(|_| !multiple) {
                lookups += 1;
                lookup_weight += weight;
                if let Some(first) = memory.first(&asked, settled) {
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
                    sets.insert(attribute.to_owned(), settled_set[..chosen].to_vec());
                    let mut sought: Vec<&String> = settled_set[chosen..].iter().collect();
                    sought.sort();
                    sought.dedup();
                    lookups += sought.len();
                    lookup_weight += sought.len() as f64 * weight;
                    for (value, score) in memory.scored(&asked.sets(&sets)) {
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

        memory.put(schema, record);
    }

    let uncaught: Vec<Answer> = matches.iter().filter(|m| !m.caught).cloned().collect();
    let replay = fit_recent(
        &uncaught,
        lookups,
        lookup_weight,
        order,
        choice.target_precision,
        choice.minimum_answered,
    );
    if multiple {
        replay
    } else {
        with_keys(replay, &matches)
    }
}

/// A set attribute's settled values, once each, in the order a replay supposes they were picked: by the 64-bit FNV-1a hash
/// of the UTF-8 of the record id, U+001F and the value, then in scalar order — so it depends neither on the order the
/// values are listed in nor on their scalar order.
pub fn pick_order(record: &Record, attribute: &str) -> Vec<String> {
    let mut values: Vec<&String> = Vec::new();
    for value in record.sets.get(attribute).into_iter().flatten() {
        if !values.contains(&value) {
            values.push(value);
        }
    }
    let mut keyed: Vec<(u64, &String)> = values
        .into_iter()
        .map(|v| (fnv1a(&format!("{}\u{1f}{v}", record.id)), v))
        .collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    keyed.into_iter().map(|(_, v)| v.clone()).collect()
}

/// A stable 64-bit FNV-1a hash of the text's UTF-8 bytes.
pub(crate) fn fnv1a(text: &str) -> u64 {
    text.bytes()
        .fold(14_695_981_039_346_656_037_u64, |hash, b| {
            (hash ^ u64::from(b)).wrapping_mul(1_099_511_628_211)
        })
}

/// How many decisions the attribute's settlement in the record counts as: none when its kind is not counted (every kind is
/// when none are named), one ÷ the draw probability for a check, one otherwise.
/// The replays a single-valued attribute's typed key thresholds are chosen on, one per length typed, shortest first.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypedKeyThresholds {
    pub by_length: Vec<ThresholdReplay>,
}

impl TypedKeyThresholds {
    /// The thresholds to set as the attribute's typed key thresholds: each replay's chosen one, `None` where none was.
    pub fn thresholds(&self) -> Vec<Option<f64>> {
        self.by_length
            .iter()
            .map(|r| r.chosen.as_ref().map(|c| c.threshold))
            .collect()
    }
}

/// One record of a typed replay: its settled value, the key layer's best value with none of it typed and with each
/// length typed, and what the thresholds chosen so far claimed for it.
struct Asked {
    settled: String,
    best: Vec<Option<(f64, String)>>,
    weight: f64,
    order: usize,
    right: bool,
    wrong: Vec<String>,
}

impl Asked {
    /// Records the claim a threshold makes on the best value, if its score reaches it.
    fn claim(&mut self, length: usize, threshold: Option<f64>) {
        let Some((score, value)) = &self.best[length] else {
            return;
        };
        if !threshold.is_some_and(|t| *score >= t) {
            return;
        }

        if *value == self.settled {
            self.right = true;
        } else {
            self.wrong.push(value.clone());
        }
    }

    /// Whether a wrong claim is still led to with `length` characters of the settled value typed.
    fn holds(&self, length: usize) -> bool {
        let typed = text::prefix(&self.settled, length);
        self.wrong
            .iter()
            .any(|v| crate::key_memory::begins(v, Some(typed)))
    }
}

/// Chooses a single-valued attribute's typed key thresholds — for one, two, … up to `longest` characters of its value
/// typed. Replays the records in settlement order as [`select_key_threshold`] does, asking the key memory for the
/// best value that begins with each length of the settled value. A value trusted with fewer characters typed is one
/// claim, kept while the typed text still leads to it: a record so answered rightly needs no more typing, one
/// answered wrongly by a value the next characters still lead to is not among the records a longer threshold is
/// chosen on until the typing has left that value behind. Each length's threshold is chosen on the records left.
///
/// # Panics
/// When `target_precision` is outside (0, 1], `minimum_answered` or `longest` is 0, or the attribute is not decided or
/// takes several values.
pub fn select_typed_key_thresholds(
    memory: &mut KeyMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    choice: Choice,
    longest: usize,
) -> TypedKeyThresholds {
    select_typed_key_thresholds_with(
        memory,
        schema,
        attribute,
        records,
        choice,
        longest,
        &mut |_| {},
    )
}

/// As [`select_typed_key_thresholds`], handing `sink` the decision record of every record the replay asks about, in
/// settlement order — the key layer's best value at each typed length ([`DecisionRecord::typed`]).
///
/// # Panics
/// As [`select_typed_key_thresholds`].
pub fn select_typed_key_thresholds_with(
    memory: &mut KeyMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    choice: Choice,
    longest: usize,
    sink: &mut dyn FnMut(&DecisionRecord),
) -> TypedKeyThresholds {
    choice.check();
    assert!(longest >= 1, "longest must be at least 1");
    let definition = schema.attribute(attribute);
    assert!(
        definition.role == AttributeRole::Decided,
        "'{attribute}' is not a decided attribute"
    );
    assert!(
        !definition.multiple,
        "'{attribute}' takes several values; typed key thresholds are for an attribute that takes one"
    );

    let mut ordered: Vec<Record> = records.iter().map(Record::remembered).collect();
    ordered.sort_by(|a, b| {
        a.settled_at
            .cmp(&b.settled_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut asked: Vec<Asked> = Vec::new();
    let mut order = 0;
    for record in &ordered {
        order += 1;
        let Some(settled) = record.values.get(attribute) else {
            continue;
        };

        memory.remove(schema.name(), &record.id); // a record settled again is not its own evidence
        let weight = weight(record, attribute, choice.counting);
        if memory.count(schema.name()) > 0 && weight > 0.0 {
            let before = arrived(schema, record, attribute, &record.values);
            let before_sets = arrived(schema, record, attribute, &record.sets);
            let best = memory.typed(
                &Lookup::new(schema, attribute, &before).sets(&before_sets),
                settled,
                longest,
            );
            sink(&DecisionRecord {
                record: record.id.clone(),
                attribute: attribute.to_owned(),
                order: Some(order as u64),
                weight,
                key: None,
                similar: None,
                coarse: None,
                typed: Some(
                    best.iter()
                        .map(|b| {
                            b.as_ref().map(|(strength, value)| TypedLook {
                                strength: *strength,
                                value: value.clone(),
                                correct: value == settled,
                            })
                        })
                        .collect(),
                ),
                answered: None,
                settled: Some(settled.clone()),
                settlement: record.settlements.get(attribute).copied(),
                probability: record.probabilities.get(attribute).copied(),
            });
            asked.push(Asked {
                settled: settled.clone(),
                best,
                weight,
                order,
                right: false,
                wrong: Vec::new(),
            });
        }

        memory.put(schema, record);
    }

    for a in &mut asked {
        a.claim(0, definition.key_threshold);
    }

    let mut by_length = Vec::new();
    for length in 1..=longest {
        let reaching: Vec<usize> = (0..asked.len())
            .filter(|&i| {
                let a = &asked[i];
                !a.right && a.best.len() > length && !a.holds(length)
            })
            .collect();
        let matches: Vec<Answer> = reaching
            .iter()
            .filter_map(|&i| {
                let a = &asked[i];
                a.best[length].as_ref().map(|(score, value)| Answer {
                    score: *score,
                    correct: *value == a.settled,
                    weight: a.weight,
                    order: a.order,
                    key: None,
                    caught: false,
                })
            })
            .collect();
        let replay = fit_recent(
            &matches,
            reaching.len(),
            reaching.iter().fold(0.0, |sum, &i| sum + asked[i].weight),
            order,
            choice.target_precision,
            choice.minimum_answered,
        );
        let chosen = replay.chosen.as_ref().map(|c| c.threshold);
        for &i in &reaching {
            asked[i].claim(length, chosen);
        }

        by_length.push(replay);
    }

    TypedKeyThresholds { by_length }
}

fn weight(record: &Record, attribute: &str, counting: Option<&[SettlementKind]>) -> f64 {
    let kind = record.settlements.get(attribute).copied();
    if let Some(counting) = counting
        && !kind.is_some_and(|k| counting.contains(&k))
    {
        return 0.0;
    }

    match (kind, record.probabilities.get(attribute)) {
        (Some(SettlementKind::Check), Some(probability)) => 1.0 / probability,
        _ => 1.0,
    }
}

/// The record's values that had arrived before `attribute` was settled — what its last suggestion was made from. With no
/// arrival order, the observed values count as there from the start and the decided ones as settled in the schema's order;
/// with one, an attribute it leaves out counts as there from the start.
fn arrived<T: Clone>(
    schema: &Schema,
    record: &Record,
    attribute: &str,
    values: &BTreeMap<String, T>,
) -> BTreeMap<String, T> {
    let rank = |name: &str| -> i64 {
        if let Some(arrival) = &record.arrival {
            return match arrival.iter().position(|n| n == name) {
                Some(at) => at as i64,
                None if name == attribute => i64::MAX,
                None => -1,
            };
        }

        // A value of no attribute of the schema is no evidence anyway.
        match schema.attributes().iter().position(|f| f.name == name) {
            Some(index) if schema.attributes()[index].role != AttributeRole::Observed => {
                index as i64
            }
            _ => -1,
        }
    };

    let own = rank(attribute);
    values
        .iter()
        .filter(|(name, _)| name.as_str() != attribute && rank(name) < own)
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/// [`fit`], then each choice's promise over the answers among the latest [`RECENT_RECORDS`] records of `latest`.
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
    ThresholdReplay {
        chosen: promised(replay.chosen, matches, latest, minimum_answered),
        most_precise: promised(replay.most_precise, matches, latest, minimum_answered),
        ..replay
    }
}

/// The choice with its promise, taken over the answers of the latest [`RECENT_RECORDS`] records of `latest`.
fn promised(
    choice: Option<ThresholdChoice>,
    matches: &[Answer],
    latest: usize,
    minimum_answered: usize,
) -> Option<ThresholdChoice> {
    let choice = choice?;
    let (mut weight, mut correct, mut squares) = (0.0_f64, 0.0_f64, 0.0_f64);
    for m in matches.iter().filter(|m| {
        m.order as i64 > latest as i64 - RECENT_RECORDS as i64 && m.score >= choice.threshold
    }) {
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
}

/// The choice of a given threshold: how the answers at or above it did over the whole replay, and its promise; `None`
/// when none is.
fn at(
    matches: &[Answer],
    lookups: usize,
    lookup_weight: f64,
    latest: usize,
    threshold: f64,
    minimum_answered: usize,
) -> Option<ThresholdChoice> {
    let (mut count, mut weight, mut correct, mut squares) = (0_usize, 0.0_f64, 0.0_f64, 0.0_f64);
    for m in matches.iter().filter(|m| m.score >= threshold) {
        count += 1;
        weight += m.weight;
        correct += if m.correct { m.weight } else { 0.0 };
        squares += m.weight * m.weight;
    }

    if weight <= 0.0 {
        return None;
    }

    promised(
        Some(ThresholdChoice {
            threshold,
            precision: correct / weight,
            answer_rate: weight / lookup_weight,
            answered: count,
            lookups,
            answered_weight: weight,
            effective_answered: weight * weight / squares,
            similarity_floor: None,
            promise: correct / weight,
            promise_answered: count as f64,
        }),
        matches,
        latest,
        minimum_answered,
    )
}

/// The lower end of the Wilson score interval at one standard error for a precision measured on `answered` independent
/// answers.
fn lower(precision: f64, answered: f64) -> f64 {
    lower_at(precision, answered, 1.0)
}

/// The lower end of the Wilson score interval at `standard_errors` for a precision measured on `answered` independent
/// answers.
fn lower_at(precision: f64, answered: f64, standard_errors: f64) -> f64 {
    if answered <= 0.0 {
        return precision;
    }

    let z2 = standard_errors * standard_errors;
    let centre = precision + (z2 / (2.0 * answered));
    let margin = standard_errors
        * ((precision * (1.0 - precision) / answered) + (z2 / (4.0 * answered * answered))).sqrt();
    (centre - margin) / (1.0 + (z2 / answered))
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
    keys.sort_by(|a, b| b.lookups.cmp(&a.lookups).then_with(|| a.key.cmp(&b.key)));
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
    // Answers of one score schema a group, in replay order; groups go from the lowest score up.
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

/// The replays of an attribute's two memory layers, in the order a suggestion consults them.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LayerThresholds {
    /// For the key threshold, replayed on every lookup.
    pub key: ThresholdReplay,
    /// For the memory threshold, replayed on the lookups the chosen key threshold would not have answered.
    pub memory: ThresholdReplay,
}

/// What the similar record layer made of one lookup: the vote's score, whether its value matched, the similarity of
/// the nearest record voting for it, and whether more than one record voted.
#[derive(Clone, Copy, Debug)]
struct Vote {
    score: f64,
    correct: bool,
    nearest: f64,
    voted: bool,
}

/// One record of a layers replay: whether the key layer was asked and what its best value was, whether the similar
/// record layer was asked and how it voted, the record's weight and its place in settlement order.
#[derive(Clone, Debug)]
struct Step {
    key_looked: bool,
    key: Option<First>,
    looked: bool,
    vote: Option<Vote>,
    weight: f64,
    order: usize,
    /// For a coarse replay: the level as the keys and the vote back it, and whether the attribute's value layers answered.
    coarse: Option<CoarseStep>,
}

/// What an attribute's coarse level made of one lookup: whether the attribute's own layers answered the value at its thresholds,
/// the score of the class the keys back and whether it was the settled value's, and how the similar records voted
/// for a class.
#[derive(Clone, Copy, Debug)]
struct CoarseStep {
    value_answered: bool,
    keys: Option<(f64, bool)>,
    vote: Option<Vote>,
}

/// The share of a threshold's answers whose nearest voter may fall below its similarity floor.
const FLOOR_SHARE: f64 = 0.01;

/// Chooses `attribute`'s key threshold and memory threshold together by replaying `records` through both memories, which
/// must be empty and configured as the ones in use. Each record, in settlement order, is asked about before it is put
/// in: the key layer as [`select_key_threshold`] asks it, and the similar record layer with the evidence of the values
/// that had arrived before the attribute, voting among the attribute's `similar_votes` nearest records. The record
/// memory keeps one record per case — evidence that reads the same after normalization — the latest settled. The key
/// threshold is chosen on every lookup; the memory threshold on the lookups the chosen key threshold would not have
/// answered, and with more than one voter each memory choice carries its similarity floor.
///
/// # Panics
/// When `target_precision` is outside (0, 1], `minimum_answered` is 0, the attribute is not decided, or it takes several
/// values — the similar record layer does not answer such an attribute.
pub fn select_layers(
    key_memory: &mut KeyMemory,
    memory: &mut LexicalMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    choice: Choice,
) -> LayerThresholds {
    select_layers_by_rule_with(
        key_memory,
        memory,
        schema,
        attribute,
        records,
        choice,
        ThresholdRule::EveryBand,
        &mut |_| {},
    )
}

/// As [`select_layers`], by the rule given. With [`ThresholdRule::Pooled`] each layer's `chosen` is that layer's half of
/// the pair, `None` when the pair leaves the layer silent; its precision, answer rate and promise are the layer's own,
/// over the whole replay, and the precision of the two layers' answers together held the target on the lookups the pair
/// was chosen on. `most_precise` is as under [`ThresholdRule::EveryBand`]. The attribute's thresholds as they stand in
/// `schema` (`key_threshold`, `memory_threshold`, `memory_floor`) decide only how far back the rule reaches.
///
/// # Panics
/// As [`select_layers`].
pub fn select_layers_by_rule(
    key_memory: &mut KeyMemory,
    memory: &mut LexicalMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    choice: Choice,
    rule: ThresholdRule,
) -> LayerThresholds {
    select_layers_by_rule_with(
        key_memory,
        memory,
        schema,
        attribute,
        records,
        choice,
        rule,
        &mut |_| {},
    )
}

/// As [`select_layers`], handing `sink` the decision record of every record the replay asks about, in settlement
/// order: what the key layer and the similar record layer made of it whatever the thresholds, and what it was settled
/// as. The thresholds are chosen from exactly these records.
///
/// # Panics
/// As [`select_layers`].
pub fn select_layers_with(
    key_memory: &mut KeyMemory,
    memory: &mut LexicalMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    choice: Choice,
    sink: &mut dyn FnMut(&DecisionRecord),
) -> LayerThresholds {
    select_layers_by_rule_with(
        key_memory,
        memory,
        schema,
        attribute,
        records,
        choice,
        ThresholdRule::EveryBand,
        sink,
    )
}

/// As [`select_layers_by_rule`], handing `sink` the decision record of every record the replay asks about, as
/// [`select_layers_with`] does.
///
/// # Panics
/// As [`select_layers`].
#[allow(clippy::too_many_arguments)]
pub fn select_layers_by_rule_with(
    key_memory: &mut KeyMemory,
    memory: &mut LexicalMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    choice: Choice,
    rule: ThresholdRule,
    sink: &mut dyn FnMut(&DecisionRecord),
) -> LayerThresholds {
    choice.check();
    let definition = schema.attribute(attribute);
    assert!(
        definition.role == AttributeRole::Decided,
        "'{attribute}' is not a decided attribute"
    );
    assert!(
        !definition.multiple,
        "'{attribute}' takes several values; the similar record layer does not answer it"
    );

    let (steps, latest) = replay(
        key_memory,
        memory,
        schema,
        attribute,
        records,
        choice.counting,
        Replaying::Values(sink),
    );

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
    let mut key_replay = fit_recent(
        &uncaught,
        keyed.len(),
        keyed.iter().fold(0.0, |sum, s| sum + s.weight),
        latest,
        choice.target_precision,
        choice.minimum_answered,
    );
    let mut pooled_memory = None;
    if rule == ThresholdRule::Pooled {
        let (key_threshold, similar_threshold) = pooled(
            &steps,
            definition,
            choice.target_precision,
            choice.minimum_answered,
        );
        key_replay = ThresholdReplay {
            chosen: key_threshold.and_then(|k| {
                at(
                    &uncaught,
                    keyed.len(),
                    keyed.iter().fold(0.0, |sum, s| sum + s.weight),
                    latest,
                    k,
                    choice.minimum_answered,
                )
            }),
            ..key_replay
        };
        pooled_memory = Some(similar_threshold);
    }

    let key = with_keys(key_replay, &answers);

    let chosen = key.chosen.as_ref().map(|c| c.threshold);
    // Left to the similar record layer: what the chosen key threshold does not answer — but not a lookup under a key
    // that settled a value to catch, which no layer answers.
    let left: Vec<&Step> = steps
        .iter()
        .filter(|s| {
            s.looked
                && !s.key.as_ref().is_some_and(|first| first.caught)
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
        definition.similar_votes,
        choice.target_precision,
        choice.minimum_answered,
        pooled_memory,
    );
    LayerThresholds { key, memory }
}

/// [`ThresholdRule::Pooled`]: of the attribute's latest lookups, reaching back for answers as the rule says, the key and
/// similar record thresholds that answer the most while the precision of their answers together holds the target at
/// [`POOLED_STANDARD_ERRORS`] standard errors, on at least the minimum number of answers. Key thresholds are tried from
/// none down through the key scores seen (at most [`POOLED_KEY_CANDIDATES`] of them, evenly spread), and for each, every
/// similarity score at which ties end; the first pair to answer the most wins. `None` for a layer the pair leaves
/// silent — both when no pair holds the target.
fn pooled(
    steps: &[Step],
    definition: &crate::schema::Attribute,
    target_precision: f64,
    minimum_answered: usize,
) -> (Option<f64>, Option<f64>) {
    let lookups: Vec<&Step> = steps.iter().filter(|s| s.key_looked || s.looked).collect();
    let mut start = lookups.len().saturating_sub(POOLED_RECENT_LOOKUPS);
    if start > 0 {
        // Reach back while the attribute's thresholds as they stand answer fewer than the answers to rest on.
        let mut answered = 0_usize;
        let mut reach = 0_usize;
        for i in (0..lookups.len()).rev() {
            if answered_as_it_stands(lookups[i], definition) {
                answered += 1;
            }

            if answered >= POOLED_RECENT_ANSWERS {
                reach = i;
                break;
            }
        }

        start = start.min(reach);
    }

    let window = &lookups[start..];
    let mut keys: Vec<f64> = window
        .iter()
        .filter(|s| s.key_looked)
        .filter_map(|s| s.key.as_ref().filter(|k| !k.caught).map(|k| k.score))
        .collect();
    keys.sort_by(f64::total_cmp);
    keys.dedup();
    let mut similar: Vec<(&Step, Vote)> = window
        .iter()
        .filter(|s| s.looked && !s.key.as_ref().is_some_and(|k| k.caught))
        .filter_map(|s| s.vote.map(|v| (*s, v)))
        .collect();
    similar.sort_by(|a, b| b.1.score.total_cmp(&a.1.score));

    let mut best: Option<(usize, f64, f64)> = None;
    for key_threshold in key_candidates(&keys) {
        let key_answers = |s: &Step| {
            s.key_looked
                && s.key
                    .as_ref()
                    .is_some_and(|k| !k.caught && k.score >= key_threshold)
        };
        let (mut count, mut weight, mut correct, mut squares) =
            (0_usize, 0.0_f64, 0.0_f64, 0.0_f64);
        for s in window.iter().filter(|s| key_answers(s)) {
            count += 1;
            weight += s.weight;
            correct += if s.key.as_ref().is_some_and(|k| k.matches) {
                s.weight
            } else {
                0.0
            };
            squares += s.weight * s.weight;
        }

        let key_count = count;
        let mut most: i64 = if holds(weight, correct, squares, target_precision, minimum_answered) {
            count as i64
        } else {
            -1
        };
        let mut at_score = f64::INFINITY;
        for (i, (s, v)) in similar.iter().enumerate() {
            if !key_answers(s) {
                count += 1;
                weight += s.weight;
                correct += if v.correct { s.weight } else { 0.0 };
                squares += s.weight * s.weight;
            }

            let tie_ends = i == similar.len() - 1 || similar[i + 1].1.score != v.score;
            if tie_ends
                && count > key_count
                && count as i64 > most
                && holds(weight, correct, squares, target_precision, minimum_answered)
            {
                most = count as i64;
                at_score = v.score;
            }
        }

        if most > 0 && best.is_none_or(|(c, _, _)| most as usize > c) {
            best = Some((most as usize, key_threshold, at_score));
        }
    }

    match best {
        Some((_, k, s)) => (
            (!k.is_infinite()).then_some(k),
            (!s.is_infinite()).then_some(s),
        ),
        None => (None, None),
    }
}

/// None, then the key scores seen from the highest down — or, when there are more than [`POOLED_KEY_CANDIDATES`], that
/// many evenly spread between the lowest and the highest, interpolated linearly between neighbouring scores.
fn key_candidates(ascending: &[f64]) -> Vec<f64> {
    let mut spread: Vec<f64> = if ascending.len() > POOLED_KEY_CANDIDATES {
        let last = ascending.len() - 1;
        (0..POOLED_KEY_CANDIDATES)
            .map(|i| {
                let position = (i as f64 * last as f64) / (POOLED_KEY_CANDIDATES - 1) as f64;
                let below = position.floor() as usize;
                if below >= last {
                    ascending[last]
                } else {
                    ascending[below]
                        + ((position - below as f64) * (ascending[below + 1] - ascending[below]))
                }
            })
            .collect()
    } else {
        ascending.to_vec()
    };
    spread.sort_by(|a, b| b.total_cmp(a));
    spread.dedup();
    let mut candidates = vec![f64::INFINITY];
    candidates.extend(spread);
    candidates
}

/// Whether answers of this weight hold the target at the low end of their Wilson interval at
/// [`POOLED_STANDARD_ERRORS`], resting on at least the minimum number.
fn holds(
    weight: f64,
    correct: f64,
    squares: f64,
    target_precision: f64,
    minimum_answered: usize,
) -> bool {
    if weight <= 0.0 {
        return false;
    }

    let effective = weight * weight / squares;
    effective >= minimum_answered as f64
        && lower_at(correct / weight, effective, POOLED_STANDARD_ERRORS) >= target_precision
}

/// Whether the attribute's thresholds as they stand would have answered the lookup, as a suggestion does.
fn answered_as_it_stands(step: &Step, definition: &crate::schema::Attribute) -> bool {
    step.key_looked
        && step
            .key
            .as_ref()
            .is_some_and(|k| !k.caught && definition.key_threshold.is_some_and(|t| k.score >= t))
        || step.looked
            && !step.key.as_ref().is_some_and(|k| k.caught)
            && step.vote.is_some_and(|v| {
                definition.memory_threshold.is_some_and(|t| v.score >= t)
                    && !(v.voted
                        && definition
                            .memory_floor
                            .is_some_and(|floor| v.nearest < floor))
            })
}

/// Chooses the thresholds of `attribute`'s coarse level in the order a suggestion offers it: on the lookups the attribute's
/// value layers leave unanswered at the thresholds it has (its key threshold, its memory threshold with its floor —
/// choose those first), the class the keys back, as [`select_key_threshold`] chooses for a value; on the lookups that
/// leaves, the class the similar records vote for, as [`select_layers`] chooses the memory threshold, with its
/// similarity floor. A class is right when it is the settled value's ([`CoarseLevel::of`]). Each record is asked
/// about in memories holding only the records settled before it, then put into both, as for the value. Each replay's
/// lookups count only the lookups left to it.
///
/// [`CoarseLevel::of`]: crate::schema::CoarseLevel::of
///
/// # Panics
/// As [`select_layers`], and when the attribute has no coarse level.
pub fn select_coarse(
    key_memory: &mut KeyMemory,
    memory: &mut LexicalMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    choice: Choice,
) -> LayerThresholds {
    select_coarse_with(
        key_memory,
        memory,
        schema,
        attribute,
        records,
        choice,
        &mut |_| {},
    )
}

/// As [`select_coarse`], handing `sink` the decision record of every record the replay asks about, in settlement
/// order — with what the coarse level made of it ([`DecisionRecord::coarse`]).
///
/// # Panics
/// As [`select_coarse`].
pub fn select_coarse_with(
    key_memory: &mut KeyMemory,
    memory: &mut LexicalMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    choice: Choice,
    sink: &mut dyn FnMut(&DecisionRecord),
) -> LayerThresholds {
    choice.check();
    let definition = schema.attribute(attribute);
    assert!(
        definition.role == AttributeRole::Decided,
        "'{attribute}' is not a decided attribute"
    );
    assert!(
        !definition.multiple,
        "'{attribute}' takes several values; the similar record layer does not answer it"
    );
    assert!(
        definition.coarse.is_some(),
        "'{attribute}' has no coarse level to choose thresholds for"
    );

    let (steps, latest) = replay(
        key_memory,
        memory,
        schema,
        attribute,
        records,
        choice.counting,
        Replaying::Coarse(sink),
    );
    // What the level made of each lookup the value layers left, as a step whose vote is the class's — not one under a
    // key that settled a value to catch, where the level is not offered either.
    let left: Vec<(Step, Option<(f64, bool)>)> = steps
        .iter()
        .filter_map(|s| {
            let coarse = s.coarse.expect("a coarse replay's step");
            let caught = s.key.as_ref().is_some_and(|first| first.caught);
            ((s.key_looked || s.looked) && !coarse.value_answered && !caught).then(|| {
                let step = Step {
                    vote: coarse.vote,
                    ..s.clone()
                };
                (step, coarse.keys)
            })
        })
        .collect();
    let answers: Vec<Answer> = left
        .iter()
        .filter_map(|(s, keys)| {
            keys.map(|(score, correct)| Answer {
                score,
                correct,
                weight: s.weight,
                order: s.order,
                key: None,
                caught: false,
            })
        })
        .collect();
    let key = fit_recent(
        &answers,
        left.len(),
        left.iter().fold(0.0, |sum, (s, _)| sum + s.weight),
        latest,
        choice.target_precision,
        choice.minimum_answered,
    );

    let chosen = key.chosen.as_ref().map(|c| c.threshold);
    let rest: Vec<&Step> = left
        .iter()
        .filter(|(_, keys)| !chosen.is_some_and(|t| keys.is_some_and(|(score, _)| score >= t)))
        .map(|(s, _)| s)
        .collect();
    let memory = similarity(
        &rest,
        latest,
        definition.similar_votes,
        choice.target_precision,
        choice.minimum_answered,
        None,
    );
    LayerThresholds { key, memory }
}

/// What a replay is for: the value layers or the coarse level as well, handing each lookup's decision record to a sink.
enum Replaying<'s> {
    Values(&'s mut dyn FnMut(&DecisionRecord)),
    Coarse(&'s mut dyn FnMut(&DecisionRecord)),
}

/// Replays the records in settlement order, asking each memory about each record before putting it in — the
/// record memory keeping one record per case.
fn replay(
    key_memory: &mut KeyMemory,
    memory: &mut LexicalMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    counting: Option<&[SettlementKind]>,
    replaying: Replaying,
) -> (Vec<Step>, usize) {
    let (coarse, sink) = match replaying {
        Replaying::Values(sink) => (false, sink),
        Replaying::Coarse(sink) => (true, sink),
    };
    let task = format!("{}/{attribute}", schema.name());
    let definition = schema.attribute(attribute);
    let votes = definition.similar_votes;
    let mut ordered: Vec<Record> = records.iter().map(Record::remembered).collect();
    ordered.sort_by(|a, b| {
        a.settled_at
            .cmp(&b.settled_at)
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut steps = Vec::new();
    let mut remembered = 0_usize;
    let mut cases: HashMap<String, String> = HashMap::new(); // case → the record representing it
    let mut case_of: HashMap<String, String> = HashMap::new(); // record → the case it represents
    let mut order = 0;
    for record in &ordered {
        order += 1;
        let Some(settled) = record.values.get(attribute) else {
            continue;
        };

        // A record settled again is not its own evidence.
        key_memory.remove(schema.name(), &record.id);
        if let Some(own) = case_of.remove(&record.id) {
            memory.forget(&task, &record.id);
            cases.remove(&own);
            remembered -= 1;
        }

        // Asked with what the attribute's last suggestion saw; remembered, below, with everything.
        let before = arrived(schema, record, attribute, &record.values);
        let before_sets = arrived(schema, record, attribute, &record.sets);
        let key_looked = key_memory.count(schema.name()) > 0;
        let asked = Lookup::new(schema, attribute, &before).sets(&before_sets);
        let first = if key_looked {
            key_memory.first(&asked, settled)
        } else {
            None
        };
        let evidence = schema.evidence(attribute, &record.values, &record.sets);
        // As a suggestion does: only records whose value lies in the attribute's domain vote.
        let found: Vec<_> = if remembered > 0 {
            memory
                .nearest(
                    &task,
                    &schema.evidence(attribute, &before, &before_sets),
                    votes,
                )
                .into_iter()
                .filter(|m| definition.admits_in(&m.answer, &before, &before_sets))
                .collect()
        } else {
            Vec::new()
        };
        let decided = similar_vote::decide(&found, votes);
        let vote = decided.as_ref().map(|(winner, score)| Vote {
            score: *score,
            correct: winner.answer == *settled,
            nearest: winner.similarity,
            voted: similar_vote::voters(&found, votes) > 1,
        });
        let (coarse_step, coarse_look) = match &definition.coarse {
            Some(level) if coarse => {
                // As a suggestion: the value layers answer at the attribute's own thresholds; only where they do not is
                // the level offered.
                let value_answered = first.as_ref().is_some_and(|f| {
                    !f.caught && definition.key_threshold.is_some_and(|t| f.score >= t)
                }) || !first.as_ref().is_some_and(|f| f.caught)
                    && vote.is_some_and(|v| {
                        definition.memory_threshold.is_some_and(|t| v.score >= t)
                            && !(v.voted
                                && definition
                                    .memory_floor
                                    .is_some_and(|floor| v.nearest < floor))
                    });
                let class = level.of(settled);
                let keys = if key_looked {
                    coarse_groups::keys(&key_memory.keyed_values(&asked), level)
                } else {
                    None
                };
                let voted_class = coarse_groups::vote(&found, votes, level);
                let step = CoarseStep {
                    value_answered,
                    keys: keys.as_ref().map(|(c, score, _)| (*score, *c == class)),
                    vote: voted_class.as_ref().map(|(c, score, nearest, voted)| Vote {
                        score: *score,
                        correct: *c == class,
                        nearest: nearest.similarity,
                        voted: *voted,
                    }),
                };
                let look = CoarseLook {
                    value_answered,
                    keys: keys.map(|(c, strength, key)| KeyLook {
                        strength,
                        correct: Some(c == class),
                        value: c,
                        key: key.to_string(),
                        caught: false,
                    }),
                    vote: voted_class.map(|(c, score, nearest, voted)| SimilarLook {
                        score,
                        correct: Some(c == class),
                        value: c,
                        record: nearest.source,
                        nearest: nearest.similarity,
                        voted,
                    }),
                };
                (Some(step), Some(look))
            }
            _ => (None, None),
        };

        // A settlement not counted is replayed into memory like any other, but not asked about.
        let weight = weight(record, attribute, counting);
        let counted = weight > 0.0;
        sink(&DecisionRecord {
            record: record.id.clone(),
            attribute: attribute.to_owned(),
            order: Some(order as u64),
            weight,
            key: first.as_ref().map(|f| KeyLook {
                strength: f.score,
                value: f.value.clone(),
                correct: Some(f.matches),
                key: f.key.clone(),
                caught: f.caught,
            }),
            similar: decided
                .as_ref()
                .zip(vote)
                .map(|((winner, _), v)| SimilarLook {
                    score: v.score,
                    value: winner.answer.clone(),
                    record: winner.source.clone(),
                    correct: Some(v.correct),
                    nearest: v.nearest,
                    voted: v.voted,
                }),
            coarse: coarse_look,
            typed: None,
            answered: None,
            settled: Some(settled.clone()),
            settlement: record.settlements.get(attribute).copied(),
            probability: record.probabilities.get(attribute).copied(),
        });
        steps.push(Step {
            key_looked: key_looked && counted,
            key: first,
            looked: remembered > 0 && counted,
            vote,
            weight,
            order,
            coarse: coarse_step,
        });
        key_memory.put(schema, record);

        let case = text::normalize(&evidence);
        if let Some(earlier) = cases.get(&case) {
            memory.forget(&task, earlier);
            case_of.remove(earlier);
            remembered -= 1;
        }

        cases.insert(case.clone(), record.id.clone());
        case_of.insert(record.id.clone(), case);
        memory.remember(&task, &record.id, &evidence, settled);
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
    pooled: Option<Option<f64>>,
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
    let mut replay = fit_recent(
        &answers,
        looked.len(),
        looked.iter().fold(0.0, |sum, s| sum + s.weight),
        latest,
        target_precision,
        minimum_answered,
    );
    if let Some(threshold) = pooled {
        replay = ThresholdReplay {
            chosen: threshold.and_then(|t| {
                at(
                    &answers,
                    looked.len(),
                    looked.iter().fold(0.0, |sum, s| sum + s.weight),
                    latest,
                    t,
                    minimum_answered,
                )
            }),
            ..replay
        };
    }

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

/// One set of attributes tried for a decided attribute's key attributes, and how its key layer replayed.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyFieldsTrial {
    /// The attributes that keyed the decided attribute in this replay; empty for every attribute it rests on.
    pub key_fields: Vec<String>,
    /// The key layer's replay with them, as [`select_key_threshold`] makes it.
    pub replay: ThresholdReplay,
    /// When typing was counted, the typed thresholds with these attributes and their key threshold, as
    /// [`select_typed_key_thresholds`] makes them.
    pub typed: Option<TypedKeyThresholds>,
}

/// The attributes that should key a decided attribute in the settled key memory, chosen by replaying its key layer, and the
/// replays the choice was made from.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyFieldsChoice {
    /// The attributes to name, in the order they were added; `None` when no narrower set gained more than every attribute the
    /// attribute rests on — leave the key attributes unset.
    pub key_fields: Option<Vec<String>>,
    /// The key layer's replay with the choice.
    pub chosen: ThresholdReplay,
    /// The key layer's replay keyed by every attribute the attribute rests on.
    pub all_fields: ThresholdReplay,
    /// Every set tried: each attribute it rests on alone, in the schema's order, then each set the greedy additions tried.
    pub trials: Vec<KeyFieldsTrial>,
    /// When typing was counted, the typed thresholds that go with the choice.
    pub typed: Option<TypedKeyThresholds>,
}

/// The schema with `attribute` keyed by `key_fields` (every attribute it rests on when `None`) — attributes that support
/// it, so the schema stays valid.
fn keyed_by(schema: &Schema, attribute: &str, key_fields: Option<Vec<String>>) -> Schema {
    schema.derived(attribute, |f| f.key_fields = key_fields)
}

/// Chooses the attributes that should key `attribute` by the gain their chosen key threshold gives over the target precision —
/// the precision above the target times the answers' weight. Replays the key layer as [`select_key_threshold`] does with
/// each attribute the attribute rests on alone, ranks them by gain (a tie keeps the schema's order), then adds them in that order
/// while each addition gains more. The result names that set only if it gains more than every such attribute together.
/// A fresh default key memory serves each replay. With `typed_longest` above 0, each set is counted by its answers
/// before typing and while typing up to that many characters together ([`select_typed_key_thresholds`], with the set's
/// key threshold), and the typed thresholds for the choice come with it.
///
/// # Panics
/// As [`select_key_threshold`], and when typing is counted for an attribute that takes several values.
pub fn select_key_fields(
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    choice: Choice,
    typed_longest: usize,
) -> KeyFieldsChoice {
    assert!(
        typed_longest == 0 || !schema.attribute(attribute).multiple,
        "'{attribute}' takes several values; typing is not counted for it"
    );
    let replay = |key_fields: Option<Vec<String>>| {
        let resting = keyed_by(schema, attribute, key_fields.clone());
        let replay = select_key_threshold(
            &mut KeyMemory::default(),
            &resting,
            attribute,
            records,
            choice,
        );
        let typed = (typed_longest > 0).then(|| {
            // Any key threshold keeps the schema valid.
            let resting = resting.derived(attribute, |f| {
                f.key_threshold = replay.chosen.as_ref().map(|c| c.threshold);
            });
            select_typed_key_thresholds(
                &mut KeyMemory::default(),
                &resting,
                attribute,
                records,
                choice,
                typed_longest,
            )
        });
        KeyFieldsTrial {
            replay,
            key_fields: key_fields.unwrap_or_default(),
            typed,
        }
    };
    // Weighted: with checks, the decisions they stand for.
    let gain = |chosen: Option<&ThresholdChoice>| {
        chosen.map_or(0.0, |c| {
            (c.precision - choice.target_precision) * c.answered_weight
        })
    };
    let gains = |trial: &KeyFieldsTrial| {
        gain(trial.replay.chosen.as_ref())
            + trial.typed.as_ref().map_or(0.0, |t| {
                t.by_length
                    .iter()
                    .fold(0.0, |sum, r| sum + gain(r.chosen.as_ref()))
            })
    };

    let mut trials: Vec<KeyFieldsTrial> = Vec::new();
    let all = replay(None);
    let evidence: Vec<String> = schema
        .attributes()
        .iter()
        .filter(|f| schema.supports(&f.name, attribute))
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
        typed: all.typed.clone(),
    };
    let Some(mut best) = ranked.first().cloned() else {
        return every_field(trials);
    };
    if gains(&best) == 0.0 {
        return every_field(trials);
    }

    for next in &ranked[1..] {
        if best.key_fields.len() + 1 == evidence.len() {
            break; // every attribute it rests on: the replay without key attributes
        }

        let mut attributes = best.key_fields.clone();
        attributes.push(next.key_fields[0].clone());
        let trial = replay(Some(attributes));
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
            typed: best.typed,
        }
    } else {
        every_field(trials)
    }
}

/// Chooses, for each attribute that keys `attribute`, how readily its keys are believed after only a few settlements. Replays
/// the records in the order the key memory weighs them; before each is remembered, every key its values give the
/// attribute that was seen before predicts the attribute's value as a Chinese restaurant process does — a value of weight w
/// under a key of weighted total W by w ÷ (W + α), a value not yet seen under the key by α ÷ (W + α) — and each key
/// attribute takes the α under which its keys' predictions are likeliest, with two anchoring predictions from a key seen
/// once. The keys are those the schema gives the attribute, from the values known when its suggestion would have been made;
/// the attribute's own concentrations are ignored. `memory` must be empty and configured as the one in use.
///
/// Returns a concentration for each key attribute whose keys were seen again; empty for an attribute that takes several values.
///
/// A concentration under 0.01 means each key points at one value: a reference to the very thing decided, or an attribute
/// written from the decided value itself (a copy, an identifier per value), which may be missing while the record is still
/// open. Check that such an attribute is there when an answer is asked for, or keep the record's arrival order so a replay
/// looks it up without it.
///
/// # Panics
/// When the attribute is not decided.
pub fn select_key_concentrations(
    memory: &mut KeyMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
) -> BTreeMap<String, f64> {
    assert!(
        schema.attribute(attribute).role == AttributeRole::Decided,
        "'{attribute}' is not a decided attribute"
    );
    let mut concentrations = BTreeMap::new();
    if schema.attribute(attribute).multiple {
        return concentrations;
    }

    // No key concentrations keeps the schema valid.
    let schema = schema.derived(attribute, |f| f.key_concentrations.clear());
    let mut ordered: Vec<Record> = records.iter().map(Record::remembered).collect();
    ordered.sort_by(|a, b| {
        a.settled_at
            .cmp(&b.settled_at)
            .then_with(|| a.id.cmp(&b.id))
    });

    // key attribute → (predictions of a new value, the totals predicted from), in record order.
    let mut predictions: BTreeMap<String, (u64, Vec<f64>)> = BTreeMap::new();
    for record in &ordered {
        let Some(settled) = record.values.get(attribute) else {
            continue;
        };

        memory.remove(schema.name(), &record.id); // a record settled again is not its own evidence
        let before = arrived(&schema, record, attribute, &record.values);
        for (key_field, total, weight) in
            memory.key_totals(&Lookup::new(&schema, attribute, &before), settled)
        {
            let (unseen, totals) = predictions.entry(key_field).or_default();
            totals.push(total);
            if weight == 0.0 {
                *unseen += 1;
            }
        }

        memory.put(&schema, record);
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
