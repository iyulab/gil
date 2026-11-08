//! What the answers came to, counted without content: per attribute, how often an answer was offered, how people
//! settled it, and how the precision the answers realized compares with what was promised.
//!
//! The settled records are the ledger: a host that keeps, on each record it saves, the suggestion in view, the layer it
//! came from and the promise it was made under ([`Record::suggested`], [`Record::sources`], [`Record::promises`]) has
//! what [`usage`] counts — no values, no text — and can send the counts on. The counting follows the settlement kinds'
//! meaning: an accepted answer was right and a corrected one wrong; a check — settled again by someone who did not see
//! the answer, drawn with a known probability — was right when it agrees with the answer, and counts as one ÷ that
//! probability decisions; a fill (settled by the host without a person) and a reject (turned down — the record keeps the
//! answer in view and no value) say nothing about whether the answer was right and are left out of the precision, as is
//! a value settled over a suggestion that was not this decider's (kept or changed — counted on their own, answered or
//! not, so a host sees how its other suggestions fare in the same form).
//!
//! Two further splits say what the counts alone cannot. A check that was a decision's **first** settlement — the answer
//! kept from view when the decision was first settled — is told from one that came **after** it, an audit of the settler
//! (the same id settled again, later): the first kind measures the answers with no pull from seeing them, the second the
//! people. And an answer shown with its promise drawn at random to be displayed or not is counted by that draw, so the
//! acceptance with the promise in view can be set against the acceptance without it. Which settlement was first is read
//! from the records given — a host that counts by period passes all the records of a decision whose first settlement
//! falls in that period, its later audits included.

use std::collections::{BTreeMap, BTreeSet};

use crate::schema::{Record, SettlementKind};

/// One attribute's counts.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AttributeUsage {
    /// Decisions settled, or turned down with an answer in view.
    pub decisions: usize,
    /// Of them, answered (a trusted answer shown).
    pub answered: usize,
    /// Answered and accepted as shown.
    pub accepted: usize,
    /// Answered and settled as another value.
    pub corrected: usize,
    /// Answered and turned down with no value settled.
    pub rejected: usize,
    /// Answered and settled by the host on its own.
    pub filled: usize,
    /// Answered and checked by someone who did not see the answer.
    pub checked: usize,
    /// Of the checks, those that agreed with the answer.
    pub checked_right: usize,
    /// Not answered, and entered by a person.
    pub entered: usize,
    /// Answered, yet settled as entered "with no answer in view" — a host that reports its own answers' acceptance as
    /// entry; such decisions would grade the answers on themselves. Left out of every other count but this one: it
    /// should stay 0.
    pub entered_over_answer: usize,
    /// Settled by a person who kept a suggestion that was not this decider's, shown in place of an answer — answered or
    /// not; left out of the precision.
    pub accepted_other: usize,
    /// Settled by a person as another value than a suggestion that was not this decider's, shown in place of an answer —
    /// answered or not; left out of the precision.
    pub corrected_other: usize,
    /// Answered with a promise: their number and the promises' sum.
    pub promised: usize,
    pub promise_sum: f64,
    /// The weighted counts the realized precision rests on: right and right-or-wrong, a check weighing one ÷ its
    /// probability.
    pub right_weight: f64,
    pub judged_weight: f64,
    /// Answered and first settled as a check: the answer kept from view when the decision was first settled. Counted
    /// once each, not weighted — the share right among them is the precision with no pull from seeing the answer.
    pub first_checks: usize,
    /// Of the first checks, those that agreed with the answer.
    pub first_checks_right: usize,
    /// First settled as a check with no answer kept on the record — such a decision cannot be told from one this
    /// decider had no answer for, so it is in no other count but this one; it should stay 0.
    pub first_checks_without_answer: usize,
    /// Checks that came after the decision's first settlement (the same id settled again, later): audits of the settler,
    /// counted here whatever else they keep.
    pub audits: usize,
    /// The answers accepted or corrected with their promise drawn at random to be displayed or not, one entry per draw
    /// probability, lowest first.
    pub promise_display: Vec<PromiseDisplay>,
}

/// How the answers shown under one promise-display draw were settled, with the promise displayed and without it.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PromiseDisplay {
    /// The probability the promise was drawn to be displayed with.
    pub probability: f64,
    /// Accepted with the promise displayed.
    pub shown_accepted: usize,
    /// Corrected with the promise displayed.
    pub shown_corrected: usize,
    /// Accepted with the promise not displayed.
    pub hidden_accepted: usize,
    /// Corrected with the promise not displayed.
    pub hidden_corrected: usize,
}

impl AttributeUsage {
    /// The decisions not answered.
    pub fn abstained(&self) -> usize {
        self.decisions - self.answered
    }

    /// The share of decisions answered; `None` with no decisions.
    pub fn answer_rate(&self) -> Option<f64> {
        (self.decisions > 0).then(|| self.answered as f64 / self.decisions as f64)
    }

    /// The precision the answers realized, among those whose rightness is known (accepted, corrected, checked —
    /// checks weighted); `None` when none is.
    pub fn realized(&self) -> Option<f64> {
        (self.judged_weight > 0.0).then(|| self.right_weight / self.judged_weight)
    }

