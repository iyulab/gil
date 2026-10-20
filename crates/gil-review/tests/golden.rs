//! The language-neutral golden (`tests/fixtures/review/golden.jsonl`): fixed inputs through every computation, the
//! outputs as JSON lines with each double in its shortest round-trip form (NaN as null), so the same text is the same
//! bits. A binding reproduces these lines; CI on another target reproduces them here. Set GIL_WRITE_REVIEW_GOLDEN to
//! rewrite the file.

use std::fmt::Write as _;

use gil_review::{
    Estimate, PopulationStratum, ScoredItem, Stratum, StratumSpread, Verdict, agreement,
    allocate_neyman, draw_pilot, estimate, estimate_bounds, unobserved_variation_bound,
};

fn num(x: f64) -> String {
    if x.is_nan() {
        "null".into()
    } else {
        format!("{x:?}")
    }
}

fn strings(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|s| format!("\"{s}\"")).collect();
    format!("[{}]", quoted.join(","))
}

fn estimate_json(e: &Estimate) -> String {
    format!(
        "{{\"point\":{},\"se\":{},\"normal\":[{},{}],\"bootstrap\":[{},{}],\"sample_size\":{},\"population\":{},\"effective_n\":{},\"seed\":{},\"caveats\":{}}}",
        num(e.point),
        num(e.se),
        num(e.normal.low),
        num(e.normal.high),
        num(e.bootstrap.low),
        num(e.bootstrap.high),
        e.sample_size,
        e.population,
        num(e.effective_n),
        e.seed,
        e.caveats
    )
}

