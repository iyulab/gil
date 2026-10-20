//! The estimate. The cases with every weight 1 are those of the reference implementation's tests, compared to the
//! digits those tests compare (its generator differs, so bootstrap cases check properties, not numbers); the weighted
//! cases are what this crate adds.

use gil_review::{
    BOOTSTRAP_DRAWS, ESTIMATE_NO_VARIATION_IN_STRATUM, ESTIMATE_STRATUM_IS_CENSUS,
    ESTIMATE_STRATUM_TOO_SMALL, ESTIMATE_WEIGHTS_DEGENERATE, NORMAL_QUANTILE_95, ReviewError,
    ScoredItem, Stratum, estimate, estimate_bounds, unobserved_variation_bound,
};

const SEED: u64 = 20_260_912;

/// Equal to `digits` decimals, as the reference tests compare (ties to even).
fn close(expected: f64, actual: f64, digits: i32) -> bool {
    let scale = 10f64.powi(digits);
    (expected * scale).round_ties_even() == (actual * scale).round_ties_even()
}

fn items(per_stratum: &[&[f64]]) -> Vec<ScoredItem> {
    per_stratum
        .iter()
        .enumerate()
        .flat_map(|(h, scores)| {
            scores.iter().map(move |&score| ScoredItem {
                stratum: h as u32,
                score,
                weight: 1.0,
            })
        })
        .collect()
}

fn strata<'a>(spec: &[(&'a str, u64)]) -> Vec<Stratum<'a>> {
    spec.iter()
        .map(|&(name, population)| Stratum { name, population })
        .collect()
}

#[test]
fn the_estimate_its_standard_error_and_its_normal_interval_match_the_formula() {
    let s = strata(&[("A", 100), ("B", 100)]);
    let e = estimate(
        &s,
        &items(&[&[1.0, 0.5], &[0.8, 0.6]]),
        BOOTSTRAP_DRAWS,
        SEED,
    )
    .unwrap();
    assert!(close(0.725, e.point, 12));
    assert!(close(0.017_762_5, e.se * e.se, 12));
    assert!(close(0.133_276_029_352_618_4, e.se, 12));
    assert!(close(0.463_778_982_468_867_9, e.normal.low, 9));
    assert!(close(0.986_221_017_531_132, e.normal.high, 9));
    assert_eq!(e.sample_size, 4);
    assert_eq!(e.population, 200);
    assert_eq!(e.effective_n, 4.0);
    assert_eq!(e.caveats, 0);
    assert!(e.is_reliable());
}

#[test]
fn the_same_sample_and_seed_give_the_same_bootstrap_interval() {
    let s = strata(&[("A", 100), ("B", 300)]);
    let i = items(&[&[1.0, 0.5, 0.9, 0.4], &[0.8, 0.6, 1.0, 0.7]]);
    let first = estimate(&s, &i, BOOTSTRAP_DRAWS, SEED).unwrap();
    let second = estimate(&s, &i, BOOTSTRAP_DRAWS, SEED).unwrap();
    let other = estimate(&s, &i, BOOTSTRAP_DRAWS, SEED + 1).unwrap();
    assert_eq!(first.bootstrap, second.bootstrap);
    assert_eq!(first.seed, SEED);
    assert_ne!(first.bootstrap, other.bootstrap);
}

#[test]
fn strata_are_weighted_by_the_population_they_stand_for() {
    let s = strata(&[("small", 100), ("large", 300)]);
    let e = estimate(
        &s,
        &items(&[&[0.0, 0.0], &[1.0, 1.0]]),
        BOOTSTRAP_DRAWS,
        SEED,
    )
    .unwrap();
    assert!(close(0.75, e.point, 12));
}

