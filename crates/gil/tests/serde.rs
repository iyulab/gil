//! The `serde` feature: the data a host keeps or sends on reads back as it was written, and a schema written by hand
//! with only the settings it needs takes the same defaults as `Attribute::new`.
#![cfg(feature = "serde")]

use std::collections::BTreeMap;

use gil::decider::Source;
use gil::schema::{
    Attribute, AttributePolicy, AttributeRole, CoarseLevel, Record, Schema, Settlement,
    SettlementKind,
};
use gil::tuning::{AttributeTuning, Stale, Tuning};
use gil::usage::usage;
use serde_json::{Value, json};

fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned>(value: &T) -> (Value, T) {
    let written = serde_json::to_value(value).expect("serializes");
    let read: T = serde_json::from_value(written.clone()).expect("reads back");
    (written, read)
}

#[test]
fn a_schema_with_only_the_settings_it_needs_takes_the_defaults() {
    let schema: Schema = serde_json::from_value(json!({
        "name": "ticket",
        "attributes": [
            { "name": "summary", "role": "observed" },
            { "name": "team", "role": "decided", "policy": "confirm_required",
              "coarse": { "prefix": 2 } },
        ],
    }))
    .expect("a valid schema");

    let mut team = Attribute::new("team", AttributeRole::Decided);
    team.policy = AttributePolicy::ConfirmRequired;
    team.coarse = Some(CoarseLevel::by_prefix(2));
    let expected = Schema::new(
        "ticket",
        vec![Attribute::new("summary", AttributeRole::Observed), team],
    )
    .expect("a valid schema");
    assert_eq!(
        serde_json::to_value(&schema).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    let (written, read) = round_trip(&expected);
    assert_eq!(serde_json::to_value(&read).unwrap(), written);
    assert_eq!(written["attributes"][1]["role"], "decided");
    assert_eq!(written["attributes"][0]["similar_votes"], 10);
    assert_eq!(written["attributes"][0]["use_as_evidence"], true);
}

#[test]
fn an_unknown_setting_or_role_does_not_read() {
    let unknown = serde_json::from_value::<Schema>(json!({
        "name": "ticket",
        "attributes": [{ "name": "team", "role": "decided", "colour": "red" }],
    }));
    assert!(unknown.unwrap_err().to_string().contains("colour"));

    let role = serde_json::from_value::<Schema>(json!({
        "name": "ticket",
        "attributes": [{ "name": "team", "role": "judged" }],
    }));
    assert!(role.unwrap_err().to_string().contains("judged"));
}

#[test]
fn a_schema_that_breaks_a_rule_does_not_read() {
    let unknown_field = serde_json::from_value::<Schema>(json!({
        "name": "ticket",
        "attributes": [{ "name": "team", "role": "decided" }],
        "colour": "red",
    }));
    assert!(unknown_field.unwrap_err().to_string().contains("colour"));

    let undecided = serde_json::from_str::<Schema>(
        r#"{ "name": "ticket", "attributes": [{ "name": "summary", "role": "observed" }] }"#,
    );
    assert!(
        undecided
            .unwrap_err()
            .to_string()
            .starts_with("a schema needs at least one decided attribute"),
    );
}

#[test]
fn a_record_reads_back_with_how_each_value_was_settled() {
    let record = Record {
        id: "t1".into(),
        values: BTreeMap::from([
            ("summary".into(), "vpn drops".into()),
            ("team".into(), "network".into()),
        ]),
        sets: BTreeMap::from([("tags".into(), vec!["remote".into(), "vpn".into()])]),
        settled_at: 638_000_000_000_000_000,
        settlements: BTreeMap::from([
            ("team".into(), SettlementKind::AcceptOther),
            ("tags".into(), SettlementKind::Set),
        ]),
        probabilities: BTreeMap::new(),
        arrival: Some(vec!["summary".into(), "team".into(), "tags".into()]),
        batch: Some("import-7".into()),
        ..Record::default()
    };
    let (written, read) = round_trip(&record);
    assert_eq!(written["settlements"]["team"], "AcceptOther");
    assert_eq!(written["batch"], "import-7");
    assert_eq!(serde_json::to_value(&read).unwrap(), written);

    // A record with nothing but its id and time.
    let bare: Record =
        serde_json::from_value(json!({ "id": "t2", "settled_at": 5 })).expect("a record");
    assert!(bare.values.is_empty() && bare.arrival.is_none() && bare.batch.is_none());
    // A record that names no batch writes none.
    assert!(serde_json::to_value(&bare).unwrap().get("batch").is_none());
    assert!(serde_json::from_value::<Record>(json!({ "settled_at": 5 })).is_err());
}

#[test]
fn a_record_carries_the_score_a_judge_outside_gil_gave_its_answer() {
    let line = json!({
        "id": "t1",
        "settled_at": 5,
        "values": { "team": "network" },
        "settlements": { "team": "Accept" },
        "suggested": { "team": "network" },
        "sources": { "team": "Model" },
        "scores": { "team": 3.2, "owner": -0.5 }
    });
    let record: Record = serde_json::from_value(line.clone()).expect("a record");
    assert_eq!(record.scores["team"], 3.2);
    assert_eq!(record.scores["owner"], -0.5);
    assert_eq!(
        serde_json::to_value(&record).unwrap()["scores"],
        line["scores"]
    );

    // Left out when none is kept, and null reads as left out.
    let bare: Record =
        serde_json::from_value(json!({ "id": "t2", "settled_at": 5, "scores": null }))
            .expect("a record");
    assert!(bare.scores.is_empty());
    assert!(serde_json::to_value(&bare).unwrap().get("scores").is_none());

    // Refused rather than half understood: a score that is not a number.
    assert!(
        serde_json::from_value::<Record>(
            json!({ "id": "t3", "settled_at": 5, "scores": { "team": "high" } })
        )
        .is_err()
    );
}

#[test]
fn a_record_carries_what_was_in_view_how_long_it_took_and_who_settled_it() {
    let line = json!({
        "id": "t1",
        "settled_at": 5,
        "values": { "summary": "vpn drops", "team": "network" },
        "settlements": { "team": "Correct" },
        "suggested": { "team": "desk" },
        "sources": { "team": "KeyMemory" },
        "promises": { "team": 0.9 },
        "settle_ms": { "team": 4200 },
        "open_ms": 61000,
        "settled_by": { "team": "p-17" },
        "form_version": "9f2c"
    });
    let record: Record = serde_json::from_value(line.clone()).expect("a record");
    assert_eq!(record.suggested["team"], "desk");
    assert_eq!(record.sources["team"], Source::KeyMemory);
    assert_eq!(record.promises["team"], 0.9);
    assert_eq!(record.settle_ms["team"], 4200);
    assert_eq!(record.open_ms, Some(61000));
    assert_eq!(record.settled_by["team"], "p-17");
    assert_eq!(record.form_version.as_deref(), Some("9f2c"));
    let written = serde_json::to_value(&record).unwrap();
    for entry in [
        "suggested",
        "sources",
        "promises",
        "settle_ms",
        "open_ms",
        "settled_by",
        "form_version",
    ] {
        assert_eq!(written[entry], line[entry], "{entry}");
    }
    // A layer named as answers name it; the host's model is "Model".
    let model: Record = serde_json::from_value(
        json!({ "id": "t1", "settled_at": 5, "sources": { "team": "Model" } }),
    )
    .expect("a record");
    assert_eq!(model.sources["team"], Source::External);

    // None of them written when none is known, and null reads as left out.
    let bare: Record = serde_json::from_value(json!({
        "id": "t2", "settled_at": 5, "suggested": null, "promises": null, "open_ms": null
    }))
    .expect("a record");
    assert!(bare.suggested.is_empty() && bare.promises.is_empty() && bare.open_ms.is_none());
    let written = serde_json::to_value(&bare).unwrap();
    for entry in [
        "suggested",
        "sources",
        "promises",
        "settle_ms",
        "open_ms",
        "settled_by",
        "form_version",
    ] {
        assert!(written.get(entry).is_none(), "{entry}");
    }

    // Refused rather than half understood: a promise out of [0, 1], a layer by another name, negative milliseconds.
    let refused = |extra: Value| {
        let mut line = json!({ "id": "t3", "settled_at": 5 });
        line.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::from_value::<Record>(line)
            .unwrap_err()
            .to_string()
    };
    assert!(refused(json!({ "promises": { "team": 1.2 } })).contains("not in [0, 1]"));
    assert!(refused(json!({ "promises": { "team": -0.1 } })).contains("not in [0, 1]"));
    refused(json!({ "sources": { "team": "Key" } }));
    assert!(refused(json!({ "sources": { "team": "None" } })).contains("layer that answered"));
    refused(json!({ "open_ms": -1 }));
    refused(json!({ "settle_ms": { "team": 1.5 } }));
}

#[test]
fn a_record_reads_its_time_as_ticks_or_as_text_and_null_as_left_out() {
    let read = |line: Value| serde_json::from_value::<Record>(line);
    // 2025-10-22 16:00 UTC, as ticks, as text in UTC and as text nine hours ahead.
    for settled_at in [
        json!(17_611_488_000_000_000_i64),
        json!("2025-10-22T16:00:00.0000000+00:00"),
        json!("2025-10-23T01:00:00.0000000+09:00"),
    ] {
        let record = read(json!({ "id": "t1", "settled_at": settled_at })).expect("a record");
        assert_eq!(record.settled_at, 17_611_488_000_000_000);
    }
    let before_the_epoch =
        read(json!({ "id": "t1", "settled_at": "1969-12-31T23:59:59.9999999-00:00" }));
    assert_eq!(before_the_epoch.expect("a record").settled_at, -1);
    // A leap day, and the fraction in ticks.
    let leap = read(json!({ "id": "t1", "settled_at": "2024-02-29T00:00:00.0000001+00:00" }))
        .expect("a record");
    assert_eq!(leap.settled_at, 17_091_648_000_000_001);

    // It is written back in ticks.
    let written = serde_json::to_value(
        read(json!({
            "id": "t1", "settled_at": "2025-10-22T16:00:00.0000000+00:00"
        }))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(written["settled_at"], 17_611_488_000_000_000_i64);

    for bad in [
        "2025-10-22 16:00:00.0000000+00:00",
        "2025-10-22T16:00:00Z",
        "2025-13-01T00:00:00.0000000+00:00",
        "2025-02-29T00:00:00.0000000+00:00",
        "2025-10-22T24:00:00.0000000+00:00",
        "2025-10-22T16:00:00.0000000+15:00",
        "2025-10-22T16:00:00.00000x0+00:00",
    ] {
        let refused = read(json!({ "id": "t1", "settled_at": bad })).expect_err(bad);
        assert!(
            refused.to_string().contains("neither ticks nor a time"),
            "{refused}"
        );
    }

    let nulls = read(json!({
        "id": "t1", "settled_at": 5, "values": null, "sets": null, "settlements": null,
        "probabilities": null, "arrival": null, "batch": null,
    }))
    .expect("a record");
    assert!(nulls.values.is_empty() && nulls.sets.is_empty() && nulls.settlements.is_empty());
    assert!(nulls.probabilities.is_empty() && nulls.arrival.is_none() && nulls.batch.is_none());
    assert!(read(json!({ "id": "t1", "settled_at": null })).is_err());
}

#[test]
fn a_settlement_reads_back_with_its_kind_and_probability() {
    for settlement in [
        Settlement::check("network", 0.25),
        Settlement::correct_other("security"),
        Settlement::set(["vpn", "remote"]),
        Settlement::reject(),
    ] {
        let (_, read) = round_trip(&settlement);
        assert_eq!(read, settlement);
    }
}

#[test]
fn a_tuning_and_its_staleness_read_back() {
    let tuning = Tuning {
        version: "0.5.0".into(),
        schema: u64::MAX - 7,
        settled: 1_200,
        target_precision: 0.9,
        attributes: vec![AttributeTuning {
            attribute: "team".into(),
            key_threshold: Some(0.83),
            key_promise: Some(0.912),
            typed_key_thresholds: Some(vec![None, Some(0.7)]),
            ..AttributeTuning::default()
        }],
    };
    let (_, read) = round_trip(&tuning);
    assert_eq!(read, tuning);

    for stale in [
        Stale::Version {
            chosen: "0.4.0".into(),
        },
        Stale::Schema,
        Stale::Settled {
            chosen: 1_200,
            now: 1_400,
        },
    ] {
        let (_, read) = round_trip(&stale);
        assert_eq!(read, stale);
    }
}

#[test]
fn usage_counts_read_back() {
    let mut record = Record {
        id: "r".to_owned(),
        ..Record::default()
    };
    record
        .values
        .insert("team".to_owned(), "network".to_owned());
    record
        .settlements
        .insert("team".to_owned(), SettlementKind::Accept);
    record.promises.insert("team".to_owned(), 0.85);
    let counts = usage([&record]);
    let (written, read) = round_trip(&counts);
    assert_eq!(read, counts);
    assert_eq!(written["team"]["accepted"], 1);
}

#[test]
fn a_judgement_from_outside_is_named_as_the_reference_names_it() {
    assert_eq!(serde_json::to_value(Source::External).unwrap(), "Model");
    assert_eq!(
        serde_json::to_value(Source::KeyMemory).unwrap(),
        Source::KeyMemory.name()
    );
    let (_, read) = round_trip(&Source::External);
    assert_eq!(read, Source::External);
}

#[test]
fn an_answer_reads_back_with_its_evidence() {
    use gil::decider::{Answer, Candidate, Evidence};
    use gil::key_memory::Key;
    let answer = Answer {
        attribute: "team".into(),
        candidates: vec![
            Candidate {
                value: "network".into(),
                score: 0.93,
                source: Source::KeyMemory,
                evidence: Some(Evidence::Key(Key {
                    attribute: "component".into(),
                    value: "vpn".into(),
                })),
                trusted: true,
            },
            Candidate {
                value: "security".into(),
                score: 0.4,
                source: Source::SimilarRecord,
                evidence: Some(Evidence::Record {
                    id: "t3".into(),
                    similarity: 0.71,
                }),
                trusted: false,
            },
        ],
        source: Source::KeyMemory,
        coarse: None,
        policy: AttributePolicy::ConfirmRequired,
        needs_external: Some(vec![("summary".into(), "vpn drops".into())]),
    };
    let (written, read) = round_trip(&answer);
    assert_eq!(read, answer);
    assert_eq!(
        written["candidates"][0]["evidence"],
        json!({ "key": { "attribute": "component", "value": "vpn" } })
    );
    assert_eq!(
        written["candidates"][1]["evidence"],
        json!({ "record": { "id": "t3", "similarity": 0.71 } })
    );
    assert_eq!(written["policy"], "confirm_required");
    assert_eq!(written["needs_external"], json!([["summary", "vpn drops"]]));
}
