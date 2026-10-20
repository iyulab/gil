//! What the reviewed scores say about the whole population: a stratified weighted estimate with a normal and a bootstrap
//! interval, and the band for items the record could not settle.

use std::collections::HashSet;

use crate::ReviewError;
use crate::rng::Xoshiro256StarStar;

/// A stratum of the population an estimate is weighted by, fixed before scoring: its name (for messages and reports
/// only) and how many records it holds.
#[derive(Clone, Copy, Debug)]
pub struct Stratum<'a> {
    pub name: &'a str,
    pub population: u64,
}

/// One reviewed item: the index of its stratum, its score in [0, 1] and its weight (above 0 — 1 for a plain sample, the
/// inverse of the probability it was drawn with for a weighted one).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScoredItem {
    pub stratum: u32,
    pub score: f64,
    pub weight: f64,
}

/// An interval, not clamped: a normal interval above 1 is the answer the arithmetic gives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub low: f64,
    pub high: f64,
}

/// A stratified estimate of the population's mean score.
#[derive(Clone, Debug, PartialEq)]
pub struct Estimate {
    pub point: f64,
    pub se: f64,
    /// The point ± 1.96 standard errors.
    pub normal: Interval,
    /// The 2.5th and 97.5th percentiles of the rescaled bootstrap; both NaN without replicates.
    pub bootstrap: Interval,
    /// Σ n_h.
    pub sample_size: u64,
    /// Σ N_h.
    pub population: u64,
    /// Σ_h (Σ w)² / Σ w² — the sample size when every weight is the same.
    pub effective_n: f64,
    pub seed: u64,
    /// `ESTIMATE_*` flags.
    pub caveats: u32,
}

impl Estimate {
    /// Whether every assumption the intervals rest on held.
    pub fn is_reliable(&self) -> bool {
        self.caveats == 0
    }
}

/// A stratum has fewer than two scored items: no sample variance can be read from it (it takes the bound of one
/// unvarying item) and the bootstrap keeps its mean.
pub const ESTIMATE_STRATUM_TOO_SMALL: u32 = 1 << 0;
/// A stratum was reviewed in full: it contributes no sampling variance.
pub const ESTIMATE_STRATUM_IS_CENSUS: u32 = 1 << 1;
/// A stratum's scores are all the same: its variance is the most the sample still allows, and the bootstrap collapses
/// there — read the normal interval.
pub const ESTIMATE_NO_VARIATION_IN_STRATUM: u32 = 1 << 2;
/// In some stratum the weights leave fewer than half the items' worth of information (effective n_h < n_h / 2).
pub const ESTIMATE_WEIGHTS_DEGENERATE: u32 = 1 << 3;

/// The bootstrap replicates an estimate is usually asked for.
pub const BOOTSTRAP_DRAWS: u32 = 2000;
/// The normal quantile of the 95% interval; the level is fixed.
pub const NORMAL_QUANTILE_95: f64 = 1.96;
/// Below this many scored items a stratum's sample variance is not read.
pub const MINIMUM_REVIEWED_PER_STRATUM: u32 = 2;
/// The Newton steps `unobserved_variation_bound` takes — enough for every n from 1 to 10⁶ to reach the cycle its
/// iteration ends in (measured; see the ignored test that measures it).
pub const NEWTON_STEPS: u32 = 16;

/// The variance given a stratum whose `n` scores are all the same: p (1 − p) with p = 1 − 0.025^(1/n), the largest
/// failure rate that still sees no failure in n draws one time in forty. `0.025^(1/n)` is the only transcendental
/// value here, and it is computed with plain arithmetic so every target gets the same bits: `NEWTON_STEPS` steps of
/// Newton's method on xⁿ = 0.025 from x = 1 (the power by repeated squaring), then the lower of the last step's value
/// and the next — in the last bits the iteration can alternate between two neighbours, and the lower one is taken.
///
/// # Panics
/// When `n` is 0.
pub fn unobserved_variation_bound(n: u64) -> f64 {
    assert!(n >= 1, "a stratum with scores has at least one");
    let root = fortieth_root(n);
    let p = 1.0 - root;
    p * (1.0 - p)
}

fn power(x: f64, e: u64) -> f64 {
    if e == 0 {
        return 1.0;
    }
    let half = power(x, e / 2);
    let square = half * half;
    if e % 2 == 1 { square * x } else { square }
}

fn newton(x: f64, n: u64) -> f64 {
    ((n - 1) as f64 * x + 0.025 / power(x, n - 1)) / n as f64
}

/// 0.025^(1/n) by the fixed Newton iteration of [`unobserved_variation_bound`].
pub(crate) fn fortieth_root(n: u64) -> f64 {
    let mut x = 1.0;
    for _ in 0..NEWTON_STEPS {
        x = newton(x, n);
    }
    x.min(newton(x, n))
}

struct Prepared<'a> {
    strata: &'a [Stratum<'a>],
    items: Vec<Vec<(f64, f64)>>,
    population: u64,
}

