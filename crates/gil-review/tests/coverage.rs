//! The property the intervals exist for, on a population whose census is known: over seeds 1–100, a pilot's 95% normal
//! interval contains the population's mean about 95 times. Three strata of binary scores — one noisy, one that a pilot
//! usually sees without an error (so it takes the unobserved-variation bound), one middling.

use gil_review::{
    BOOTSTRAP_DRAWS, PILOT_RECORDS_PER_STRATUM, PopulationStratum, ScoredItem, Stratum, draw_pilot,
    estimate,
};

/// Stratum `h` of `size` records, of which a share `rate` (in hundredths) score 1, spread evenly.
fn stratum(h: usize, size: usize, rate: usize) -> Vec<(String, f64)> {
    (0..size)
        .map(|i| {
            let correct = (i * 37 + h * 11) % 100 < rate;
            (format!("s{h}-{i:05}"), if correct { 1.0 } else { 0.0 })
        })
        .collect()
}

#[test]
fn the_normal_interval_covers_the_census_about_as_often_as_it_says() {
    let population = [
        stratum(0, 400, 60),
        stratum(1, 1500, 98),
        stratum(2, 300, 85),
    ];
    let total: usize = population.iter().map(Vec::len).sum();
    let census = population.iter().flatten().map(|(_, s)| s).sum::<f64>() / total as f64;
    let ids: Vec<Vec<&str>> = population
        .iter()
        .map(|s| s.iter().map(|(id, _)| id.as_str()).collect())
        .collect();
    let names = ["noisy", "quiet", "middling"];
    let frame: Vec<PopulationStratum> = ids
        .iter()
        .zip(names)
        .map(|(record_ids, name)| PopulationStratum { name, record_ids })
        .collect();
    let strata: Vec<Stratum> = population
        .iter()
        .zip(names)
        .map(|(s, name)| Stratum {
            name,
            population: s.len() as u64,
        })
        .collect();

    let (mut normal, mut boot, mut quiet_bound) = (0, 0, 0);
    for seed in 1..=100u64 {
        let sample = draw_pilot(&frame, PILOT_RECORDS_PER_STRATUM, seed).unwrap();
        let items: Vec<ScoredItem> = sample
            .strata
            .iter()
            .enumerate()
            .flat_map(|(h, drawn)| {
                let scores = &population[h];
                drawn.selected.iter().map(move |id| ScoredItem {
                    stratum: h as u32,
                    score: scores.iter().find(|(r, _)| r == id).unwrap().1,
                    weight: 1.0,
                })
            })
            .collect();
        let e = estimate(&strata, &items, BOOTSTRAP_DRAWS, seed).unwrap();
        normal += usize::from(e.normal.low <= census && census <= e.normal.high);
        boot += usize::from(e.bootstrap.low <= census && census <= e.bootstrap.high);
        quiet_bound += usize::from(e.caveats & gil_review::ESTIMATE_NO_VARIATION_IN_STRATUM != 0);
    }
    println!(
        "census {census:.4}: normal {normal}/100, bootstrap {boot}/100, quiet stratum unvarying {quiet_bound}/100"
    );
    // 95% nominal; 90 is below the 1st percentile of Binomial(100, 0.95) only if coverage is really under nominal.
    assert!(
        normal >= 90,
        "normal interval covered the census {normal}/100"
    );
    assert!(
        quiet_bound > 30,
        "the quiet stratum should often show no error ({quiet_bound}/100)"
    );
}