    /// The mean promise of the answers made under one; `None` when none was.
    pub fn promise(&self) -> Option<f64> {
        (self.promised > 0).then(|| self.promise_sum / self.promised as f64)
    }

    /// The share right among the first checks; `None` when there are none.
    pub fn first_checks_realized(&self) -> Option<f64> {
        (self.first_checks > 0).then(|| self.first_checks_right as f64 / self.first_checks as f64)
    }

    fn display_at(&mut self, probability: f64) -> &mut PromiseDisplay {
        let at = match self
            .promise_display
            .binary_search_by(|d| d.probability.total_cmp(&probability))
        {
            Ok(at) => at,
            Err(at) => {
                self.promise_display.insert(
                    at,
                    PromiseDisplay {
                        probability,
                        ..PromiseDisplay::default()
                    },
                );
                at
            }
        };
        &mut self.promise_display[at]
    }
}

/// The counts of the decisions `records` settled, by attribute. A decided attribute counts once per record: settled,
/// or turned down with this decider's answer in view (a record that names no settlement for it, or `Reject`). It was answered when its record names the layer the answer came
/// from, or was settled as accepted or corrected; a check is judged against the suggestion the record keeps. A
/// decision's first settlement is the earliest `settled_at` among the records of its id that settle the attribute
/// (the earlier given, when two tie).
pub fn usage<'a>(
    records: impl IntoIterator<Item = &'a Record>,
) -> BTreeMap<String, AttributeUsage> {
    let records: Vec<&Record> = records.into_iter().collect();
    let mut first: BTreeMap<(&str, &str), (i64, usize)> = BTreeMap::new();
    for (at, record) in records.iter().enumerate() {
        for attribute in record.settlements.keys() {
            let key = (record.id.as_str(), attribute.as_str());
            let here = (record.settled_at, at);
            first
                .entry(key)
                .and_modify(|earliest| *earliest = (*earliest).min(here))
                .or_insert(here);
        }
    }

    let mut out: BTreeMap<String, AttributeUsage> = BTreeMap::new();
    for (at, record) in records.iter().enumerate() {
        let attributes: BTreeSet<&String> = record
            .settlements
            .keys()
            .chain(record.sources.keys())
            .collect();
        for attribute in attributes {
            let kind = record.settlements.get(attribute).copied();
            let u = out.entry(attribute.clone()).or_default();
            u.decisions += 1;
            if kind == Some(SettlementKind::Check) {
                let is_first = first[&(record.id.as_str(), attribute.as_str())].1 == at;
                if !is_first {
                    u.audits += 1;
                } else if !record.suggested.contains_key(attribute) {
                    u.first_checks_without_answer += 1;
                }
            }
            match kind {
                Some(SettlementKind::AcceptOther) => u.accepted_other += 1,
                Some(SettlementKind::CorrectOther) => u.corrected_other += 1,
                _ => {}
            }
            let answered = record.sources.contains_key(attribute)
                || matches!(kind, Some(SettlementKind::Accept | SettlementKind::Correct));
            if !answered {
                if kind == Some(SettlementKind::Enter) {
                    u.entered += 1;
                }
                continue;
            }

            u.answered += 1;
            if let Some(p) = record.promises.get(attribute) {
                u.promised += 1;
                u.promise_sum += p;
            }

            match kind {
                Some(SettlementKind::Accept) => {
                    u.accepted += 1;
                    u.right_weight += 1.0;
                    u.judged_weight += 1.0;
                    if let Some(display) = record.promise_shown.get(attribute) {
                        let d = u.display_at(display.probability);
                        if display.shown {
                            d.shown_accepted += 1;
                        } else {
                            d.hidden_accepted += 1;
                        }
                    }
                }
                Some(SettlementKind::Correct) => {
                    u.corrected += 1;
                    u.judged_weight += 1.0;
                    if let Some(display) = record.promise_shown.get(attribute) {
                        let d = u.display_at(display.probability);
                        if display.shown {
                            d.shown_corrected += 1;
                        } else {
                            d.hidden_corrected += 1;
                        }
                    }
                }
                Some(SettlementKind::Check) => {
                    let suggested = record.suggested.get(attribute);
                    let right = suggested.is_some() && record.values.get(attribute) == suggested;
                    if suggested.is_some()
                        && first[&(record.id.as_str(), attribute.as_str())].1 == at
                    {
                        u.first_checks += 1;
                        if right {
                            u.first_checks_right += 1;
                        }
                    }
                    if let Some(weight) = record.check_weight(attribute)
                        && suggested.is_some()
                    {
                        u.checked += 1;
                        u.judged_weight += weight;
                        if right {
                            u.checked_right += 1;
                            u.right_weight += weight;
                        }
                    }
                }
                None | Some(SettlementKind::Reject) => u.rejected += 1,
                Some(SettlementKind::Fill) => u.filled += 1,
                Some(SettlementKind::Enter) => u.entered_over_answer += 1,
                _ => {}
            }
        }
    }

    out
}
