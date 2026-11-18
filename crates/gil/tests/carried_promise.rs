//! A promise measured on records kept in another store (a tuning carried from elsewhere) is kept on the record apart from
//! one measured on this store's own, so `usage` can say whether each held.

use gil::decider::{Decider, Source};
use gil::key_memory::KeyMemory;
use gil::open_record::OpenRecord;
use gil::schema::{Attribute, AttributeRole, Record, Schema, Settlement, SettlementKind};
use gil::usage::usage;

const DAY: i64 = 864_000_000_000;

fn schema() -> Schema {
    let mut team = Attribute::new("team", AttributeRole::Decided);
    team.key_threshold = Some(0.5);
    Schema::new(
        "ticket",
        vec![Attribute::new("channel", AttributeRole::Observed), team],
    )
    .expect("a valid schema")
}

fn history() -> Vec<Record> {
    (1..=4)
        .map(|i| Record {
            id: format!("h{i}"),
            values: [("channel", "phone"), ("team", "network")]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
            settled_at: i,
            ..Record::default()
        })
        .collect()
}

/// A record whose `team` answer was accepted or corrected, under `promise`, carried or not.
fn answered(id: &str, kind: SettlementKind, promise: f64, carried: bool) -> Record {
    let mut record = Record {
        id: id.to_owned(),
        ..Record::default()
    };
    record.values.insert("team".into(), "network".into());
    record.settlements.insert("team".into(), kind);
    record.suggested.insert("team".into(), "network".into());
    record.sources.insert("team".into(), Source::KeyMemory);
    record.promises.insert("team".into(), promise);
    if carried {
        record.promises_carried.insert("team".into());
    }
    record
}

#[test]
fn a_carried_promise_is_kept_by_the_settlement_that_marks_it_and_cleared_by_the_next() {
    let schema = schema();
    let mut decider = Decider::new(KeyMemory::default(), None);
    decider.rebuild(&schema, &history());
    let mut open = OpenRecord::new(schema.clone(), "d1", DAY);
    let shown = open
        .observe(&mut decider, "channel", Some("phone"))
        .remove(0);
    assert!(shown.answered());

    open.settle(
        &mut decider,
        "team",
        &Settlement::accept("network")
            .with_answer(&shown, Some(0.9))
            .with_carried_promise(),
        2 * DAY,
    );
    let saved = open.snapshot();
    assert_eq!(saved.promises["team"], 0.9);
    assert!(saved.promises_carried.contains("team"));
    assert_eq!(schema.check_record(&saved), Ok(()));

    // Settled again without the mark, the earlier one is not carried over.
    open.settle(
        &mut decider,
        "team",
        &Settlement::accept("network").with_answer(&shown, Some(0.9)),
        3 * DAY,
    );
    assert!(open.snapshot().promises_carried.is_empty());

    // It survives reopening what was saved.
    let reopened = OpenRecord::reopen(schema, &saved);
    assert!(reopened.snapshot().promises_carried.contains("team"));
}

#[test]
fn only_a_promise_can_be_carried() {
    let schema = schema();
    let mut decider = Decider::new(KeyMemory::default(), None);
    decider.rebuild(&schema, &history());
    let mut open = OpenRecord::new(schema.clone(), "d2", DAY);
    let shown = open
        .observe(&mut decider, "channel", Some("phone"))
        .remove(0);

    // The mark on a suggestion with no promise is dropped by the builder and refused if set by hand.
    assert_eq!(
        Settlement::accept("network")
            .with_answer(&shown, None)
            .with_carried_promise()
            .in_view
            .map(|v| v.carried),
        Some(false)
    );
    let mut by_hand = Settlement::accept("network").with_answer(&shown, None);
    by_hand.in_view.as_mut().unwrap().carried = true;
    let error = open.check_settle("team", &by_hand).unwrap_err();
    assert!(
        error.to_string().contains("carried promise is a promise"),
        "{error}"
    );

    let mut record = answered("r1", SettlementKind::Accept, 0.9, true);
    assert_eq!(schema.check_record(&record), Ok(()));
    record.promises.clear();
    let error = schema.check_record(&record).unwrap_err();
    assert!(
        error.contains("'team' is marked as carrying a promise"),
        "{error}"
    );
}

#[test]
fn usage_counts_the_promises_carried_from_another_store_apart() {
    let records = [
        answered("a", SettlementKind::Accept, 0.9, false),
        answered("b", SettlementKind::Accept, 0.9, false),
        answered("c", SettlementKind::Correct, 0.9, false),
        answered("d", SettlementKind::Accept, 0.8, true),
        answered("e", SettlementKind::Correct, 0.8, true),
        answered("f", SettlementKind::Correct, 0.8, true),
    ];
    let team = &usage(&records)["team"];
    assert_eq!((team.promised, team.carried_promised), (6, 3));
    assert!((team.carried_promise().unwrap() - 0.8).abs() < 1e-12);
    assert!((team.promise().unwrap() - 0.85).abs() < 1e-12);
    // The whole realized 3 of 6; the carried ones 1 of 3; this store's own 2 of 3.
    assert_eq!(team.realized(), Some(0.5));
    assert!((team.carried_realized().unwrap() - 1.0 / 3.0).abs() < 1e-12);
    assert_eq!(
        (team.right_weight - team.carried_right_weight)
            / (team.judged_weight - team.carried_judged_weight),
        2.0 / 3.0
    );

    // Nothing carried: the carried counts are empty and the whole is what it was.
    let own = usage(&records[..3])["team"].clone();
    assert_eq!(own.carried_promised, 0);
    assert_eq!(own.carried_realized(), None);
    assert_eq!(own.carried_promise(), None);
}
