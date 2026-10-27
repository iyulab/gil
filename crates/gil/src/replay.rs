//! Chooses a decided attribute's key threshold by replaying settled records: each record, in settlement order, is asked
//! about with the values that had arrived before the attribute, against a key memory holding the records settled before
//! it, then remembered. Precision is fitted as a non-decreasing function of the score, and the threshold is the lowest
//! score down to which every fitted band of answers meets the target precision.
//!
//! Every sum here is taken in a fixed order — the same operations on the same operands — so the numbers come out the
//! same to the bit on every run and every platform.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::decision_record::{CoarseLook, DecisionRecord, KeyLook, SimilarLook, TypedLook};
use crate::key_memory::{First, KeyMemory, Lookup};
use crate::lexical_memory::LexicalMemory;
use crate::schema::{Attribute, AttributeRole, Record, Schema, SettlementKind};
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

/// How many times [`BatchCheck`] splits the latest records in two, each split chosen on one half and counted on the other,
/// both ways.
pub const BATCH_SPLITS: usize = 20;

/// The seed of [`BatchCheck`]'s splits, so the same records split the same way on every run.
const BATCH_SEED: u64 = 0x6769_6c5f_6261_7463;

/// The fewest of the latest decisions a settled value needs before [`ValueTail`] reports it — below that, a share of a
/// few decisions says nothing about the next.
pub const VALUE_TAIL_DECISIONS: usize = 30;

/// How [`select_layers`] chooses an attribute's two thresholds ([`LayerOptions::rule`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ThresholdRule {
    /// Each layer on its own: the lowest threshold down to which every band of its answers meets the target precision.
    /// No band of weak answers is admitted on the strength of good ones above it, so the attribute answers less than it
    /// could while keeping its promise. The default.
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
    /// Experimental. The answers that cost least per decision, reading the target as where answering pays: a wrong
    /// answer costs ρ = 1 / (1 − target) values entered with no suggestion — the `Choice` reading of the target, turned
    /// around — so an answer pays when the chance it is right is above the target. Each layer's best value gets a
    /// calibrated chance of being right, fitted on the latest [`RECENT_RECORDS`] records' lookups by isotonic regression
    /// of whether it matched on its score, layer by layer; of the two layers the more likely answers, and one chance
    /// threshold is chosen where answering at or above it costs least over those lookups, resting on at least the
    /// minimum number of answers. The answers it chooses are right at least the target's share of the time on the
    /// lookups it was chosen on, on average — not in every band. The schema then needs the calibration to answer as the
    /// rule chose ([`LayerThresholds::calibration`]). In replays on public recall records and public device event reports
    /// at targets of 0.5 and 0.75 — a wrong answer costing two and four entries — it cost 0.7–15.2 points less per
    /// decision than thresholds chosen layer by layer for the same cost; at other targets, and on the other collections
    /// tried, it was never dearer beyond chance.
    LeastCost,
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
    pub(crate) fn check(&self) {
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

/// Precision as a non-decreasing function of the score: isotonic regression of whether each answer matched on its score,
/// by pooling adjacent violators. The blocks go from the lowest score up; a block's precision is above the one below it.
fn pool(matches: &[Answer]) -> Vec<Block> {
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
    blocks
}

/// Fits precision as a non-decreasing function of the score ([`pool`]), then walks the fitted blocks from the highest score down while each block's own precision meets the target. The threshold
/// is the lowest score of the last such block; the most precise is the first block, from the top, whose answers above it
/// are worth `minimum_answered`.
fn fit(
    matches: &[Answer],
    lookups: usize,
    lookup_weight: f64,
    target_precision: f64,
    minimum_answered: usize,
) -> ThresholdReplay {
    let blocks = pool(matches);
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
    /// How much the batches the latest records were settled in flatter the promise; `None` when none of them names its
    /// batch ([`Record::batch`]).
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub batches: Option<BatchCheck>,
    /// Experimental. Value by settled value, how the chosen thresholds answered the latest records settled as it
    /// ([`ValueTail`]); `None` unless asked for ([`LayerOptions::value_tails`]).
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub value_tails: Option<Vec<ValueTail>>,
    /// Experimental. The calibration [`ThresholdRule::LeastCost`] chose with; `None` under the other rules.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub calibration: Option<Calibration>,
}

/// Experimental. What [`ThresholdRule::LeastCost`] answers with: each layer's best value's calibrated chance of being
/// right, as a step function of its score, and the chance at or above which the more likely of the two layers answers.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Calibration {
    /// The key layer's steps, by rising score; empty when it had no best value to fit.
    pub key: Vec<CalibrationStep>,
    /// The similar record layer's steps, by rising score; empty when it had no vote to fit.
    pub memory: Vec<CalibrationStep>,
    /// The chance at or above which an answer is given; `None` when no chance paid on enough answers.
    pub threshold: Option<f64>,
}

/// One step of a calibration: from `score` up to the next step's, the chance of being right is `chance`. Below the
/// first step's score, the first step's chance holds.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CalibrationStep {
    pub score: f64,
    pub chance: f64,
}

impl Calibration {
    /// The chance a layer's best value at `score` is right, by `steps`; `None` with no steps.
    pub fn chance(steps: &[CalibrationStep], score: f64) -> Option<f64> {
        let first = steps.first()?;
        Some(
            steps
                .iter()
                .take_while(|s| s.score <= score)
                .last()
                .unwrap_or(first)
                .chance,
        )
    }

    /// The lowest score at which `steps` reach `threshold` — where the layer can answer at all; `None` when they never
    /// do.
    fn reach(steps: &[CalibrationStep], threshold: f64) -> Option<f64> {
        steps
            .iter()
            .find(|s| s.chance >= threshold)
            .map(|s| s.score)
    }
}

