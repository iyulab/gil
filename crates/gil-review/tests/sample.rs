//! The draw and the allocation. The allocation cases are those of the reference implementation's tests, which use no
//! random numbers and so must give the same counts here.

use gil_review::{
    PopulationStratum, ReviewError, SAMPLE_STRATUM_EMPTY, SAMPLE_STRATUM_TAKEN_WHOLE,
    StratumSpread, allocate_neyman, draw_pilot, minimum_design_deviation,
};

fn spread(population: u64, deviation: f64) -> StratumSpread {
    StratumSpread {
        population,
        deviation,
    }
}

#[test]
fn allocation_follows_population_times_spread_and_sums_to_the_budget() {
    assert_eq!(
        allocate_neyman(&[spread(100, 0.5), spread(900, 0.3)], 100).unwrap(),
        [16, 84]
    );
}

#[test]
fn a_pilot_deviation_below_the_design_floor_is_planned_against_the_floor() {
    assert_eq!(
        allocate_neyman(&[spread(100, 0.5), spread(900, 0.1)], 100).unwrap(),
        [20, 80]
    );
}

#[test]
fn a_stratum_with_no_pilot_spread_keeps_a_share_and_a_capped_one_hands_its_surplus_on() {
    let a = allocate_neyman(&[spread(3, 0.0), spread(8, 0.0), spread(5, 0.14)], 12).unwrap();
    assert_eq!(a, [2, 6, 4]);
}

#[test]
fn a_capped_stratum_passes_its_surplus_on_until_the_budget_is_spent() {
    assert_eq!(
        allocate_neyman(&[spread(3, 0.9), spread(100, 0.3)], 50).unwrap(),
        [3, 47]
    );
}

#[test]
fn a_budget_larger_than_the_population_stops_at_the_population() {
    assert_eq!(
        allocate_neyman(&[spread(3, 0.5), spread(4, 0.5)], 50).unwrap(),
        [3, 4]
    );
}

#[test]
fn a_small_noisy_stratum_outranks_a_large_quiet_one() {
    let a = allocate_neyman(&[spread(100, 0.9), spread(300, 0.01)], 100).unwrap();
    assert!(a[0] > a[1], "{a:?}");
}

#[test]
fn no_stratum_gets_zero_records_or_more_than_it_holds() {
    let a = allocate_neyman(&[spread(5, 0.0), spread(10_000, 0.4)], 100).unwrap();
    assert!((1..=5).contains(&a[0]), "{a:?}");
}

#[test]
fn with_no_spread_anywhere_the_budget_splits_by_population() {
    assert_eq!(
        allocate_neyman(&[spread(100, 0.0), spread(300, 0.0)], 100).unwrap(),
        [25, 75]
    );
}

#[test]
fn allocation_refuses_what_it_cannot_share() {
    let three = [spread(100, 0.5); 3];
    assert_eq!(
        allocate_neyman(&three, 2),
        Err(ReviewError::BudgetTooSmall {
            budget: 2,
            strata: 3
        })
    );
    assert_eq!(allocate_neyman(&[], 10), Err(ReviewError::NoStrata));
    assert!(matches!(
        allocate_neyman(&[spread(0, 0.5)], 10),
        Err(ReviewError::EmptyPopulation { .. })
    ));
    assert_eq!(
        allocate_neyman(&[spread(10, f64::NAN)], 10),
        Err(ReviewError::Deviation { stratum: 0 })
    );
}

#[test]
fn the_design_floor_is_the_deviation_of_one_failure_in_twenty() {
    assert_eq!(minimum_design_deviation(), (0.05f64 * 0.95).sqrt());
}

fn ids(prefix: &str, n: usize) -> Vec<String> {
    (0..n).map(|i| format!("{prefix}{i:03}")).collect()
}

#[test]
fn a_pilot_takes_per_stratum_records_reproducibly_whatever_order_the_records_come_in() {
    let a = ids("a", 50);
    let b = ids("b", 10);
    let a_refs: Vec<&str> = a.iter().map(String::as_str).collect();
    let b_refs: Vec<&str> = b.iter().map(String::as_str).collect();
    let mut a_reversed = a_refs.clone();
    a_reversed.reverse();
    let strata = [
        PopulationStratum {
            name: "busy",
            record_ids: &a_refs,
        },
        PopulationStratum {
            name: "quiet",
            record_ids: &b_refs,
        },
    ];
    let first = draw_pilot(&strata, 30, 7).unwrap();
    let again = draw_pilot(
        &[
            PopulationStratum {
                name: "busy",
                record_ids: &a_reversed,
            },
            strata[1],
        ],
        30,
        7,
    )
    .unwrap();
    assert_eq!(first, again);
    assert_eq!(first.strata[0].selected.len(), 30);
    assert_eq!(first.strata[0].population, 50);
    let mut unique = first.strata[0].selected.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 30);
    assert_eq!(
        first.strata[1].selected, b,
        "a small stratum is taken whole, in order"
    );
    assert_eq!(first.caveats, SAMPLE_STRATUM_TAKEN_WHOLE);
    assert!(!first.is_reliable());
    assert_eq!(first.selected_count(), 40);
    assert_ne!(
        draw_pilot(&strata, 30, 8).unwrap().strata[0].selected,
        first.strata[0].selected
    );
}

#[test]
fn an_empty_stratum_is_left_out_and_flagged() {
    let a = ids("a", 5);
    let a_refs: Vec<&str> = a.iter().map(String::as_str).collect();
    let sample = draw_pilot(
        &[
            PopulationStratum {
                name: "none",
                record_ids: &[],
            },
            PopulationStratum {
                name: "some",
                record_ids: &a_refs,
            },
        ],
        2,
        1,
    )
    .unwrap();
    assert_eq!(sample.strata.len(), 1);
    assert_eq!(sample.strata[0].name, "some");
    assert_eq!(sample.caveats, SAMPLE_STRATUM_EMPTY);
}

#[test]
fn a_pilot_refuses_strata_that_do_not_partition_the_population() {
    let s = |name, record_ids| PopulationStratum { name, record_ids };
    assert_eq!(
        draw_pilot(&[s("a", &["x", "y"]), s("b", &["y"])], 1, 0),
        Err(ReviewError::RecordInTwoStrata { record: "y".into() })
    );
    assert!(matches!(
        draw_pilot(&[s("a", &["x"]), s("a", &["y"])], 1, 0),
        Err(ReviewError::StratumName { .. })
    ));
    assert!(matches!(
        draw_pilot(&[s("", &["x"])], 1, 0),
        Err(ReviewError::StratumName { .. })
    ));
    assert_eq!(
        draw_pilot(&[s("a", &["x"])], 0, 0),
        Err(ReviewError::PerStratumTooSmall)
    );
}
