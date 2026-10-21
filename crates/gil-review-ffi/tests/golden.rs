//! The C ABI reproduces the review golden (`tests/fixtures/review/golden.jsonl`) line for line — the same inputs as the
//! crate's own golden test, every value through the flat structures and caller buffers a binding uses. Also: a refusal
//! comes back as a status and a message, and a malformed call never reaches the computation.

use std::fmt::Write as _;
use std::ptr;

use gil_review_native::*;

fn s(text: &str) -> GilReviewStr {
    GilReviewStr {
        ptr: text.as_ptr(),
        len: text.len(),
    }
}

fn last_error() -> String {
    let len = unsafe { gil_review_last_error(ptr::null_mut(), 0) };
    let mut buffer = vec![0u8; len];
    let written = unsafe { gil_review_last_error(buffer.as_mut_ptr(), buffer.len()) };
    assert_eq!(written, len);
    String::from_utf8(buffer).unwrap()
}

fn ok(status: i32) {
    assert_eq!(status, GIL_REVIEW_OK, "{}", last_error());
    assert_eq!(last_error(), "");
}

fn num(x: f64) -> String {
    if x.is_nan() {
        "null".into()
    } else {
        format!("{x:?}")
    }
}

fn estimate_json(e: &GilReviewEstimate) -> String {
    format!(
        "{{\"point\":{},\"se\":{},\"normal\":[{},{}],\"bootstrap\":[{},{}],\"sample_size\":{},\"population\":{},\"effective_n\":{},\"seed\":{},\"caveats\":{}}}",
        num(e.point),
        num(e.se),
        num(e.normal_low),
        num(e.normal_high),
        num(e.bootstrap_low),
        num(e.bootstrap_high),
        e.sample_size,
        e.population,
        num(e.effective_n),
        e.seed,
        e.caveats
    )
}

const STRATUM_NAMES: [&str; 3] = ["A", "B", "C"];

fn estimate_strata() -> Vec<GilReviewStratum> {
    STRATUM_NAMES
        .iter()
        .zip([400u64, 1_500, 6])
        .map(|(name, population)| GilReviewStratum {
            name: s(name),
            population,
        })
        .collect()
}

fn plain_items() -> Vec<GilReviewScoredItem> {
    let item = |stratum, score| GilReviewScoredItem {
        stratum,
        score,
        weight: 1.0,
    };
    [0.0, 1.0, 1.0, 0.5, 1.0, 0.25, 1.0, 0.0]
        .iter()
        .map(|&x| item(0, x))
        .chain((0..9).map(|_| item(1, 1.0)))
        .chain([1.0, 0.0, 1.0, 1.0, 0.0, 1.0].iter().map(|&x| item(2, x)))
        .collect()
}