/// Experimental. How the chosen thresholds of both layers together answered the latest [`RECENT_RECORDS`] records
/// settled as one value. A precision promised over the whole attribute is an average over its values, and a value the
/// layers keep taking for another can sit far below it while the average holds: `answered_otherwise` is that value's
/// own share. In two forward replays on public collections, the values whose share was 0.20 or more on the first half
/// of the records were mostly still at 0.10 or more on the second (1.00 and 0.88 of them, against 0.10 and 0.06 of the
/// other values) — candidates for a value the attribute must not miss. Reported only for values settled on at least
/// [`VALUE_TAIL_DECISIONS`] of those records, in value order.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ValueTail {
    /// The settled value.
    pub value: String,
    /// How many of the latest records the replay asked about were settled as it.
    pub decisions: usize,
    /// The share of those the thresholds answered with another value.
    pub answered_otherwise: f64,
    /// The value given in its place most often; `None` when none was. Ties go to the first in value order.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub given_instead: Option<String>,
}

/// What the batches among the latest [`RECENT_RECORDS`] records do to a promise. Records settled together — several at
/// once from one source, alike in what they show — can be right or wrong together, and the replay then counts more
/// independent answers than there are. The latest records are split in two [`BATCH_SPLITS`] times; on each split the
/// thresholds are chosen on one half, by the same rule, and the answers they give on the other half are counted, both
/// ways. Split by batch, no batch is on both sides; split by record, batch mates are. Where `across_records` is above
/// `across_batches` by more than chance, the batches flatter a promise taken on records they span, and `across_batches`
/// is the closer count of what the next batches will get. Both are counts of the choosing, not of the thresholds chosen
/// on all the records: on records whose batches are not right or wrong together, they can sit above the precision the
/// next records get by more than the promise does. A record that names no batch is a batch of its own.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BatchCheck {
    /// The share of the answers counted across halves of the batches that matched, weighted; `None` when no half chose
    /// thresholds that answered.
    pub across_batches: Option<f64>,
    /// How many answers `across_batches` counts, over every split and both ways.
    pub across_batches_answered: usize,
    /// The share of the answers counted across halves of the records that matched, weighted; `None` as for
    /// `across_batches`.
    pub across_records: Option<f64>,
    /// How many answers `across_records` counts.
    pub across_records_answered: usize,
    /// How many batches the latest records came in, a record that names none counting as one.
    pub batches: usize,
    /// How many answers a batch holding more than one holds, in the mean that weighs batch sizes as the correlation does
    /// (n₀ of a one-way analysis of variance); `None` when fewer than two batches hold more than one answer.
    pub batch_size: Option<f64>,
    /// How much the answers of one batch are right or wrong together, under the thresholds chosen on all the records:
    /// the intraclass correlation ICC(1) of whether an answer matched, over the batches holding more than one answer;
    /// `None` as for `batch_size`.
    pub correlation: Option<f64>,
    /// `1 + (batch_size − 1) × correlation` — how many answers count as one independent answer. Near one, batches flatter
    /// nothing; well above it (seven on public device event reports that flattered a promise by 1.6 points), they do.
    pub design_effect: Option<f64>,
}

/// How [`select_layers`] chooses and what it reports along the way.
#[derive(Default)]
pub struct LayerOptions<'a> {
    /// How the two thresholds are chosen.
    pub rule: ThresholdRule,
    /// Handed the decision record of every record the replay asks about, in settlement order: what the key layer and
    /// the similar record layer made of it whatever the thresholds, and what it was settled as. The thresholds are
    /// chosen from exactly these records.
    pub sink: Option<&'a mut dyn FnMut(&DecisionRecord)>,
    /// Experimental. Whether to report, value by settled value, how the chosen thresholds answered
    /// ([`LayerThresholds::value_tails`]).
    pub value_tails: bool,
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
pub(crate) struct Step {
    key_looked: bool,
    key: Option<First>,
    looked: bool,
    vote: Option<Vote>,
    weight: f64,
    order: usize,
    /// The value the record was settled as.
    settled: String,
    /// The value the similar record layer voted for, when it voted.
    voted_for: Option<String>,
    /// The batch the record was settled in, when it names one.
    batch: Option<String>,
    /// Whether no other record settled before it named its batch — the first of its batch, as one that names none is.
    pub(crate) first: bool,
    /// For a coarse replay: the level as the keys and the vote back it, and whether the attribute's value layers answered.
    coarse: Option<CoarseStep>,
}

/// What an attribute's coarse level made of one lookup: the score of the class the keys back and whether it was the
/// settled value's, and how the similar records voted for a class. Whether the value layers answered first depends on
/// their thresholds, so it is read off the step when the level's thresholds are chosen ([`value_answered`]).
#[derive(Clone, Copy, Debug)]
struct CoarseStep {
    keys: Option<(f64, bool)>,
    vote: Option<Vote>,
}

/// The share of a threshold's answers whose nearest voter may fall below its similarity floor.
const FLOOR_SHARE: f64 = 0.01;

/// Why the layer selections refuse an attribute that takes several values, and where its key layer's threshold is
/// chosen instead.
fn several_values_refusal(attribute: &str) -> String {
    format!(
        "'{attribute}' takes several values; the similar record layer does not answer it — choose the key layer's \
         threshold with select_key_fields"
    )
}

/// Chooses `attribute`'s key threshold and memory threshold together by replaying `records` through both memories, which
/// must be empty and configured as the ones in use. Each record, in settlement order, is asked about before it is put
/// in: the key layer as [`select_key_threshold`] asks it, and the similar record layer with the evidence of the values
/// that had arrived before the attribute, voting among the attribute's `similar_votes` nearest records. The record
/// memory keeps one record per case — evidence that reads the same after normalization — the latest settled. The key
/// threshold is chosen on every lookup; the memory threshold on the lookups the chosen key threshold would not have
/// answered, and with more than one voter each memory choice carries its similarity floor.
///
/// By [`ThresholdRule::Pooled`] each layer's `chosen` is that layer's half of the pair, `None` when the pair leaves the
/// layer silent; its precision, answer rate and promise are the layer's own, over the whole replay, and the precision of
/// the two layers' answers together held the target on the lookups the pair was chosen on. `most_precise` is as under
/// [`ThresholdRule::EveryBand`]. The attribute's thresholds as they stand in `schema` (`key_threshold`,
/// `memory_threshold`, `memory_floor`) decide only how far back the rule reaches.
///
/// When any of the latest [`RECENT_RECORDS`] records names its batch, `batches` says what the batches do to the promise
/// ([`BatchCheck`]).
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
    options: LayerOptions,
) -> LayerThresholds {
    choice.check();
    let definition = schema.attribute(attribute);
    assert!(
        definition.role == AttributeRole::Decided,
        "'{attribute}' is not a decided attribute"
    );
    assert!(
        !definition.multiple,
        "{}",
        several_values_refusal(attribute)
    );

    let mut ignore = |_: &DecisionRecord| {};
    let (steps, latest) = replay(
        key_memory,
        memory,
        schema,
        attribute,
        records,
        choice.counting,
        Replaying::Values(options.sink.unwrap_or(&mut ignore)),
    );
    let thresholds = layers(&steps, latest, definition, choice, options.rule);
    let batches = batch_check(
        &steps,
        latest,
        definition,
        choice,
        options.rule,
        &thresholds,
    );
    let value_tails = options
        .value_tails
        .then(|| value_tails(&steps, latest, &thresholds));
    LayerThresholds {
        batches,
        value_tails,
        ..thresholds
    }
}

