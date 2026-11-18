//! The score scale (`gil::tuning::SCALE`) says when a threshold chosen elsewhere stops meaning what it did, so it must
//! move whenever a score does. The golden decision lines pin every score to the bit; their digest is pinned here for the
//! scale they were made under. A change that moves a score fails this test until the scale is raised and its digest
//! added below — the fixture is then regenerated on purpose, not by accident.

mod common;

use common::fixture;
use gil::tuning::SCALE;
use serde_json::Value;

/// Each scale's digest of the golden `decision` lines; a new scale adds a row, an old row is never edited.
const DIGESTS: &[(u32, u64)] = &[(1, 0x0966_b6f9_249b_e32f)];

fn fnv1a(bytes: &[u8], mut hash: u64) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[test]
fn the_golden_scores_are_those_of_the_current_score_scale() {
    let golden = fixture("golden.jsonl");
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    let mut decisions = 0;
    for line in golden.lines().filter(|l| !l.trim().is_empty()) {
        let value: Value = serde_json::from_str(line).expect("a golden line is JSON");
        if value["kind"] == "decision" {
            digest = fnv1a(line.as_bytes(), digest);
            digest = fnv1a(b"\n", digest);
            decisions += 1;
        }
    }
    assert!(decisions > 0, "the golden has decision lines");

    let pinned = DIGESTS
        .iter()
        .find(|(scale, _)| *scale == SCALE)
        .unwrap_or_else(|| panic!("add a row for score scale {SCALE}: ({SCALE}, {digest:#x})"));
    assert_eq!(
        pinned.1,
        digest,
        "the golden scores changed under score scale {SCALE}: raise `gil::tuning::SCALE` and add the row ({}, {digest:#x}) — \
         thresholds chosen on the old scores no longer mean what they did",
        SCALE + 1
    );
}
