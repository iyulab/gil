//! Which records get reviewed: a seeded pilot draw over strata the caller formed, and the Neyman allocation of the round
//! after it.

use std::collections::HashSet;

use crate::ReviewError;
use crate::rng::Xoshiro256StarStar;

/// A stratum is empty in this batch, so it is absent from the draw.
pub const SAMPLE_STRATUM_EMPTY: u32 = 1 << 0;
/// A stratum held no more records than were asked for, so all of it was taken — a census downstream.
pub const SAMPLE_STRATUM_TAKEN_WHOLE: u32 = 1 << 1;
/// The caller had nothing to stratify on (set by the caller; this crate never sets it).
pub const SAMPLE_NO_PARAMETERS: u32 = 1 << 2;

/// The pilot size per stratum: enough to get a standard deviation out of it, which the next round's allocation needs.
pub const PILOT_RECORDS_PER_STRATUM: u32 = 30;

/// One stratum of the population a review is drawn from: its name and every record in it.
#[derive(Clone, Copy, Debug)]
pub struct PopulationStratum<'a> {
    pub name: &'a str,
    pub record_ids: &'a [&'a str],
}

/// One stratum as drawn: its whole membership (the `N_h` an estimate is weighted by) and the records selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampledStratum {
    pub name: String,
    pub population: u64,
    pub selected: Vec<String>,
}

/// A stratified draw for review, reproducible from its seed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewSample {
    /// The strata drawn, in the order given; an empty stratum is left out (`SAMPLE_STRATUM_EMPTY`).
    pub strata: Vec<SampledStratum>,
    pub requested_per_stratum: u32,
    pub seed: u64,
    /// `SAMPLE_*` flags.
    pub caveats: u32,
}

impl ReviewSample {
    /// How many records a reviewer has to look at.
    pub fn selected_count(&self) -> usize {
        self.strata.iter().map(|s| s.selected.len()).sum()
    }

    /// Whether every stratum held records and each gave up as many as were asked for.
    pub fn is_reliable(&self) -> bool {
        self.caveats == 0
    }
}

/// A pilot draw: `per_stratum` records from every stratum, because before a pilot there is no variance to allocate
/// against. Within a stratum the ids are put in ordinal (byte) order and a partial Fisher–Yates shuffle takes the
/// sample, so the order a stratum lists its records in does not matter; the strata are drawn in the order given, which
/// with `seed` fixes the sample. A stratum with no more records than asked for is taken whole.
///
/// # Errors
/// When fewer than one record per stratum is asked for, a stratum has no name or shares one, or a record is in two
/// strata.
pub fn draw_pilot(
    strata: &[PopulationStratum],
    per_stratum: u32,
    seed: u64,
) -> Result<ReviewSample, ReviewError> {
    if per_stratum < 1 {
        return Err(ReviewError::PerStratumTooSmall);
    }

    let mut names = HashSet::new();
    let mut seen = HashSet::new();
    for stratum in strata {
        if stratum.name.is_empty() || !names.insert(stratum.name) {
            return Err(ReviewError::StratumName {
                name: stratum.name.to_owned(),
            });
        }
        for id in stratum.record_ids {
            if !seen.insert(*id) {
                return Err(ReviewError::RecordInTwoStrata {
                    record: (*id).to_owned(),
                });
            }
        }
    }

    let mut caveats = 0;
    let mut random = Xoshiro256StarStar::new(seed);
    let mut drawn = Vec::new();
    for stratum in strata {
        let mut members: Vec<&str> = stratum.record_ids.to_vec();
        members.sort_unstable();
        if members.is_empty() {
            caveats |= SAMPLE_STRATUM_EMPTY;
            continue;
        }

        let population = members.len() as u64;
        let selected = if population <= u64::from(per_stratum) {
            caveats |= SAMPLE_STRATUM_TAKEN_WHOLE;
            members
        } else {
            let count = per_stratum as usize;
            for i in 0..count {
                let pick = i + random.below((members.len() - i) as u64) as usize;
                members.swap(i, pick);
            }
            members.truncate(count);
            members
        };
        drawn.push(SampledStratum {
            name: stratum.name.to_owned(),
            population,
            selected: selected.into_iter().map(str::to_owned).collect(),
        });
    }

    Ok(ReviewSample {
        strata: drawn,
        requested_per_stratum: per_stratum,
        seed,
        caveats,
    })
}