#[test]
fn a_stratum_whose_scores_never_vary_keeps_the_uncertainty_its_sample_allows() {
    let s = strata(&[("all-correct", 500)]);
    let e = estimate(&s, &items(&[&[1.0; 4]]), BOOTSTRAP_DRAWS, SEED).unwrap();
    let p = 1.0 - 0.025f64.powf(0.25);
    let expected_se = ((1.0 - 4.0 / 500.0) * p * (1.0 - p) / 4.0).sqrt();
    assert!(close(1.0, e.point, 12));
    assert!(close(expected_se, e.se, 12));
    assert!(close(1.0 - 1.96 * expected_se, e.normal.low, 12));
    assert!(e.normal.high > 1.0, "not clamped");
    assert!(close(1.0, e.bootstrap.low, 12) && close(1.0, e.bootstrap.high, 12));
    assert_eq!(e.caveats, ESTIMATE_NO_VARIATION_IN_STRATUM);
    assert!(!e.is_reliable());
}

#[test]
fn the_unobserved_variation_bound_is_the_bound_on_a_differing_fraction() {
    for (n, fraction) in [(1, 0.975), (30, 0.115_70), (300, 0.012_22)] {
        assert!(
            close(
                fraction * (1.0 - fraction),
                unobserved_variation_bound(n),
                4
            ),
            "{n}"
        );
    }
}

#[test]
fn a_census_stratum_contributes_no_variance_even_when_its_scores_never_vary() {
    let s = strata(&[("census", 4)]);
    let e = estimate(&s, &items(&[&[1.0; 4]]), BOOTSTRAP_DRAWS, SEED).unwrap();
    assert!(close(0.0, e.se, 12));
    assert_ne!(e.caveats & ESTIMATE_STRATUM_IS_CENSUS, 0);
}

#[test]
fn a_stratum_reviewed_in_full_contributes_no_sampling_variance_and_says_so() {
    let s = strata(&[("census", 3)]);
    let e = estimate(&s, &items(&[&[1.0, 0.5, 0.0]]), BOOTSTRAP_DRAWS, SEED).unwrap();
    assert!(close(0.0, e.se, 12));
    assert!(close(0.5, e.bootstrap.low, 12) && close(0.5, e.bootstrap.high, 12));
    assert_ne!(e.caveats & ESTIMATE_STRATUM_IS_CENSUS, 0);
}

#[test]
fn the_bootstrap_interval_tracks_the_finite_population_standard_error() {
    let scores: Vec<f64> = (0..40)
        .map(|i| if i % 4 == 0 { 0.5 } else { 1.0 })
        .collect();
    let s = strata(&[("half-reviewed", 80)]);
    let e = estimate(&s, &items(&[&scores]), BOOTSTRAP_DRAWS, SEED).unwrap();
    let normal = NORMAL_QUANTILE_95 * e.se;
    let boot = (e.bootstrap.high - e.bootstrap.low) / 2.0;
    assert!(
        boot > 0.7 * normal && boot < 1.25 * normal,
        "{boot} vs {normal}"
    );
}

#[test]
fn a_stratum_reviewed_once_is_flagged_as_too_small_to_vary() {
    let s = strata(&[("thin", 100), ("ordinary", 100)]);
    let e = estimate(&s, &items(&[&[0.5], &[0.8, 0.6]]), BOOTSTRAP_DRAWS, SEED).unwrap();
    assert_ne!(e.caveats & ESTIMATE_STRATUM_TOO_SMALL, 0);
    assert!(!e.is_reliable());
}

#[test]
fn a_stratum_with_a_population_but_no_scored_item_is_refused_by_name() {
    let s = strata(&[("reviewed", 100), ("skipped", 400)]);
    let error = estimate(&s, &items(&[&[0.5, 0.9]]), BOOTSTRAP_DRAWS, SEED).unwrap_err();
    assert!(error.to_string().contains("skipped"), "{error}");
}

#[test]
fn reviewing_more_records_than_a_stratum_holds_is_refused_by_name() {
    let s = strata(&[("impossible", 2)]);
    let error = estimate(&s, &items(&[&[1.0; 3]]), BOOTSTRAP_DRAWS, SEED).unwrap_err();
    assert!(error.to_string().contains("impossible"), "{error}");
}

#[test]
fn an_empty_sample_is_refused() {
    assert_eq!(
        estimate(&[], &[], BOOTSTRAP_DRAWS, SEED),
        Err(ReviewError::NoStrata)
    );
}

