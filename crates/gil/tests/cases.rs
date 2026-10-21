//! Cases in the similar record memory, held to what the reference implementation's latest settlement tests expect: records
//! whose evidence reads the same after normalization form one case, answered by its latest settlement whatever order the
//! records are rebuilt in; when the latest moves to another case the record it superseded answers again, and a record
//! whose value is cleared is forgotten; an older saved copy rebuilt after a live settlement does not take the case back;
//! and choosing thresholds replays unordered records in settlement order, keeping one record per case.
//!
//! Recency weighing in the key memory (equal counts ranked by the latest settlement, the decay a correction needs to
//! overtake) is held in `key_memory.rs`, and the settlement time a restore keeps in `open_record.rs`.

use std::collections::BTreeMap;

use gil::decider::{Decider, Evidence, Source};
use gil::decision_record::DecisionRecord;
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::open_record::OpenRecord;
use gil::replay::{Choice, select_layers_with};
use gil::schema::{Attribute, AttributeRole, Record, Schema, Settlement};

const MINUTE: i64 = 60 * 10_000_000;
const DAY: i64 = 24 * 60 * MINUTE;
const MONDAY: i64 = 20_000 * DAY;

/// An observed summary and a decided team trusted from 0.3 on the similar record layer, answered by the nearest record.
fn ticket() -> Schema {
    let mut team = Attribute::new("team", AttributeRole::Decided);
    team.memory_threshold = Some(0.3);
    team.similar_votes = 1;
    Schema {
        name: "ticket".into(),
        attributes: vec![Attribute::new("summary", AttributeRole::Observed), team],
    }
}

fn doc(id: &str, summary: &str, team: Option<&str>, settled_at: i64) -> Record {
    let mut values = BTreeMap::from([("summary".to_owned(), summary.to_owned())]);
    if let Some(team) = team {
        values.insert("team".to_owned(), team.to_owned());
    }
    Record {
        id: id.into(),
        values,
        settled_at,
        ..Record::default()
    }
}

fn decider() -> Decider {
    Decider::new(KeyMemory::default(), Some(LexicalMemory::default()))
}

/// The value the similar record layer answers for a new record with this summary, and the record it came from.
fn similar_answer(decider: &Decider, summary: &str) -> Option<(String, String)> {
    let values = BTreeMap::from([("summary".to_owned(), summary.to_owned())]);
    let answer = decider.answer_one(&ticket(), "q", "team", &values, &BTreeMap::new(), None);
    answer
        .candidates
        .into_iter()
        .find(|c| c.source == Source::SimilarDocument)
        .map(|c| match c.evidence {
            Some(Evidence::Record { id, .. }) => (c.value, id),
            other => panic!("a similar record's candidate rests on a record, not {other:?}"),
        })
}

fn answered(decider: &Decider, summary: &str) -> Option<String> {
    similar_answer(decider, summary).map(|(value, _)| value)
}

#[test]
fn records_of_the_same_case_are_answered_by_the_latest_whatever_the_rebuild_order() {
    let schema = ticket();
    let records = [
        doc(
            "d1",
            "vpn drops every ten minutes",
            Some("facilities"),
            MONDAY,
        ),
        // The same case after normalization.
        doc(
            "d2",
            "VPN drops  every ten minutes",
            Some("network"),
            MONDAY + 2 * DAY,
        ),
        doc(
            "d3",
            "vpn drops every ten minutes",
            Some("security"),
            MONDAY + DAY,
        ),
    ];

    let reversed: Vec<Record> = records.iter().rev().cloned().collect();
    let shuffled = [records[1].clone(), records[0].clone(), records[2].clone()];
    for order in [&records[..], &reversed[..], &shuffled[..]] {
        let mut decider = decider();
        decider.rebuild(&schema, order);
        assert_eq!(
            similar_answer(&decider, "vpn drops every ten minutes"),
            Some(("network".to_owned(), "d2".to_owned())),
            "rebuilt in the order {:?}",
            order.iter().map(|r| r.id.as_str()).collect::<Vec<_>>()
        );
    }
}

