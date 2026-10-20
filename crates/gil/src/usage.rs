//! What the answers came to, counted without content: per attribute, how often an answer was offered, how people
//! settled it, and how the precision the answers realized compares with what was promised.
//!
//! A host keeps, for each decision it asked about, the trusted answer it showed (if any), how the attribute was settled
//! and the promise the answer was made under. [`usage`] turns those into counts — no values, no text — that a host can
//! send on. The counting follows the settlement kinds' meaning: an accepted answer was right and a corrected one wrong; a
//! check — settled again by someone who did not see the answer, drawn with a known probability — was right when it agrees
//! with the answer, and counts as one ÷ that probability decisions; a fill (settled by the host without a person) and a
//! reject (turned down, no value) say nothing about whether the answer was right and are left out of the precision, as
//! is a value settled over a suggestion that was not this decider's (kept or changed — counted on their own, answered or
//! not, so a host sees how its other suggestions fare in the same form).

use std::collections::BTreeMap;

use crate::schema::SettlementKind;

/// One decision as a host saw it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision<'a> {
    /// The decided attribute.
    pub attribute: &'a str,
    /// The trusted answer shown; `None` when the answer abstained (guesses shown or not).
    pub answer: Option<&'a str>,
    /// The value it was settled as; `None` when it was not (yet), or rejected.
    pub settled: Option<&'a str>,
    /// How it was settled; `None` when it was not (yet).
    pub settlement: Option<SettlementKind>,
    /// For a check, the probability it was drawn with; ignored otherwise.
    pub probability: f64,
    /// The promise the answer was made under; `None` when none was known or nothing was answered.
    pub promise: Option<f64>,
}

/// One attribute's counts.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AttributeUsage {
    /// Decisions asked about.
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
}

/// The counts of `decisions`, by attribute.
pub fn usage<'a>(
    decisions: impl IntoIterator<Item = Decision<'a>>,
) -> BTreeMap<String, AttributeUsage> {
    let mut out: BTreeMap<String, AttributeUsage> = BTreeMap::new();
    for d in decisions {
        let u = out.entry(d.attribute.to_owned()).or_default();
        u.decisions += 1;
        match d.settlement {
            Some(SettlementKind::AcceptOther) => u.accepted_other += 1,
            Some(SettlementKind::CorrectOther) => u.corrected_other += 1,
            _ => {}
        }
        let Some(answer) = d.answer else {
            if d.settlement == Some(SettlementKind::Enter) {
                u.entered += 1;
            }
            continue;
        };

        u.answered += 1;
        if let Some(p) = d.promise {
            u.promised += 1;
            u.promise_sum += p;
        }

        match d.settlement {
            Some(SettlementKind::Accept) => {
                u.accepted += 1;
                u.right_weight += 1.0;
                u.judged_weight += 1.0;
            }
            Some(SettlementKind::Correct) => {
                u.corrected += 1;
                u.judged_weight += 1.0;
            }
            Some(SettlementKind::Check) if d.probability > 0.0 => {
                let weight = 1.0 / d.probability;
                u.checked += 1;
                u.judged_weight += weight;
                if d.settled == Some(answer) {
                    u.checked_right += 1;
                    u.right_weight += weight;
                }
            }
            Some(SettlementKind::Reject) => u.rejected += 1,
            Some(SettlementKind::Fill) => u.filled += 1,
            Some(SettlementKind::Enter) => u.entered_over_answer += 1,
            _ => {}
        }
    }

    out
}