fn prepare<'a>(
    strata: &'a [Stratum<'a>],
    items: &[ScoredItem],
) -> Result<Prepared<'a>, ReviewError> {
    if strata.is_empty() {
        return Err(ReviewError::NoStrata);
    }
    let mut names = HashSet::new();
    for s in strata {
        if s.name.is_empty() || !names.insert(s.name) {
            return Err(ReviewError::StratumName {
                name: s.name.to_owned(),
            });
        }
        if s.population == 0 {
            return Err(ReviewError::EmptyPopulation {
                stratum: s.name.to_owned(),
            });
        }
    }
    let mut grouped = vec![Vec::new(); strata.len()];
    for (index, item) in items.iter().enumerate() {
        let Some(group) = grouped.get_mut(item.stratum as usize) else {
            return Err(ReviewError::UnknownStratum {
                stratum: item.stratum,
            });
        };
        if !(0.0..=1.0).contains(&item.score) || !item.weight.is_finite() || item.weight <= 0.0 {
            return Err(ReviewError::Item { index });
        }
        group.push((item.score, item.weight));
    }
    for (s, group) in strata.iter().zip(&grouped) {
        if group.is_empty() {
            return Err(ReviewError::UnsampledStratum {
                stratum: s.name.to_owned(),
            });
        }
        if group.len() as u64 > s.population {
            return Err(ReviewError::MoreItemsThanRecords {
                stratum: s.name.to_owned(),
            });
        }
    }
    let population = strata.iter().map(|s| s.population).sum();
    Ok(Prepared {
        strata,
        items: grouped,
        population,
    })
}

/// Σ w s / Σ w (Hájek).
fn weighted_mean(group: &[(f64, f64)]) -> f64 {
    let (mut sum, mut weights) = (0.0, 0.0);
    for &(s, w) in group {
        sum += w * s;
        weights += w;
    }
    sum / weights
}

fn is_constant(group: &[(f64, f64)]) -> bool {
    group.iter().all(|&(s, _)| s == group[0].0)
}

/// A stratified estimate of the mean score of the population `strata` describe, from the scored `items` (each naming
/// its stratum by index, in a fixed order), with a rescaled bootstrap of `replicates` draws seeded by `seed`.
///
/// Per stratum: the mean is the weighted (Hájek) mean, the variance the weighted sample variance — or, where every score
/// is the same, [`unobserved_variation_bound`] — and the standard error adds (N_h/N)² (1 − n_h/N_h) var_h / n_eff_h.
/// The bootstrap is Rao and Wu's: n_h − 1 items drawn with replacement in each stratum, the draw's mean pulled toward
/// the stratum mean by √(1 − n_h/N_h); a stratum with fewer than two items or reviewed in full keeps its mean. The
/// interval is the nearest-rank 2.5th and 97.5th percentiles of the sorted replicates. With every weight 1 this is the
/// arithmetic of the reference implementation.
///
/// # Errors
/// When no strata are given, a stratum has no name or shares one, a population is 0, an item names a stratum that is
/// not there or has a score outside [0, 1] or a weight not above 0, a stratum has no item, or a stratum has more items
/// than records.
pub fn estimate(
    strata: &[Stratum],
    items: &[ScoredItem],
    replicates: u32,
    seed: u64,
) -> Result<Estimate, ReviewError> {
    let p = prepare(strata, items)?;
    let mut caveats = 0;
    let (mut point, mut variance_sum, mut effective_n) = (0.0, 0.0, 0.0);
    let mut sample_size = 0u64;
    let mut means = Vec::with_capacity(strata.len());
    for (s, group) in p.strata.iter().zip(&p.items) {
        let n = group.len() as u64;
        sample_size += n;
        let weight = s.population as f64 / p.population as f64;
        let mean = weighted_mean(group);
        means.push(mean);
        point += weight * mean;

        let (sum_w, sum_w2) = group
            .iter()
            .fold((0.0, 0.0), |(a, b), &(_, w)| (a + w, b + w * w));
        let n_eff = sum_w * sum_w / sum_w2;
        effective_n += n_eff;
        if n < u64::from(MINIMUM_REVIEWED_PER_STRATUM) {
            caveats |= ESTIMATE_STRATUM_TOO_SMALL;
        }
        if n == s.population {
            caveats |= ESTIMATE_STRATUM_IS_CENSUS;
        }
        if n_eff < n as f64 / 2.0 {
            caveats |= ESTIMATE_WEIGHTS_DEGENERATE;
        }
        let constant = is_constant(group);
        if constant {
            caveats |= ESTIMATE_NO_VARIATION_IN_STRATUM;
        }

        let correction = 1.0 - n as f64 / s.population as f64;
        // A stratum of one item is constant, so it takes the bound (as the reference implementation does).
        let variance = if constant {
            unobserved_variation_bound(n)
        } else {
            let spread: f64 = group
                .iter()
                .map(|&(s, w)| {
                    let share = w / sum_w;
                    share * share * (s - mean) * (s - mean)
                })
                .sum();
            n as f64 / (n - 1) as f64 * spread * n_eff
        };
        variance_sum += weight * weight * correction * variance / n_eff;
    }

    let se = variance_sum.sqrt();
    Ok(Estimate {
        point,
        se,
        normal: Interval {
            low: point - NORMAL_QUANTILE_95 * se,
            high: point + NORMAL_QUANTILE_95 * se,
        },
        bootstrap: bootstrap(&p, &means, replicates, seed),
        sample_size,
        population: p.population,
        effective_n,
        seed,
        caveats,
    })
}

