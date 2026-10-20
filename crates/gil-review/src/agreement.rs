//! How far two reviewers agree on the same items.

use std::collections::BTreeMap;

use crate::ReviewError;

/// One reviewer's verdict on one (item, candidate): the outcome's index among the categories, and who gave it.
#[derive(Clone, Copy, Debug)]
pub struct Verdict<'a> {
    pub item: &'a str,
    pub candidate: &'a str,
    pub outcome: u8,
    pub reviewer: &'a str,
}

/// Agreement between two reviewers over the pairs both judged.
#[derive(Clone, Debug, PartialEq)]
pub struct Agreement {
    pub compared_pairs: u64,
    /// The share of compared pairs on which the two gave the same outcome.
    pub observed: f64,
    /// Cohen's κ; NaN when both reviewers used one outcome only (chance agreement 1).
    pub kappa: f64,
    /// Gwet's AC1 (2008) — always defined.
    pub ac1: f64,
    /// Per outcome: of the times either reviewer gave it, how often both did; NaN for an outcome neither gave.
    pub specific: Vec<f64>,
    /// The largest share of all verdicts compared that one outcome took — reported, never used to judge.
    pub prevalence_max: f64,
}

/// Agreement between the two reviewers of every (item, candidate) judged exactly twice; a pair judged once is passed
/// over. The reviewer whose id comes first in ordinal (byte) order is the first reviewer — it matters only to κ's
/// marginals. Outcomes are indices below `categories`.
///
/// # Errors
/// When `categories` is below 2, an outcome is not below it, a pair has more than two verdicts or two from one
/// reviewer, or no pair was judged twice.
pub fn agreement(verdicts: &[Verdict], categories: u8) -> Result<Agreement, ReviewError> {
    if categories < 2 {
        return Err(ReviewError::Categories);
    }
    let mut by_pair: BTreeMap<(&str, &str), Vec<&Verdict>> = BTreeMap::new();
    for v in verdicts {
        if v.outcome >= categories {
            return Err(ReviewError::Outcome {
                item: v.item.to_owned(),
                candidate: v.candidate.to_owned(),
            });
        }
        by_pair.entry((v.item, v.candidate)).or_default().push(v);
    }

    let k = usize::from(categories);
    let mut table = vec![vec![0u64; k]; k];
    let mut compared = 0u64;
    for ((item, candidate), pair) in &by_pair {
        match pair.len() {
            1 => continue,
            2 => {}
            _ => {
                return Err(ReviewError::MoreThanTwoVerdicts {
                    item: (*item).to_owned(),
                    candidate: (*candidate).to_owned(),
                });
            }
        }
        let (mut first, mut second) = (pair[0], pair[1]);
        match first.reviewer.cmp(second.reviewer) {
            std::cmp::Ordering::Equal => {
                return Err(ReviewError::SameReviewerTwice {
                    item: (*item).to_owned(),
                    candidate: (*candidate).to_owned(),
                });
            }
            std::cmp::Ordering::Greater => std::mem::swap(&mut first, &mut second),
            std::cmp::Ordering::Less => {}
        }
        table[usize::from(first.outcome)][usize::from(second.outcome)] += 1;
        compared += 1;
    }
    if compared == 0 {
        return Err(ReviewError::NothingCompared);
    }

    let n = compared as f64;
    let mut agreed = 0.0;
    let mut first_marginal = vec![0.0; k];
    let mut second_marginal = vec![0.0; k];
    for i in 0..k {
        agreed += table[i][i] as f64;
        for j in 0..k {
            first_marginal[i] += table[i][j] as f64;
            second_marginal[j] += table[i][j] as f64;
        }
    }
    let observed = agreed / n;

    let mut kappa_chance = 0.0;
    for i in 0..k {
        kappa_chance += (first_marginal[i] / n) * (second_marginal[i] / n);
    }
    let kappa = if kappa_chance >= 1.0 {
        f64::NAN
    } else {
        (observed - kappa_chance) / (1.0 - kappa_chance)
    };

    let mut ac1_chance = 0.0;
    for i in 0..k {
        let pi = (first_marginal[i] + second_marginal[i]) / (2.0 * n);
        ac1_chance += pi * (1.0 - pi);
    }
    ac1_chance /= (k - 1) as f64;
    let ac1 = (observed - ac1_chance) / (1.0 - ac1_chance);

    let specific = (0..k)
        .map(|i| {
            let denominator = first_marginal[i] + second_marginal[i];
            if denominator == 0.0 {
                f64::NAN
            } else {
                2.0 * table[i][i] as f64 / denominator
            }
        })
        .collect();
    let prevalence_max = (0..k)
        .map(|i| (first_marginal[i] + second_marginal[i]) / (2.0 * n))
        .fold(0.0, f64::max);

    Ok(Agreement {
        compared_pairs: compared,
        observed,
        kappa,
        ac1,
        specific,
        prevalence_max,
    })
}
