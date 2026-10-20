//! The `serde` feature: the data a host keeps or sends on reads back as it was written, and a schema written by hand
//! with only the settings it needs takes the same defaults as `Attribute::new`.
#![cfg(feature = "serde")]

use std::collections::BTreeMap;

use gil::schema::{
    Attribute, AttributePolicy, AttributeRole, CoarseLevel, Record, Schema, Settlement,
    SettlementKind,
};
use gil::tuning::{AttributeTuning, Stale, Tuning};
use gil::usage::{Decision, usage};
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
    .expect("a schema");
    schema.check().expect("a valid schema");

    let mut team = Attribute::new("team", AttributeRole::Decided);
    team.policy = AttributePolicy::ConfirmRequired;
    team.coarse = Some(CoarseLevel::by_prefix(2));
    let expected = Schema {
        name: "ticket".into(),
        attributes: vec![Attribute::new("summary", AttributeRole::Observed), team],
    };
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
    };
    let (written, read) = round_trip(&record);
    assert_eq!(written["settlements"]["team"], "AcceptOther");
    assert_eq!(serde_json::to_value(&read).unwrap(), written);

    // A record with nothing but its id and time.
    let bare: Record =
        serde_json::from_value(json!({ "id": "t2", "settled_at": 5 })).expect("a record");
    assert!(bare.values.is_empty() && bare.arrival.is_none());
    assert!(serde_json::from_value::<Record>(json!({ "settled_at": 5 })).is_err());
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
    let counts = usage([Decision {
        attribute: "team",
        answer: Some("network"),
        settled: Some("network"),
        settlement: Some(SettlementKind::Accept),
        probability: 1.0,
        promise: Some(0.85),
    }]);
    let (written, read) = round_trip(&counts);
    assert_eq!(read, counts);
    assert_eq!(written["team"]["accepted"], 1);
}