fn lines() -> Vec<String> {
    let mut out = Vec::new();

    // The draw: three strata, one empty, one smaller than the request, ids out of order.
    let busy: Vec<String> = (0..60).rev().map(|i| format!("b{i:02}")).collect();
    let busy: Vec<&str> = busy.iter().map(String::as_str).collect();
    let small = ["s3", "s1", "s2"];
    for seed in [0u64, 1, 7, 20_260_912] {
        let sample = draw_pilot(
            &[
                PopulationStratum {
                    name: "busy",
                    record_ids: &busy,
                },
                PopulationStratum {
                    name: "empty",
                    record_ids: &[],
                },
                PopulationStratum {
                    name: "small",
                    record_ids: &small,
                },
            ],
            5,
            seed,
        )
        .unwrap();
        let mut line = format!("{{\"kind\":\"draw\",\"seed\":{seed},\"strata\":[");
        for (i, s) in sample.strata.iter().enumerate() {
            if i > 0 {
                line.push(',');
            }
            write!(
                line,
                "{{\"name\":\"{}\",\"population\":{},\"selected\":{}}}",
                s.name,
                s.population,
                strings(&s.selected)
            )
            .unwrap();
        }
        write!(line, "],\"caveats\":{}}}", sample.caveats).unwrap();
        out.push(line);
    }

    // The allocation.
    let cases: [(&[(u64, f64)], u64); 4] = [
        (&[(100, 0.5), (900, 0.3)], 100),
        (&[(3, 0.0), (8, 0.0), (5, 0.14)], 12),
        (&[(3, 0.9), (100, 0.3)], 50),
        (&[(7, 0.31), (70, 0.05), (700, 0.2), (7_000, 0.0)], 333),
    ];
    for (spec, budget) in cases {
        let strata: Vec<StratumSpread> = spec
            .iter()
            .map(|&(population, deviation)| StratumSpread {
                population,
                deviation,
            })
            .collect();
        let a = allocate_neyman(&strata, budget).unwrap();
        let counts: Vec<String> = a.iter().map(u64::to_string).collect();
        out.push(format!(
            "{{\"kind\":\"allocation\",\"budget\":{budget},\"counts\":[{}]}}",
            counts.join(",")
        ));
    }

    // The bound.
    for n in [1u64, 2, 3, 15, 30, 300, 4_096, 1_000_000] {
        out.push(format!(
            "{{\"kind\":\"bound\",\"n\":{n},\"variance\":{}}}",
            num(unobserved_variation_bound(n))
        ));
    }

    // The estimate: unweighted, weighted, an unvarying and a census stratum, and a band.
    let strata = [
        Stratum {
            name: "A",
            population: 400,
        },
        Stratum {
            name: "B",
            population: 1_500,
        },
        Stratum {
            name: "C",
            population: 6,
        },
    ];
    let item = |stratum, score, weight| ScoredItem {
        stratum,
        score,
        weight,
    };
    let plain: Vec<ScoredItem> = [0.0, 1.0, 1.0, 0.5, 1.0, 0.25, 1.0, 0.0]
        .iter()
        .map(|&s| item(0, s, 1.0))
        .chain((0..9).map(|_| item(1, 1.0, 1.0)))
        .chain(
            [1.0, 0.0, 1.0, 1.0, 0.0, 1.0]
                .iter()
                .map(|&s| item(2, s, 1.0)),
        )
        .collect();
    let weighted: Vec<ScoredItem> = plain
        .iter()
        .enumerate()
        .map(|(i, it)| ScoredItem {
            weight: 1.0 + (i % 4) as f64 * 2.5,
            ..*it
        })
        .collect();
    for (name, items) in [("plain", &plain), ("weighted", &weighted)] {
        for (replicates, seed) in [(0u32, 0u64), (2_000, 7)] {
            let e = estimate(&strata, items, replicates, seed).unwrap();
            out.push(format!(
                "{{\"kind\":\"estimate\",\"items\":\"{name}\",\"replicates\":{replicates},\"estimate\":{}}}",
                estimate_json(&e)
            ));
        }
    }
    let mut upper = plain.clone();
    upper[0].score = 1.0;
    let (low, high) = estimate_bounds(&strata, &plain, &upper, 500, 3).unwrap();
    out.push(format!(
        "{{\"kind\":\"bounds\",\"lower\":{},\"upper\":{}}}",
        estimate_json(&low),
        estimate_json(&high)
    ));

    // Agreement over three outcomes.
    let pattern = [
        (0u8, 0u8),
        (0, 0),
        (1, 1),
        (0, 1),
        (2, 2),
        (1, 0),
        (0, 0),
        (2, 0),
        (1, 1),
        (0, 0),
    ];
    let ids: Vec<String> = (0..pattern.len()).map(|i| format!("i{i}")).collect();
    let verdicts: Vec<Verdict> = pattern
        .iter()
        .zip(&ids)
        .flat_map(|(&(a, b), item)| {
            [
                Verdict {
                    item,
                    candidate: "c",
                    outcome: b,
                    reviewer: "zed",
                },
                Verdict {
                    item,
                    candidate: "c",
                    outcome: a,
                    reviewer: "amy",
                },
            ]
        })
        .collect();
    let a = agreement(&verdicts, 3).unwrap();
    let specific: Vec<String> = a.specific.iter().map(|&x| num(x)).collect();
    out.push(format!(
        "{{\"kind\":\"agreement\",\"compared_pairs\":{},\"observed\":{},\"kappa\":{},\"ac1\":{},\"specific\":[{}],\"prevalence_max\":{}}}",
        a.compared_pairs,
        num(a.observed),
        num(a.kappa),
        num(a.ac1),
        specific.join(","),
        num(a.prevalence_max)
    ));
    out
}

#[test]
fn the_review_statistics_reproduce_their_golden() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/review/golden.jsonl"
    );
    let got = lines();
    if std::env::var_os("GIL_WRITE_REVIEW_GOLDEN").is_some() {
        std::fs::write(path, got.join("\n") + "\n").unwrap();
        return;
    }
    let want = std::fs::read_to_string(path).expect("the golden is committed");
    let want: Vec<&str> = want.lines().collect();
    assert_eq!(got.len(), want.len());
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g, w, "golden line {}", i + 1);
    }
}