fn bootstrap(p: &Prepared, means: &[f64], replicates: u32, seed: u64) -> Interval {
    if replicates == 0 {
        return Interval {
            low: f64::NAN,
            high: f64::NAN,
        };
    }
    let rescale: Vec<f64> = p
        .strata
        .iter()
        .zip(&p.items)
        .map(|(s, group)| {
            if (group.len() as u32) < MINIMUM_REVIEWED_PER_STRATUM {
                0.0
            } else {
                (1.0 - group.len() as f64 / s.population as f64).sqrt()
            }
        })
        .collect();
    let mut random = Xoshiro256StarStar::new(seed);
    let mut draws = Vec::with_capacity(replicates as usize);
    for _ in 0..replicates {
        let mut resampled = 0.0;
        for (h, (s, group)) in p.strata.iter().zip(&p.items).enumerate() {
            let weight = s.population as f64 / p.population as f64;
            let m = group.len() - 1;
            if m < 1 || rescale[h] == 0.0 {
                resampled += weight * means[h];
                continue;
            }
            let (mut sum, mut weights) = (0.0, 0.0);
            for _ in 0..m {
                let (score, w) = group[random.below(group.len() as u64) as usize];
                sum += w * score;
                weights += w;
            }
            resampled += weight * (means[h] + rescale[h] * (sum / weights - means[h]));
        }
        draws.push(resampled);
    }
    draws.sort_by(f64::total_cmp);
    Interval {
        low: percentile(&draws, 0.025),
        high: percentile(&draws, 0.975),
    }
}

/// Nearest rank, round(q · (B − 1)) with ties to even.
fn percentile(sorted: &[f64], q: f64) -> f64 {
    let at = (q * (sorted.len() - 1) as f64).round_ties_even();
    sorted[(at.max(0.0) as usize).min(sorted.len() - 1)]
}

/// The band for items the record could not settle: the caller builds two item sets — those items scored at their worst
/// in `lower`, at their best in `upper`, every other item the same — and each goes through [`estimate`] on its own, so
/// each bound has its own standard error and bootstrap (not interval arithmetic).
///
/// # Errors
/// As [`estimate`], and when the two sets do not give each stratum the same number of items.
pub fn estimate_bounds(
    strata: &[Stratum],
    lower: &[ScoredItem],
    upper: &[ScoredItem],
    replicates: u32,
    seed: u64,
) -> Result<(Estimate, Estimate), ReviewError> {
    let counts = |items: &[ScoredItem]| {
        let mut c = vec![0usize; strata.len()];
        for item in items {
            if let Some(slot) = c.get_mut(item.stratum as usize) {
                *slot += 1;
            }
        }
        c
    };
    let low = estimate(strata, lower, replicates, seed)?;
    let high = estimate(strata, upper, replicates, seed)?;
    if counts(lower) != counts(upper) {
        return Err(ReviewError::BoundsDiffer);
    }
    Ok((low, high))
}

/// Per stratum, how many reviewed items were kept and how many left out as unsettled — for reporting the exclusion rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exclusion {
    pub stratum: u32,
    pub reviewed: u64,
    pub excluded: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_is_within_two_ulps_of_the_power_function() {
        for n in (1..=5_000u64).chain([10_000, 99_991, 1_000_000]) {
            let want = 0.025f64.powf(1.0 / n as f64);
            let got = fortieth_root(n);
            let ulps = (want.to_bits() as i64 - got.to_bits() as i64).abs();
            assert!(ulps <= 2, "n {n}: {want} vs {got} ({ulps} ulps)");
        }
    }

    #[test]
    fn the_root_is_the_lower_of_the_cycle_the_iteration_ends_in() {
        for n in 1..=2_000u64 {
            let x = fortieth_root(n);
            let next = newton(x, n);
            assert!(x <= next && (newton(next, n) == x || next == x), "n {n}");
        }
    }

    /// Measures how many Newton steps every n from 1 to 10⁶ needs to reach the cycle its iteration ends in, and checks
    /// `NEWTON_STEPS` covers it. Slow in a debug build: `cargo test --release -p iyulab-gil-review -- --ignored`.
    #[test]
    #[ignore]
    fn newton_steps_reach_the_final_cycle_for_every_n_to_a_million() {
        let mut worst = (0usize, 0u64);
        for n in 1..=1_000_000u64 {
            let mut xs = vec![1.0f64];
            for _ in 0..40 {
                let next = newton(*xs.last().unwrap(), n);
                xs.push(next);
            }
            let tail = &xs[xs.len() - 4..];
            let mut entry = xs.len() - 1;
            while entry > 0 && tail.contains(&xs[entry - 1]) {
                entry -= 1;
            }
            if entry > worst.0 {
                worst = (entry, n);
            }
        }
        println!("worst entry {worst:?}");
        assert!(worst.0 as u32 <= NEWTON_STEPS, "{worst:?}");
    }
}