fn lines() -> Vec<String> {
    let mut out = Vec::new();

    // The draw: the busy stratum's ids out of order, an empty stratum, one smaller than the request.
    let busy: Vec<String> = (0..60).rev().map(|i| format!("b{i:02}")).collect();
    let mut ids: Vec<&str> = busy.iter().map(String::as_str).collect();
    ids.extend(["s3", "s1", "s2"]);
    let records: Vec<GilReviewStr> = ids.iter().map(|id| s(id)).collect();
    let names = ["busy", "empty", "small"];
    let strata = [
        GilReviewPopulationStratum {
            name: s(names[0]),
            first: 0,
            count: 60,
        },
        GilReviewPopulationStratum {
            name: s(names[1]),
            first: 60,
            count: 0,
        },
        GilReviewPopulationStratum {
            name: s(names[2]),
            first: 60,
            count: 3,
        },
    ];
    for seed in [0u64, 1, 7, 20_260_912] {
        let mut drawn = [GilReviewDrawnStratum::default(); 3];
        let mut selected = vec![u32::MAX; records.len()];
        let mut caveats = 0;
        ok(unsafe {
            gil_review_draw_pilot(
                strata.as_ptr(),
                strata.len(),
                records.as_ptr(),
                records.len(),
                5,
                seed,
                drawn.as_mut_ptr(),
                selected.as_mut_ptr(),
                selected.len(),
                &raw mut caveats,
            )
        });
        let mut line = format!("{{\"kind\":\"draw\",\"seed\":{seed},\"strata\":[");
        let mut at = 0;
        let mut first = true;
        for (stratum, d) in strata.iter().zip(&drawn) {
            if d.population == 0 {
                continue; // left out of the sample, as in the crate
            }
            if !first {
                line.push(',');
            }
            first = false;
            let chosen: Vec<String> = selected[at..at + d.selected as usize]
                .iter()
                .map(|&i| format!("\"{}\"", ids[stratum.first + i as usize]))
                .collect();
            at += d.selected as usize;
            let name = names[strata.iter().position(|x| ptr::eq(x, stratum)).unwrap()];
            write!(
                line,
                "{{\"name\":\"{name}\",\"population\":{},\"selected\":[{}]}}",
                d.population,
                chosen.join(",")
            )
            .unwrap();
        }
        write!(line, "],\"caveats\":{caveats}}}").unwrap();
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
        let spreads: Vec<GilReviewStratumSpread> = spec
            .iter()
            .map(|&(population, deviation)| GilReviewStratumSpread {
                population,
                deviation,
            })
            .collect();
        let mut counts = vec![0u64; spreads.len()];
        ok(unsafe {
            gil_review_allocate_neyman(spreads.as_ptr(), spreads.len(), budget, counts.as_mut_ptr())
        });
        let counts: Vec<String> = counts.iter().map(u64::to_string).collect();
        out.push(format!(
            "{{\"kind\":\"allocation\",\"budget\":{budget},\"counts\":[{}]}}",
            counts.join(",")
        ));
    }

    // The bound.
    for n in [1u64, 2, 3, 15, 30, 300, 4_096, 1_000_000] {
        out.push(format!(
            "{{\"kind\":\"bound\",\"n\":{n},\"variance\":{}}}",
            num(gil_review_unobserved_variation_bound(n))
        ));
    }

    // The estimate.
    let strata = estimate_strata();
    let plain = plain_items();
    let weighted: Vec<GilReviewScoredItem> = plain
        .iter()
        .enumerate()
        .map(|(i, it)| GilReviewScoredItem {
            weight: 1.0 + (i % 4) as f64 * 2.5,
            ..*it
        })
        .collect();
    for (name, items) in [("plain", &plain), ("weighted", &weighted)] {
        for (replicates, seed) in [(0u32, 0u64), (2_000, 7)] {
            let mut e = GilReviewEstimate::default();
            ok(unsafe {
                gil_review_estimate(
                    strata.as_ptr(),
                    strata.len(),
                    items.as_ptr(),
                    items.len(),
                    replicates,
                    seed,
                    &raw mut e,
                )
            });
            out.push(format!(
                "{{\"kind\":\"estimate\",\"items\":\"{name}\",\"replicates\":{replicates},\"estimate\":{}}}",
                estimate_json(&e)
            ));
        }
    }
    let mut upper = plain.clone();
    upper[0].score = 1.0;
    let (mut low, mut high) = (GilReviewEstimate::default(), GilReviewEstimate::default());
    ok(unsafe {
        gil_review_estimate_bounds(
            strata.as_ptr(),
            strata.len(),
            plain.as_ptr(),
            plain.len(),
            upper.as_ptr(),
            upper.len(),
            500,
            3,
            &raw mut low,
            &raw mut high,
        )
    });
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
    let items: Vec<String> = (0..pattern.len()).map(|i| format!("i{i}")).collect();
    let verdicts: Vec<GilReviewVerdict> = pattern
        .iter()
        .zip(&items)
        .flat_map(|(&(a, b), item)| {
            [(b, "zed"), (a, "amy")].map(|(outcome, reviewer)| GilReviewVerdict {
                item: s(item),
                candidate: s("c"),
                reviewer: s(reviewer),
                outcome,
            })
        })
        .collect();
    let mut a = GilReviewAgreement::default();
    let mut specific = [0.0f64; 3];
    ok(unsafe {
        gil_review_agreement(
            verdicts.as_ptr(),
            verdicts.len(),
            3,
            &raw mut a,
            specific.as_mut_ptr(),
        )
    });
    let specific: Vec<String> = specific.iter().map(|&x| num(x)).collect();
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
fn the_c_abi_reproduces_the_review_golden() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/review/golden.jsonl"
    );
    let want = std::fs::read_to_string(path).expect("the golden is committed");
    let want: Vec<&str> = want.lines().collect();
    let got = lines();
    assert_eq!(got.len(), want.len());
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g, w, "golden line {}", i + 1);
    }
}