/// [`ValueTail`] for each value settled on at least [`VALUE_TAIL_DECISIONS`] of the latest [`RECENT_RECORDS`] records
/// the replay asked about, under `thresholds` as a decider answers with them: the key layer at its threshold, then,
/// where it does not answer, the similar record layer at its threshold and floor — and neither under a key that settled
/// a value to catch.
pub(crate) fn value_tails(
    steps: &[Step],
    latest: usize,
    thresholds: &LayerThresholds,
) -> Vec<ValueTail> {
    let key = thresholds.key.chosen.as_ref().map(|c| c.threshold);
    let memory = thresholds.memory.chosen.as_ref();
    let mut tally: BTreeMap<&str, (usize, usize, BTreeMap<&str, usize>)> = BTreeMap::new();
    for s in steps.iter().filter(|s| {
        (s.key_looked || s.looked) && s.order as i64 > latest as i64 - RECENT_RECORDS as i64
    }) {
        let caught = s.key.as_ref().is_some_and(|first| first.caught);
        let by_key = s
            .key
            .as_ref()
            .filter(|first| s.key_looked && !first.caught && key.is_some_and(|t| first.score >= t))
            .map(|first| first.value.as_str());
        let answer = by_key.or_else(|| {
            let v = s.vote.filter(|_| s.looked && !caught)?;
            let m = memory?;
            let answers = v.score >= m.threshold
                && !(v.voted && m.similarity_floor.is_some_and(|floor| v.nearest < floor));
            s.voted_for.as_deref().filter(|_| answers)
        });
        let (decisions, otherwise, given) = tally.entry(s.settled.as_str()).or_default();
        *decisions += 1;
        if let Some(a) = answer.filter(|a| *a != s.settled) {
            *otherwise += 1;
            *given.entry(a).or_default() += 1;
        }
    }
    tally
        .into_iter()
        .filter(|(_, (decisions, _, _))| *decisions >= VALUE_TAIL_DECISIONS)
        .map(|(value, (decisions, otherwise, given))| ValueTail {
            value: value.to_owned(),
            decisions,
            answered_otherwise: otherwise as f64 / decisions as f64,
            // The most given, the first in value order among ties.
            given_instead: given
                .iter()
                .fold(None::<(&str, usize)>, |best, (v, n)| match best {
                    Some((_, m)) if m >= *n => best,
                    _ => Some((v, *n)),
                })
                .map(|(v, _)| v.to_owned()),
        })
        .collect()
}

/// A layers replay's steps with the lookups of records in the other situation — first of their batch when `first` is
/// false, not first when it is — left uncounted, as a settlement kind not counted leaves them: in memory, not asked
/// about.
pub(crate) fn in_situation(steps: &[Step], first: bool) -> Vec<Step> {
    steps
        .iter()
        .map(|s| {
            if s.first == first {
                s.clone()
            } else {
                Step {
                    key_looked: false,
                    looked: false,
                    weight: 0.0,
                    ..s.clone()
                }
            }
        })
        .collect()
}

/// The layer thresholds chosen on a layers replay's steps, `batches` left out.
pub(crate) fn layers(
    steps: &[Step],
    latest: usize,
    definition: &crate::schema::Attribute,
    choice: Choice,
    rule: ThresholdRule,
) -> LayerThresholds {
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
            steps,
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
    if rule == ThresholdRule::LeastCost {
        return least_cost(steps, latest, choice, key, memory);
    }
    LayerThresholds {
        key,
        memory,
        batches: None,
        value_tails: None,
        calibration: None,
    }
}