#[test]
fn when_the_latest_of_a_case_moves_to_another_case_the_record_it_superseded_answers_again() {
    let schema = ticket();
    let mut decider = decider();
    decider.rebuild(
        &schema,
        &[
            doc("d1", "vpn drops", Some("facilities"), MONDAY),
            doc("d2", "vpn drops", Some("network"), MONDAY + DAY),
        ],
    );
    assert_eq!(answered(&decider, "vpn drops").as_deref(), Some("network"));

    // The latest record moves to another case: the one it superseded answers again.
    decider.rebuild(
        &schema,
        &[doc("d2", "printer jammed", Some("network"), MONDAY + DAY)],
    );
    assert_eq!(
        similar_answer(&decider, "vpn drops"),
        Some(("facilities".to_owned(), "d1".to_owned()))
    );

    // Then its value is cleared: it is forgotten — nothing answers its own summary any more — and the other case is
    // left as it was.
    decider.rebuild(&schema, &[doc("d2", "printer jammed", None, MONDAY + DAY)]);
    assert_eq!(similar_answer(&decider, "printer jammed"), None);
    assert_eq!(
        similar_answer(&decider, "vpn drops"),
        Some(("facilities".to_owned(), "d1".to_owned()))
    );
}

#[test]
fn rebuilding_an_older_copy_after_a_live_settlement_does_not_take_the_case_back() {
    let schema = ticket();
    let mut decider = decider();
    decider.rebuild(
        &schema,
        &[doc("d1", "vpn drops", Some("facilities"), MONDAY)],
    );

    let now = MONDAY + 5 * DAY;
    let mut open = OpenRecord::new(schema.clone(), "d2", now);
    open.observe(&mut decider, "summary", Some("vpn drops"));
    open.settle(&mut decider, "team", &Settlement::correct("network"), now);
    assert_eq!(answered(&decider, "vpn drops").as_deref(), Some("network"));

    // An older saved copy of d1 arrives again, as after a sync: it must not take the case back.
    decider.rebuild(
        &schema,
        &[doc("d1", "vpn drops", Some("facilities"), MONDAY)],
    );
    assert_eq!(
        similar_answer(&decider, "vpn drops"),
        Some(("network".to_owned(), "d2".to_owned()))
    );
}

#[test]
fn choosing_thresholds_replays_in_settlement_order_with_one_record_per_case() {
    let schema = ticket();
    let records = [
        doc("d3", "vpn drops", Some("network"), MONDAY + 2 * DAY),
        doc("d1", "vpn drops", Some("facilities"), MONDAY),
        doc("d2", "printer jammed", Some("facilities"), MONDAY + DAY),
    ];

    // Settlement order: d1, d2, d3. d3's lookup finds d1 at similarity 1 with the answer it corrected — wrong, so no
    // threshold down to 1 meets full precision — and d3 then replaces d1 as its case's representative.
    let mut key_memory = KeyMemory::default();
    let mut memory = LexicalMemory::default();
    let mut asked: Vec<DecisionRecord> = Vec::new();
    let choice = Choice {
        target_precision: 1.0,
        minimum_answered: 1,
        counting: None,
    };
    let layers = select_layers_with(
        &mut key_memory,
        &mut memory,
        &schema,
        "team",
        &records,
        choice,
        &mut |r| asked.push(r.clone()),
    );

    assert_eq!(
        asked.iter().map(|r| r.record.as_str()).collect::<Vec<_>>(),
        ["d1", "d2", "d3"]
    );
    let d3 = asked[2]
        .similar
        .as_ref()
        .expect("d3 finds a similar record");
    assert_eq!(
        (d3.record.as_str(), d3.value.as_str()),
        ("d1", "facilities")
    );
    assert!((d3.nearest - 1.0).abs() < 1e-9, "{}", d3.nearest);
    assert_eq!(d3.correct, Some(false));
    assert!(layers.memory.chosen.is_none());

    let nearest = memory.nearest("ticket/team", "summary: vpn drops", 5);
    assert_eq!(
        nearest
            .iter()
            .map(|m| (m.source.as_str(), m.answer.as_str()))
            .collect::<Vec<_>>(),
        [("d3", "network"), ("d2", "facilities")]
    );
}