/// One stratum's size and the per-record score deviation a pilot saw in it, for sizing the round after the pilot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StratumSpread {
    pub population: u64,
    pub deviation: f64,
}

/// The smallest per-stratum deviation the allocation plans against: that of a score failing one time in twenty,
/// `sqrt(0.05 · 0.95)`. A pilot that saw no error reports 0, which bounds the spread rather than measuring it.
pub fn minimum_design_deviation() -> f64 {
    (0.05f64 * 0.95).sqrt()
}

/// Neyman allocation for the round after a pilot: the budget shared in proportion to `N_h · max(s_h,
/// minimum_design_deviation())`. Every stratum gets at least one record and none more than it holds; what a capped
/// stratum cannot take is shared again among the strata still open, in the same proportions, until a round fixes no
/// stratum. Within a round the counts are floored and what is left goes by largest remainder (the lower index first
/// where remainders tie), so the counts sum to the budget unless the strata hold fewer records than that.
///
/// # Errors
/// When no strata are given, a stratum's population is 0, a deviation is negative or not finite, or the budget is
/// below the number of strata.
pub fn allocate_neyman(strata: &[StratumSpread], budget: u64) -> Result<Vec<u64>, ReviewError> {
    if strata.is_empty() {
        return Err(ReviewError::NoStrata);
    }
    for (i, stratum) in strata.iter().enumerate() {
        if stratum.population == 0 {
            return Err(ReviewError::EmptyPopulation {
                stratum: i.to_string(),
            });
        }
        if !stratum.deviation.is_finite() || stratum.deviation < 0.0 {
            return Err(ReviewError::Deviation { stratum: i });
        }
    }
    if budget < strata.len() as u64 {
        return Err(ReviewError::BudgetTooSmall {
            budget,
            strata: strata.len(),
        });
    }

    let floor = minimum_design_deviation();
    let weights: Vec<f64> = strata
        .iter()
        .map(|s| s.population as f64 * s.deviation.max(floor))
        .collect();
    let mut allocation = vec![0i64; strata.len()];
    let mut fixed = vec![false; strata.len()];
    let mut open: Vec<usize> = (0..strata.len()).collect();
    let budget = budget as i64;
    while !open.is_empty() {
        let remaining = budget
            - (0..strata.len())
                .filter(|&i| fixed[i])
                .map(|i| allocation[i])
                .sum::<i64>();
        let total: f64 = open.iter().map(|&i| weights[i]).sum();
        let mut remainders = Vec::with_capacity(open.len());
        let mut handed_out = 0i64;
        for &i in &open {
            let ideal = remaining.max(0) as f64 * weights[i] / total;
            allocation[i] = ideal.floor() as i64;
            handed_out += allocation[i];
            remainders.push((i, ideal - allocation[i] as f64));
        }
        remainders.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        for (i, _) in remainders {
            if handed_out >= remaining {
                break;
            }
            allocation[i] += 1;
            handed_out += 1;
        }

        let before = open.len();
        open.retain(|&i| {
            let capacity = strata[i].population as i64;
            if allocation[i] < 1 {
                allocation[i] = 1;
            } else if allocation[i] <= capacity {
                return true;
            } else {
                allocation[i] = capacity;
            }
            fixed[i] = true;
            false
        });
        if open.len() == before {
            break;
        }
    }

    Ok(allocation.into_iter().map(|a| a as u64).collect())
}