/// [`ThresholdRule::LeastCost`] over the latest [`RECENT_RECORDS`] records' lookups: `key` and `memory` are the layers'
/// replays by [`ThresholdRule::EveryBand`], whose `chosen` it replaces with each layer's share of the answers the rule
/// gives — the lowest score at which the layer's chance reaches the threshold, with the precision, answer rate and
/// promise of the answers it gave. Neither layer answers under a key that settled a value to catch.
fn least_cost(
    steps: &[Step],
    latest: usize,
    choice: Choice,
    key: ThresholdReplay,
    memory: ThresholdReplay,
) -> LayerThresholds {
    let recent: Vec<&Step> = steps
        .iter()
        .filter(|s| s.order as i64 > latest as i64 - RECENT_RECORDS as i64)
        .collect();
    let caught = |s: &Step| s.key.as_ref().is_some_and(|first| first.caught);
    let by_key = |s: &Step| {
        s.key
            .as_ref()
            .filter(|first| s.key_looked && !first.caught)
            .map(|first| (first.score, first.matches))
    };
    let by_vote = |s: &Step| {
        s.vote
            .filter(|_| s.looked && !caught(s))
            .map(|v| (v.score, v.correct))
    };
    let fitted = |of: &dyn Fn(&Step) -> Option<(f64, bool)>| -> Vec<CalibrationStep> {
        let answers: Vec<Answer> = recent
            .iter()
            .filter_map(|s| {
                of(s).map(|(score, correct)| Answer {
                    score,
                    correct,
                    weight: s.weight,
                    order: s.order,
                    key: None,
                    caught: false,
                })
            })
            .collect();
        pool(&answers)
            .iter()
            .map(|b| CalibrationStep {
                score: b.lowest,
                chance: b.precision(),
            })
            .collect()
    };
    let key_steps = fitted(&by_key);
    let memory_steps = fitted(&by_vote);

    // Each lookup's answer: the more likely layer, the key layer on a tie — its chance, whether it matched, whether it
    // is the key layer's, and the lookup's weight.
    let candidates: Vec<(f64, bool, bool, f64)> = recent
        .iter()
        .filter_map(|s| {
            let k = by_key(s)
                .and_then(|(score, ok)| Calibration::chance(&key_steps, score).map(|c| (c, ok)));
            let m = by_vote(s)
                .and_then(|(score, ok)| Calibration::chance(&memory_steps, score).map(|c| (c, ok)));
            match (k, m) {
                (Some(k), Some(m)) if m.0 > k.0 => Some((m.0, m.1, false, s.weight)),
                (Some(k), _) => Some((k.0, k.1, true, s.weight)),
                (None, Some(m)) => Some((m.0, m.1, false, s.weight)),
                (None, None) => None,
            }
        })
        .collect();

    // Answering at or above a chance gains one entry saved when right and 1 − ρ when wrong; the threshold gains most,
    // the higher on a tie, among those resting on the minimum number of answers.
    let rho = 1.0 / (1.0 - choice.target_precision);
    let mut order: Vec<&(f64, bool, bool, f64)> = candidates.iter().collect();
    order.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));
    let (mut gain, mut weight, mut squares) = (0.0_f64, 0.0_f64, 0.0_f64);
    let (mut best, mut threshold) = (0.0_f64, None);
    for group in order.chunk_by(|a, b| a.0 == b.0) {
        for (_, ok, _, w) in group {
            gain += if *ok { *w } else { (1.0 - rho) * *w };
            weight += *w;
            squares += *w * *w;
        }
        if weight > 0.0
            && weight * weight / squares >= choice.minimum_answered as f64
            && gain > best + 1e-12
        {
            best = gain;
            threshold = Some(group[0].0);
        }
    }

    // Each layer's share of the answers given.
    let share = |of_key: bool, steps: &[CalibrationStep], replay: ThresholdReplay| {
        let chosen = threshold.and_then(|t| {
            let reach = Calibration::reach(steps, t)?;
            let looked = recent
                .iter()
                .filter(|s| if of_key { s.key_looked } else { s.looked });
            let lookups = looked.clone().count();
            let lookup_weight = looked.fold(0.0, |sum, s| sum + s.weight);
            let given: Vec<&(f64, bool, bool, f64)> = candidates
                .iter()
                .filter(|c| c.2 == of_key && c.0 >= t)
                .collect();
            let answered_weight = given.iter().fold(0.0, |sum, c| sum + c.3);
            if answered_weight <= 0.0 {
                return None;
            }
            let correct = given.iter().filter(|c| c.1).fold(0.0, |sum, c| sum + c.3);
            let effective = answered_weight * answered_weight
                / given.iter().fold(0.0, |sum, c| sum + c.3 * c.3);
            let precision = correct / answered_weight;
            Some(ThresholdChoice {
                threshold: reach,
                precision,
                answer_rate: answered_weight / lookup_weight,
                answered: given.len(),
                lookups,
                answered_weight,
                effective_answered: effective,
                similarity_floor: None,
                promise: lower(precision, effective),
                promise_answered: effective,
            })
        });
        ThresholdReplay { chosen, ..replay }
    };
    let key = share(true, &key_steps, key);
    let memory = share(false, &memory_steps, memory);
    LayerThresholds {
        key,
        memory,
        batches: None,
        value_tails: None,
        calibration: Some(Calibration {
            key: key_steps,
            memory: memory_steps,
            threshold,
        }),
    }
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
    answer(
        step,
        definition.key_threshold,
        definition.memory_threshold,
        definition.memory_floor,
    )
    .is_some()
}

/// Whether the lookup's answer at these thresholds matched, as a suggestion would have answered it; `None` when it
/// would not have been answered — a lookup under a key that settled a value to catch never is.
fn answer(
    step: &Step,
    key_threshold: Option<f64>,
    memory_threshold: Option<f64>,
    memory_floor: Option<f64>,
) -> Option<bool> {
    if step.key.as_ref().is_some_and(|k| k.caught) {
        return None;
    }

    if let (true, Some(k), Some(t)) = (step.key_looked, step.key.as_ref(), key_threshold)
        && k.score >= t
    {
        return Some(k.matches);
    }

    match (step.vote, memory_threshold) {
        (Some(v), Some(t))
            if step.looked
                && v.score >= t
                && !(v.voted && memory_floor.is_some_and(|floor| v.nearest < floor)) =>
        {
            Some(v.correct)
        }
        _ => None,
    }
}

/// The thresholds a layers choice answers with: the key threshold, the memory threshold and its floor.
fn answering(thresholds: &LayerThresholds) -> (Option<f64>, Option<f64>, Option<f64>) {
    let memory = thresholds.memory.chosen.as_ref();
    (
        thresholds.key.chosen.as_ref().map(|c| c.threshold),
        memory.map(|c| c.threshold),
        memory.and_then(|c| c.similarity_floor),
    )
}