#[test]
fn a_normal_interval_past_the_score_range_is_left_visible() {
    let s = strata(&[("near-ceiling", 1000)]);
    let e = estimate(&s, &items(&[&[1.0, 1.0, 1.0, 0.5]]), BOOTSTRAP_DRAWS, SEED).unwrap();
    assert!(e.normal.high > 1.0, "{}", e.normal.high);
    assert!(e.bootstrap.high <= 1.0, "{}", e.bootstrap.high);
}

// ---- what the weights add ----

fn weighted(scores: &[(f64, f64)]) -> Vec<ScoredItem> {
    scores
        .iter()
        .map(|&(score, weight)| ScoredItem {
            stratum: 0,
            score,
            weight,
        })
        .collect()
}

#[test]
fn equal_weights_of_any_size_give_the_unweighted_estimate() {
    let s = strata(&[("A", 100)]);
    let plain = estimate(&s, &items(&[&[1.0, 0.5, 0.9, 0.4]]), 200, SEED).unwrap();
    let doubled = estimate(
        &s,
        &weighted(&[(1.0, 2.0), (0.5, 2.0), (0.9, 2.0), (0.4, 2.0)]),
        200,
        SEED,
    )
    .unwrap();
    assert!(close(plain.point, doubled.point, 15));
    assert!(close(plain.se, doubled.se, 15));
    assert!(close(plain.effective_n, doubled.effective_n, 12));
    assert_eq!(plain.bootstrap, doubled.bootstrap);
}

#[test]
fn a_weighted_mean_is_the_hajek_mean_and_uneven_weights_lower_the_effective_n() {
    let s = strata(&[("checked", 1000)]);
    // An item drawn with probability 0.1 stands for ten; three drawn with probability 1 stand for one each.
    let e = estimate(
        &s,
        &weighted(&[(0.0, 10.0), (1.0, 1.0), (1.0, 1.0), (1.0, 1.0)]),
        0,
        SEED,
    )
    .unwrap();
    assert!(close(3.0 / 13.0, e.point, 15));
    assert!(close(169.0 / 103.0, e.effective_n, 12));
    assert_ne!(e.caveats & ESTIMATE_WEIGHTS_DEGENERATE, 0);
    assert!(
        e.bootstrap.low.is_nan() && e.bootstrap.high.is_nan(),
        "no replicates"
    );
}

#[test]
fn items_outside_the_rules_are_refused() {
    let s = strata(&[("A", 10)]);
    for bad in [
        (1.5, 1.0),
        (-0.1, 1.0),
        (f64::NAN, 1.0),
        (0.5, 0.0),
        (0.5, f64::INFINITY),
    ] {
        assert_eq!(
            estimate(&s, &weighted(&[bad]), 0, SEED),
            Err(ReviewError::Item { index: 0 })
        );
    }
    let stray = [ScoredItem {
        stratum: 3,
        score: 0.5,
        weight: 1.0,
    }];
    assert_eq!(
        estimate(&s, &stray, 0, SEED),
        Err(ReviewError::UnknownStratum { stratum: 3 })
    );
}

#[test]
fn the_band_runs_each_bound_through_the_estimate() {
    let s = strata(&[("A", 100), ("B", 100)]);
    // One unsettled item in B: scored 0 at worst, 1 at best.
    let lower = items(&[&[1.0, 0.5], &[0.8, 0.0]]);
    let upper = items(&[&[1.0, 0.5], &[0.8, 1.0]]);
    let (low, high) = estimate_bounds(&s, &lower, &upper, 300, SEED).unwrap();
    assert_eq!(low, estimate(&s, &lower, 300, SEED).unwrap());
    assert_eq!(high, estimate(&s, &upper, 300, SEED).unwrap());
    assert!(low.point < high.point);
    let short = items(&[&[1.0, 0.5], &[0.8]]);
    assert_eq!(
        estimate_bounds(&s, &lower, &short, 300, SEED),
        Err(ReviewError::BoundsDiffer)
    );
}