#[test]
fn a_refused_input_comes_back_as_a_status_and_a_message() {
    let strata = [GilReviewStratumSpread {
        population: 10,
        deviation: 0.2,
    }; 3];
    let mut counts = [0u64; 3];
    let status = unsafe { gil_review_allocate_neyman(strata.as_ptr(), 3, 2, counts.as_mut_ptr()) };
    assert_eq!(status, GIL_REVIEW_INVALID_INPUT);
    assert!(last_error().contains("budget"), "{}", last_error());

    // The next successful call clears it.
    ok(unsafe { gil_review_allocate_neyman(strata.as_ptr(), 3, 9, counts.as_mut_ptr()) });
    assert_eq!(counts.iter().sum::<u64>(), 9);

    // A message too long for the buffer is not written; its length still comes back.
    let _ = unsafe { gil_review_allocate_neyman(strata.as_ptr(), 3, 2, counts.as_mut_ptr()) };
    let mut small = [0u8; 4];
    let len = unsafe { gil_review_last_error(small.as_mut_ptr(), small.len()) };
    assert!(len > small.len());
    assert_eq!(small, [0; 4]);
}

#[test]
fn a_malformed_call_is_refused_before_the_computation() {
    let mut out = GilReviewEstimate::default();
    // A null pointer with a length.
    let status = unsafe { gil_review_estimate(ptr::null(), 2, ptr::null(), 0, 0, 0, &raw mut out) };
    assert_eq!(status, GIL_REVIEW_INVALID_ARGUMENT);
    assert!(last_error().contains("strata is null"), "{}", last_error());

    // A stratum name that is not UTF-8.
    let bad = [0xffu8, 0xfe];
    let strata = [GilReviewStratum {
        name: GilReviewStr {
            ptr: bad.as_ptr(),
            len: bad.len(),
        },
        population: 5,
    }];
    let items = [GilReviewScoredItem {
        stratum: 0,
        score: 1.0,
        weight: 1.0,
    }];
    let status =
        unsafe { gil_review_estimate(strata.as_ptr(), 1, items.as_ptr(), 1, 0, 0, &raw mut out) };
    assert_eq!(status, GIL_REVIEW_INVALID_ARGUMENT);
    assert!(last_error().contains("not UTF-8"), "{}", last_error());

    // No output.
    let strata = estimate_strata();
    let items = plain_items();
    let status = unsafe {
        gil_review_estimate(
            strata.as_ptr(),
            strata.len(),
            items.as_ptr(),
            items.len(),
            0,
            0,
            ptr::null_mut(),
        )
    };
    assert_eq!(status, GIL_REVIEW_INVALID_ARGUMENT);

    // A stratum range past the record ids.
    let ids = [s("r1")];
    let strata = [GilReviewPopulationStratum {
        name: s("x"),
        first: 0,
        count: 2,
    }];
    let mut drawn = [GilReviewDrawnStratum::default()];
    let mut selected = [0u32; 2];
    let mut caveats = 0;
    let status = unsafe {
        gil_review_draw_pilot(
            strata.as_ptr(),
            1,
            ids.as_ptr(),
            1,
            5,
            0,
            drawn.as_mut_ptr(),
            selected.as_mut_ptr(),
            2,
            &raw mut caveats,
        )
    };
    assert_eq!(status, GIL_REVIEW_INVALID_ARGUMENT);
    assert!(last_error().contains("past record_ids"), "{}", last_error());

    // Too small an index buffer.
    let ids: Vec<String> = (0..10).map(|i| format!("r{i}")).collect();
    let records: Vec<GilReviewStr> = ids.iter().map(|id| s(id)).collect();
    let strata = [GilReviewPopulationStratum {
        name: s("x"),
        first: 0,
        count: 10,
    }];
    let status = unsafe {
        gil_review_draw_pilot(
            strata.as_ptr(),
            1,
            records.as_ptr(),
            records.len(),
            5,
            0,
            drawn.as_mut_ptr(),
            selected.as_mut_ptr(),
            2,
            &raw mut caveats,
        )
    };
    assert_eq!(status, GIL_REVIEW_INVALID_ARGUMENT);
    assert!(
        last_error().contains("selected holds 2"),
        "{}",
        last_error()
    );
}

#[test]
fn the_library_says_which_interface_it_speaks() {
    assert_eq!(gil_review_abi_version(), GIL_REVIEW_ABI_VERSION);
    assert_eq!(GIL_REVIEW_ABI_VERSION, 1);
}