/// [`BatchCheck`] over the steps of the latest [`RECENT_RECORDS`] records; `None` when none of them names its batch.
pub(crate) fn batch_check(
    steps: &[Step],
    latest: usize,
    definition: &crate::schema::Attribute,
    choice: Choice,
    rule: ThresholdRule,
    thresholds: &LayerThresholds,
) -> Option<BatchCheck> {
    let recent: Vec<&Step> = steps
        .iter()
        .filter(|s| s.order as i64 > latest as i64 - RECENT_RECORDS as i64)
        .collect();
    if !recent.iter().any(|s| s.batch.is_some()) {
        return None;
    }

    // Each step's batch as a number, in the order the batches first come; a step that names none is one of its own.
    let mut numbers: HashMap<&str, usize> = HashMap::new();
    let mut batch_of = Vec::with_capacity(recent.len());
    let mut batches = 0_usize;
    for step in &recent {
        let number = match step.batch.as_deref() {
            Some(name) => *numbers.entry(name).or_insert_with(|| {
                batches += 1;
                batches - 1
            }),
            None => {
                batches += 1;
                batches - 1
            }
        };
        batch_of.push(number);
    }

    let mut random = SplitMix64(BATCH_SEED);
    let mut across_batches = Tally::default();
    let mut across_records = Tally::default();
    for _ in 0..BATCH_SPLITS {
        let by_batch: Vec<bool> = (0..batches).map(|_| random.coin()).collect();
        let halves: Vec<bool> = batch_of.iter().map(|&b| by_batch[b]).collect();
        across_batches.cross(&recent, &halves, latest, definition, choice, rule);
        let halves: Vec<bool> = recent.iter().map(|_| random.coin()).collect();
        across_records.cross(&recent, &halves, latest, definition, choice, rule);
    }

    // Whether each answer under the thresholds chosen on all the records matched, by batch.
    let (key, similar, floor) = answering(thresholds);
    let mut by_batch: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    for (step, &batch) in recent.iter().zip(&batch_of) {
        if let Some(correct) = answer(step, key, similar, floor) {
            let (count, right) = by_batch.entry(batch).or_insert((0, 0));
            *count += 1;
            *right += usize::from(correct);
        }
    }
    let (batch_size, correlation) = intraclass(by_batch.values().copied().filter(|(n, _)| *n >= 2));

    Some(BatchCheck {
        across_batches: across_batches.precision(),
        across_batches_answered: across_batches.answered,
        across_records: across_records.precision(),
        across_records_answered: across_records.answered,
        batches,
        batch_size,
        correlation,
        design_effect: batch_size
            .zip(correlation)
            .map(|(n, icc)| 1.0 + ((n - 1.0) * icc)),
    })
}

/// Answers counted across halves: how many, their weight and the weight of those that matched.
#[derive(Default)]
struct Tally {
    answered: usize,
    weight: f64,
    correct: f64,
}

impl Tally {
    /// Chooses on each half of `steps` (`halves` says which) and counts the other half's answers at its thresholds.
    fn cross(
        &mut self,
        steps: &[&Step],
        halves: &[bool],
        latest: usize,
        definition: &crate::schema::Attribute,
        choice: Choice,
        rule: ThresholdRule,
    ) {
        for side in [false, true] {
            let fitted: Vec<Step> = steps
                .iter()
                .zip(halves)
                .filter(|(_, h)| **h == side)
                .map(|(s, _)| (*s).clone())
                .collect();
            let (key, similar, floor) =
                answering(&layers(&fitted, latest, definition, choice, rule));
            for (step, _) in steps.iter().zip(halves).filter(|(_, h)| **h != side) {
                if let Some(correct) = answer(step, key, similar, floor) {
                    self.answered += 1;
                    self.weight += step.weight;
                    self.correct += if correct { step.weight } else { 0.0 };
                }
            }
        }
    }

    fn precision(&self) -> Option<f64> {
        (self.weight > 0.0).then(|| self.correct / self.weight)
    }
}

/// The n₀-adjusted mean size and the ICC(1) of a one-way analysis of variance over groups given as (answers, matched);
/// both `None` with fewer than two groups or no answer beyond one per group.
fn intraclass(groups: impl Iterator<Item = (usize, usize)>) -> (Option<f64>, Option<f64>) {
    let groups: Vec<(usize, usize)> = groups.collect();
    let k = groups.len();
    let n: usize = groups.iter().map(|(count, _)| count).sum();
    if k < 2 || n <= k {
        return (None, None);
    }

    let right: usize = groups.iter().map(|(_, right)| right).sum();
    let grand = right as f64 / n as f64;
    let (mut between, mut within, mut squares) = (0.0_f64, 0.0_f64, 0.0_f64);
    for &(count, right) in &groups {
        let (count, right) = (count as f64, right as f64);
        let mean = right / count;
        between += count * (mean - grand) * (mean - grand);
        // Of 0/1 values: Σ (y − mean)² = right − count · mean².
        within += right - (count * mean * mean);
        squares += count * count;
    }

    let msb = between / (k - 1) as f64;
    let msw = within / (n - k) as f64;
    let n0 = (n as f64 - (squares / n as f64)) / (k - 1) as f64;
    let denominator = msb + ((n0 - 1.0) * msw);
    let icc = (denominator > 0.0).then(|| (msb - msw) / denominator);
    (Some(n0), icc)
}

/// SplitMix64 — a small generator with a fixed seed, so a split comes out the same on every platform.
struct SplitMix64(u64);

impl SplitMix64 {
    fn coin(&mut self) -> bool {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) & 1 == 1
    }
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
        "{}",
        several_values_refusal(attribute)
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
    coarse_layers(&steps, latest, definition, None, choice)
}

/// Whether `definition`'s value layers answered a lookup at its thresholds — the key layer with a strong enough key that
/// settled no value to catch, or the similar record layer with a strong enough vote whose nearest voter, when several
/// voted, clears the floor. Only where they do not is the coarse level offered.
fn value_answered(key: Option<&First>, vote: Option<Vote>, definition: &Attribute) -> bool {
    let caught = key.is_some_and(|first| first.caught);
    key.is_some_and(|f| !f.caught && definition.key_threshold.is_some_and(|t| f.score >= t))
        || !caught
            && vote.is_some_and(|v| {
                definition.memory_threshold.is_some_and(|t| v.score >= t)
                    && !(v.voted
                        && definition
                            .memory_floor
                            .is_some_and(|floor| v.nearest < floor))
            })
}

/// Chooses the coarse level's thresholds from a coarse replay's steps, on the lookups `definition`'s value layers leave
/// at the thresholds it has — what [`select_coarse`] does once it has replayed.
pub(crate) fn coarse_layers(
    steps: &[Step],
    latest: usize,
    definition: &Attribute,
    first_of_batch: Option<&Attribute>,
    choice: Choice,
) -> LayerThresholds {
    // What the level made of each lookup the value layers left, as a step whose vote is the class's — not one under a
    // key that settled a value to catch, where the level is not offered either.
    let left: Vec<(Step, Option<(f64, bool)>)> = steps
        .iter()
        .filter_map(|s| {
            let coarse = s.coarse.expect("a coarse replay's step");
            let caught = s.key.as_ref().is_some_and(|first| first.caught);
            // A first record of its batch was answered at the thresholds for one, when the attribute has them.
            let values = first_of_batch.filter(|_| s.first).unwrap_or(definition);
            let answered = value_answered(s.key.as_ref(), s.vote, values);
            ((s.key_looked || s.looked) && !answered && !caught).then(|| {
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
    LayerThresholds {
        key,
        memory,
        batches: None,
        value_tails: None,
        calibration: None,
    }
}

/// What a replay is for: the value layers or the coarse level as well, handing each lookup's decision record to a sink.
pub(crate) enum Replaying<'s> {
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
    let mut state = ReplayState::default();
    carry_on(
        &mut state, key_memory, memory, schema, attribute, records, counting, replaying,
    );
    (state.steps, state.order)
}

/// Where a replay stands: its steps, the records the record memory holds and the case each represents, how many
/// records it has gone through, and the last of them in settlement order.
#[derive(Default)]
pub(crate) struct ReplayState {
    pub(crate) steps: Vec<Step>,
    remembered: usize,
    cases: HashMap<String, String>,
    case_of: HashMap<String, String>,
    pub(crate) order: usize,
    pub(crate) last: Option<(i64, String)>,
    /// The records gone through by the batch they name.
    batches: HashMap<String, HashSet<String>>,
}

/// Carries a replay on through `records`, all settled after those it has gone through, with the memories it left —
/// the same steps, and the same memories, as replaying them all from the start.
#[allow(clippy::too_many_arguments)]
pub(crate) fn carry_on(
    state: &mut ReplayState,
    key_memory: &mut KeyMemory,
    memory: &mut LexicalMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    counting: Option<&[SettlementKind]>,
    replaying: Replaying,
) {
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

    if let (Some(last), Some(first)) = (&state.last, ordered.first()) {
        assert!(
            (first.settled_at, first.id.as_str()) > (last.0, last.1.as_str()),
            "records carried on with must be settled after those replayed"
        );
    }
    let mut steps = std::mem::take(&mut state.steps);
    let mut remembered = state.remembered;
    let mut cases = std::mem::take(&mut state.cases); // case → the record representing it
    let mut case_of = std::mem::take(&mut state.case_of); // record → the case it represents
    let mut order = state.order;
    for record in &ordered {
        order += 1;
        let first_of_batch = match &record.batch {
            None => true,
            Some(batch) => {
                let mates = state.batches.entry(batch.clone()).or_default();
                let first = !mates.iter().any(|r| r != &record.id);
                mates.insert(record.id.clone());
                first
            }
        };
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
                let value_answered = value_answered(first.as_ref(), vote, definition);
                let class = level.of(settled);
                let keys = if key_looked {
                    coarse_groups::keys(&key_memory.keyed_values(&asked), level)
                } else {
                    None
                };
                let voted_class = coarse_groups::vote(&found, votes, level);
                let step = CoarseStep {
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
            settled: settled.clone(),
            voted_for: decided.as_ref().map(|(winner, _)| winner.answer.clone()),
            batch: record.batch.clone(),
            first: first_of_batch,
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

    if let Some(last) = ordered.last() {
        state.last = Some((last.settled_at, last.id.clone()));
    }
    (
        state.steps,
        state.remembered,
        state.cases,
        state.case_of,
        state.order,
    ) = (steps, remembered, cases, case_of, order);
}

/// Where a replay of the similar record layer of an attribute that takes several values stands: the answers it would
/// have given at some threshold, the elements sought, the records the record memory holds and the case each represents,
/// how many records it has gone through, and the last of them in settlement order.
#[derive(Default)]
pub(crate) struct ElementState {
    answers: Vec<Answer>,
    lookups: usize,
    lookup_weight: f64,
    remembered: usize,
    cases: HashMap<String, String>,
    case_of: HashMap<String, String>,
    pub(crate) order: usize,
    pub(crate) last: Option<(i64, String)>,
}

/// Experimental. Chooses the threshold of `attribute`'s similar record layer, which answers an attribute that takes
/// several values by the share of the nearest records that hold each element ([`crate::schema::Attribute::similar_elements`]),
/// by replaying `records` through `memory`, which must be empty and configured as the one in use. Each record, in
/// settlement order, is asked about before it is put in — with the evidence of the values that had arrived before the
/// attribute, none of its own chosen — and every element offered counts as one answer at its share of the vote, right
/// when the record settled it. The record memory keeps one record per case, the latest settled. The threshold is chosen
/// on those answers as [`select_layers`] chooses the memory threshold: `lookups` counts the elements the records settled,
/// and the promise is about one element at a time, not about a whole set. A value the application filled is neither asked
/// about nor remembered.
///
/// # Panics
/// When `target_precision` is outside (0, 1], `minimum_answered` is 0, or the attribute is not a decided attribute
/// answered by similar elements.
pub fn select_similar_elements(
    memory: &mut LexicalMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    choice: Choice,
) -> ThresholdReplay {
    choice.check();
    let mut state = ElementState::default();
    carry_on_elements(
        &mut state,
        memory,
        schema,
        attribute,
        records,
        choice.counting,
    );
    element_layers(&state, choice).memory
}

/// The layers a replay of the similar record layer of an attribute that takes several values chose: the key layer's
/// replay left empty, the memory's chosen on the elements.
pub(crate) fn element_layers(state: &ElementState, choice: Choice) -> LayerThresholds {
    let memory = fit_recent(
        &state.answers,
        state.lookups,
        state.lookup_weight,
        state.order,
        choice.target_precision,
        choice.minimum_answered,
    );
    LayerThresholds {
        key: ThresholdReplay {
            chosen: None,
            most_precise: None,
            lookups: 0,
            candidates: 0,
            keys: None,
        },
        memory,
        batches: None,
        value_tails: None,
        calibration: None,
    }
}

/// Carries a replay of the similar record layer of an attribute that takes several values on through `records`, all
/// settled after those it has gone through, with the memory it left — the same answers, and the same memory, as replaying
/// them all from the start.
pub(crate) fn carry_on_elements(
    state: &mut ElementState,
    memory: &mut LexicalMemory,
    schema: &Schema,
    attribute: &str,
    records: &[Record],
    counting: Option<&[SettlementKind]>,
) {
    let definition = schema.attribute(attribute);
    assert!(
        definition.role == AttributeRole::Decided
            && definition.multiple
            && definition.similar_elements,
        "'{attribute}' is not answered by similar elements"
    );
    let task = format!("{}/{attribute}", schema.name());
    let votes = definition.similar_votes;
    let mut ordered: Vec<Record> = records.iter().map(Record::remembered).collect();
    ordered.sort_by(|a, b| {
        a.settled_at
            .cmp(&b.settled_at)
            .then_with(|| a.id.cmp(&b.id))
    });

    if let (Some(last), Some(first)) = (&state.last, ordered.first()) {
        assert!(
            (first.settled_at, first.id.as_str()) > (last.0, last.1.as_str()),
            "records carried on with must be settled after those replayed"
        );
    }
    for record in &ordered {
        state.order += 1;
        // A record settled again is not its own evidence.
        if let Some(own) = state.case_of.remove(&record.id) {
            memory.forget(&task, &record.id);
            state.cases.remove(&own);
            state.remembered -= 1;
        }

        let settled = similar_vote::elements_of(record.sets.get(attribute));
        let Some(joined) = similar_vote::join_elements(&settled) else {
            continue;
        };

        // Asked with what the attribute's last suggestion saw; remembered, below, with everything but its own.
        let before = arrived(schema, record, attribute, &record.values);
        let before_sets = arrived(schema, record, attribute, &record.sets);
        let weight = weight(record, attribute, counting);
        if state.remembered > 0 && weight > 0.0 {
            state.lookups += settled.len();
            state.lookup_weight += settled.len() as f64 * weight;
            let evidence = schema.evidence(attribute, &before, &before_sets);
            if !evidence.is_empty() {
                let found = memory.nearest(&task, &evidence, votes);
                for (element, share) in similar_vote::element_shares(&found, votes) {
                    if definition.admits_in(&element, &before, &before_sets) {
                        state.answers.push(Answer {
                            score: share,
                            correct: settled.binary_search(&element).is_ok(),
                            weight,
                            order: state.order,
                            key: None,
                            caught: false,
                        });
                    }
                }
            }
        }

        let mut own = record.sets.clone();
        own.remove(attribute);
        let evidence = schema.evidence(attribute, &record.values, &own);
        let case = text::normalize(&evidence);
        if let Some(earlier) = state.cases.get(&case) {
            memory.forget(&task, earlier);
            state.case_of.remove(earlier);
            state.remembered -= 1;
        }

        state.cases.insert(case.clone(), record.id.clone());
        state.case_of.insert(record.id.clone(), case);
        memory.remember(&task, &record.id, &evidence, &joined);
        state.remembered += 1;
    }

    if let Some(last) = ordered.last() {
        state.last = Some((last.settled_at, last.id.clone()));
    }
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

/// Experimental. One decision a judge outside Gil — a model, a rule, a person's score — made about a case a person later
/// settled: how sure the judge was of its best value, and whether that value was the one settled. A judge that gave no
/// candidate has `score` `None`; it still counts as a decision asked.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScoredDecision {
    /// Names the case; decisions settled at the same instant are taken in the scalar order of their ids.
    pub id: String,
    /// When the case was settled, in the same ticks as [`crate::schema::Record::settled_at`]; decisions are replayed in
    /// this order.
    pub settled_at: i64,
    /// How sure the judge was of its best value, on the judge's own scale, higher meaning surer; `None` when it gave no
    /// candidate.
    #[cfg_attr(feature = "serde", serde(default))]
    pub score: Option<f64>,
    /// Whether the judge's best value was the settled one. Not read when `score` is `None`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub right: bool,
    /// How many decisions this one counts as: 1, or one ÷ its draw probability for a case drawn for a check the judge
    /// did not see settled.
    #[cfg_attr(feature = "serde", serde(default = "one_decision"))]
    pub weight: f64,
}

#[cfg(feature = "serde")]
fn one_decision() -> f64 {
    1.0
}

impl ScoredDecision {
    /// A decision that counts once.
    pub fn new(id: impl Into<String>, settled_at: i64, score: Option<f64>, right: bool) -> Self {
        Self {
            id: id.into(),
            settled_at,
            score,
            right,
            weight: 1.0,
        }
    }

    /// The same decision counting as `weight` decisions.
    #[must_use]
    pub fn weighted(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
}

/// Experimental. Chooses the threshold for a judge outside Gil from nothing but its scores and what people settled: the
/// decisions are replayed in settlement order, and the threshold, its precision, answer rate and promise come out as
/// [`select_layers`] gives them for a memory layer — the lowest score down to which every band of answers met
/// `choice.target_precision`, resting on at least `choice.minimum_answered` independent answers, the promise being that of
/// the latest [`RECENT_RECORDS`] decisions at one standard error. A decision with no score counts as asked and not
/// answered. `choice.counting` is not read: the caller passes the decisions that count. The judge's answers are admitted
/// as trusted ([`crate::decider::ExternalCandidate::trusted`]) at or above the chosen threshold, and the promise is the
/// one to show beside them.
///
/// The score is the judge's own: nothing here assumes it is a probability, only that a higher one is more often right.
/// Choosing a threshold for a judge is not choosing how often to check it or where to send the cases it will not answer.
///
/// # Panics
/// When `choice.target_precision` is outside (0, 1], `choice.minimum_answered` is 0, a score is not finite, or a weight
/// is not positive and finite.
pub fn select_score_threshold(decisions: &[ScoredDecision], choice: Choice) -> ThresholdReplay {
    choice.check();
    for decision in decisions {
        assert!(
            decision.score.is_none_or(f64::is_finite),
            "a score must be finite"
        );
        assert!(
            decision.weight > 0.0 && decision.weight.is_finite(),
            "a weight must be positive and finite"
        );
    }

    let mut ordered: Vec<&ScoredDecision> = decisions.iter().collect();
    ordered.sort_by(|a, b| {
        a.settled_at
            .cmp(&b.settled_at)
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut answers: Vec<Answer> = Vec::new();
    let mut lookup_weight = 0.0_f64;
    for (index, decision) in ordered.iter().enumerate() {
        lookup_weight += decision.weight;
        if let Some(score) = decision.score {
            answers.push(Answer {
                score,
                correct: decision.right,
                weight: decision.weight,
                order: index + 1,
                key: None,
                caught: false,
            });
        }
    }

    fit_recent(
        &answers,
        ordered.len(),
        lookup_weight,
        ordered.len(),
        choice.target_precision,
        choice.minimum_answered,
    )
}

#[cfg(test)]
mod tests {
    use super::{Choice, ReplayState, Replaying, ThresholdRule, carry_on, layers};
    use super::{fnv1a, intraclass};
    use crate::decision_record::DecisionRecord;
    use crate::key_memory::KeyMemory;
    use crate::lexical_memory::LexicalMemory;
    use crate::schema::{Attribute, AttributeRole, Record, Schema};
    use std::collections::BTreeMap;

    /// Records alike in their products and apart in their summaries, some settled again later under the same id.
    fn settled(n: usize) -> Vec<Record> {
        let teams = ["network", "billing", "hardware", "auth"];
        let words = [
            "vpn drops",
            "invoice twice",
            "screen broken",
            "password reset",
            "slow link",
            "refund",
        ];
        (0..n)
            .map(|i| {
                let id = if i % 9 == 8 {
                    format!("t{}", i / 3)
                } else {
                    format!("t{i}")
                };
                let team = teams[(i * 7 + i / 5) % teams.len()];
                Record {
                    id,
                    values: BTreeMap::from([
                        ("product".into(), format!("p{}", i % 5)),
                        (
                            "summary".into(),
                            format!("{} {}", words[(i * 3) % words.len()], i % 4),
                        ),
                        ("team".into(), team.into()),
                    ]),
                    settled_at: 1_000 + i as i64,
                    ..Record::default()
                }
            })
            .collect()
    }

    #[test]
    fn a_replay_carried_on_is_the_replay_from_the_start() {
        let schema = Schema::new(
            "ticket",
            vec![
                Attribute::new("product", AttributeRole::Observed),
                Attribute::new("summary", AttributeRole::Observed),
                Attribute::new("team", AttributeRole::Decided),
            ],
        )
        .expect("a valid schema");
        let records = settled(600);
        let choice = Choice {
            target_precision: 0.8,
            minimum_answered: 10,
            counting: None,
        };
        let definition = schema.attribute("team");

        let run = |parts: &[&[Record]]| {
            let (mut keys, mut memory, mut state) = (
                KeyMemory::default(),
                LexicalMemory::default(),
                ReplayState::default(),
            );
            let mut lines: Vec<String> = Vec::new();
            for part in parts {
                let mut sink = |r: &DecisionRecord| lines.push(format!("{r:?}"));
                carry_on(
                    &mut state,
                    &mut keys,
                    &mut memory,
                    &schema,
                    "team",
                    part,
                    None,
                    Replaying::Values(&mut sink),
                );
            }
            let chosen = layers(
                &state.steps,
                state.order,
                definition,
                choice,
                ThresholdRule::EveryBand,
            );
            (format!("{chosen:?}"), lines)
        };
        let whole = run(&[&records]);
        for at in [1, 37, 300, 599] {
            assert_eq!(
                run(&[&records[..at], &records[at..]]),
                whole,
                "split at {at}"
            );
        }
        assert_eq!(
            run(&[&records[..100], &records[100..400], &records[400..]]),
            whole
        );
    }

    #[test]
    #[should_panic(expected = "settled after those replayed")]
    fn a_replay_is_not_carried_on_with_earlier_records() {
        let schema = Schema::new(
            "ticket",
            vec![
                Attribute::new("summary", AttributeRole::Observed),
                Attribute::new("team", AttributeRole::Decided),
            ],
        )
        .expect("a valid schema");
        let records = settled(20);
        let (mut keys, mut memory, mut state) = (
            KeyMemory::default(),
            LexicalMemory::default(),
            ReplayState::default(),
        );
        let mut ignore = |_: &DecisionRecord| {};
        carry_on(
            &mut state,
            &mut keys,
            &mut memory,
            &schema,
            "team",
            &records[10..],
            None,
            Replaying::Values(&mut ignore),
        );
        carry_on(
            &mut state,
            &mut keys,
            &mut memory,
            &schema,
            "team",
            &records[..10],
            None,
            Replaying::Values(&mut ignore),
        );
    }

    #[test]
    fn intraclass_matches_a_one_way_analysis_of_variance() {
        // Batches of (answers, matched): 1 1 1 0 · 0 0 1 · 1 1 · 0 1 0 0 0 — the same ICC(1) and n₀ as a one-way ANOVA on
        // the 0/1 values (the experiments' `batch_icc.icc1` gives 0.215697674… and 3.380952…).
        let (n0, icc) = intraclass([(4, 3), (3, 1), (2, 2), (5, 1)].into_iter());
        assert!((n0.unwrap() - 3.380_952_380_952_381).abs() < 1e-12);
        assert!((icc.unwrap() - 0.215_697_674_418_604_63).abs() < 1e-12);
        // One batch, or no answer beyond one per batch, says nothing.
        assert_eq!(intraclass([(5, 2)].into_iter()), (None, None));
        assert_eq!(intraclass([(1, 1), (1, 0)].into_iter()), (None, None));
        // Every answer alike: no variance at all, so no correlation, but a size.
        let (n0, icc) = intraclass([(3, 3), (3, 3)].into_iter());
        assert_eq!((n0, icc), (Some(3.0), None));
    }

    #[test]
    fn fnv1a_matches_the_published_vectors() {
        assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a("a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a("foobar"), 0x85944171f73967e8);
    }
}
