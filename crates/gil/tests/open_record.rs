//! An open record and the external judgement round trip, held to what the reference implementation's session and model
//! layer do (its `FormSession`, `IFieldModel` and settlement origin tests): which attributes are answered after each event,
//! how settlements travel with the snapshot, and where an external judgement's values land.

use std::collections::BTreeMap;

use gil::decider::{Decider, ExternalCandidate, Source};
use gil::key_memory::KeyMemory;
use gil::open_record::OpenRecord;
use gil::schema::{
    Attribute, AttributePolicy, AttributeRole, Record, Schema, Settlement, SettlementKind,
};

const MINUTE: i64 = 60 * 10_000_000;
const DAY: i64 = 24 * 60 * MINUTE;

fn attribute(name: &str, role: AttributeRole) -> Attribute {
    Attribute::new(name, role)
}

fn ticket() -> Schema {
    let mut reporter = attribute("reporter", AttributeRole::Observed);
    reporter.use_as_evidence = false;
    Schema {
        name: "ticket".into(),
        attributes: vec![
            reporter,
            attribute("component", AttributeRole::Observed),
            attribute("summary", AttributeRole::Observed),
            attribute("team", AttributeRole::Decided),
        ],
    }
}

fn record(id: &str, values: &[(&str, &str)]) -> Record {
    Record {
        id: id.into(),
        values: values
            .iter()
            .map(|(a, v)| ((*a).to_owned(), (*v).to_owned()))
            .collect(),
        settled_at: id[1..].parse::<i64>().unwrap() * MINUTE,
        ..Record::default()
    }
}

fn keys(answer: &gil::decider::Answer) -> Vec<&str> {
    answer
        .needs_external
        .as_ref()
        .expect("an answer asking for an external judgement")
        .iter()
        .map(|(a, _)| a.as_str())
        .collect()
}

#[test]
fn an_external_judgement_is_asked_for_only_where_no_memory_has_evidence_and_then_leads_the_frequency_guess()
 {
    let schema = ticket();
    let mut decider = Decider::new(KeyMemory::default(), None);
    decider.rebuild(
        &schema,
        &[
            record("d1", &[("component", "vpn"), ("team", "network")]),
            record("d2", &[("component", "vpn"), ("team", "network")]),
        ],
    );
    let mut open = OpenRecord::new(schema, "d3", 3 * MINUTE);

    // A known key: memory has evidence, and no external judgement is asked for.
    let keyed = open.observe(&mut decider, "component", Some("vpn"));
    assert_eq!(keyed.len(), 1);
    assert_eq!(keyed[0].source, Source::SettledFieldMemory);
    assert_eq!(keyed[0].needs_external, None);

    // No key matches: the answer asks, with the evidence the reporter is kept out of.
    open.observe(&mut decider, "reporter", Some("Kim"));
    let asked = open.observe(&mut decider, "component", Some("badge reader"));
    assert_eq!(keys(&asked[0]), ["component"]);
    assert_eq!(
        asked[0].needs_external.as_ref().unwrap()[0].1,
        "badge reader"
    );

    // The judgement offered back leads; the overall frequency follows.
    let offered = open.offer_external(
        &decider,
        "team",
        &[ExternalCandidate {
            value: "security".into(),
            probability: 0.8,
            trusted: true,
        }],
    );
    assert_eq!(offered.source, Source::External);
    assert!(offered.answered());
    assert_eq!(offered.needs_external, None);
    assert_eq!(
        offered
            .candidates
            .iter()
            .map(|c| (c.value.as_str(), c.source))
            .collect::<Vec<_>>(),
        [
            ("security", Source::External),
            ("network", Source::SettledFieldMemory)
        ]
    );
}

#[test]
fn a_judgement_offered_where_memory_had_evidence_changes_nothing_and_one_outside_the_domain_is_left_out()
 {
    let mut schema = ticket();
    schema.attributes[3].candidates = Some(vec!["network".into(), "security".into()]);
    let mut decider = Decider::new(KeyMemory::default(), None);
    decider.rebuild(
        &schema,
        &[record("d1", &[("component", "vpn"), ("team", "network")])],
    );
    let mut open = OpenRecord::new(schema, "d2", 2 * MINUTE);
    let keyed = open
        .observe(&mut decider, "component", Some("vpn"))
        .remove(0);
    let ranked = [ExternalCandidate {
        value: "security".into(),
        probability: 0.9,
        trusted: true,
    }];
    assert_eq!(open.offer_external(&decider, "team", &ranked), keyed);

    open.observe(&mut decider, "component", Some("printer"));
    let outside = open.offer_external(
        &decider,
        "team",
        &[ExternalCandidate {
            value: "facilities".into(),
            probability: 0.9,
            trusted: true,
        }],
    );
    assert!(
        outside
            .candidates
            .iter()
            .all(|c| c.source != Source::External)
    );
}

#[test]
fn an_open_record_hands_its_evidence_over_in_arrival_order_and_a_changed_value_moves_to_the_end() {
    let schema = ticket();
    let mut decider = Decider::new(KeyMemory::default(), None);
    let mut open = OpenRecord::new(schema.clone(), "d1", MINUTE);

    let first = open.observe(&mut decider, "summary", Some("door will not open"));
    let second = open.observe(&mut decider, "component", Some("badge reader"));
    let third = open.observe(&mut decider, "summary", Some("door opens late"));
    assert_eq!(keys(&first[0]), ["summary"]);
    assert_eq!(keys(&second[0]), ["summary", "component"]);
    assert_eq!(keys(&third[0]), ["component", "summary"]);
    assert_eq!(
        third[0].needs_external.as_ref().unwrap()[1].1,
        "door opens late"
    );

    // A stateless answer has no history: the schema's order.
    let values = BTreeMap::from([
        ("summary".to_owned(), "x".to_owned()),
        ("component".to_owned(), "y".to_owned()),
    ]);
    let stateless = decider.answer(&schema, "d2", &values, &BTreeMap::new());
    assert_eq!(keys(&stateless[0]), ["component", "summary"]);
}

fn settled_ticket() -> Schema {
    let mut team = attribute("team", AttributeRole::Decided);
    team.memory_threshold = Some(0.3);
    Schema {
        name: "ticket".into(),
        attributes: vec![attribute("summary", AttributeRole::Observed), team],
    }
}

#[test]
fn entering_or_filling_records_how_the_attribute_was_settled_and_moves_the_time() {
    let monday = 20_000 * DAY;
    let mut decider = Decider::new(KeyMemory::default(), None);
    let mut open = OpenRecord::new(settled_ticket(), "d1", monday - 3 * DAY);
    open.observe(&mut decider, "summary", Some("vpn drops"));

    open.settle(&mut decider, "team", &Settlement::enter("network"), monday);
    let snapshot = open.snapshot();
    assert_eq!(
        snapshot.settlements,
        BTreeMap::from([("team".to_owned(), SettlementKind::Enter)])
    );
    assert_eq!(snapshot.settled_at, monday);

    open.settle(
        &mut decider,
        "team",
        &Settlement::fill("network"),
        monday + DAY,
    );
    assert_eq!(open.snapshot().settlements["team"], SettlementKind::Fill);
    assert_eq!(open.snapshot().settled_at, monday + DAY);

    open.settle(
        &mut decider,
        "team",
        &Settlement::correct("facilities"),
        monday + DAY,
    );
    assert_eq!(open.snapshot().settlements["team"], SettlementKind::Correct);
    assert!(!open.snapshot().settlements.contains_key("summary"));

    open.settle(
        &mut decider,
        "team",
        &Settlement::check("facilities", 0.25),
        monday + 2 * DAY,
    );
    assert_eq!(open.snapshot().probabilities["team"], 0.25);
}

#[test]
fn rejecting_reverting_or_restoring_forgets_how_the_attribute_was_settled() {
    let mut decider = Decider::new(KeyMemory::default(), None);
    let mut open = OpenRecord::new(settled_ticket(), "d1", 0);
    open.observe(&mut decider, "summary", Some("vpn drops"));

    open.settle(&mut decider, "team", &Settlement::accept("network"), DAY);
    open.settle(
        &mut decider,
        "team",
        &Settlement::restore("network"),
        2 * DAY,
    );
    assert!(open.snapshot().settlements.is_empty());
    assert_eq!(open.snapshot().settled_at, DAY); // restoring keeps the time

    open.settle(
        &mut decider,
        "team",
        &Settlement::accept("network"),
        3 * DAY,
    );
    open.settle(&mut decider, "team", &Settlement::reject(), 4 * DAY);
    assert!(open.snapshot().settlements.is_empty());
    assert!(open.answer(&decider).is_empty()); // a rejected attribute is not answered again…

    let reverted = open.settle(&mut decider, "team", &Settlement::revert(), 5 * DAY);
    assert_eq!(reverted.len(), 1); // …until it is reverted, which answers it afresh
    assert_eq!(reverted[0].attribute, "team");
    assert!(open.snapshot().settlements.is_empty());
}

#[test]
fn reopening_a_saved_record_brings_back_its_values_arrival_and_settlements() {
    let saved = Record {
        id: "d1".into(),
        values: BTreeMap::from([
            ("summary".to_owned(), "vpn drops".to_owned()),
            ("team".to_owned(), "network".to_owned()),
        ]),
        settled_at: DAY,
        arrival: Some(vec!["team".into(), "summary".into()]),
        settlements: BTreeMap::from([
            ("team".to_owned(), SettlementKind::Enter),
            ("summary".to_owned(), SettlementKind::Accept), // not decided: nothing to keep
        ]),
        ..Record::default()
    };
    let reopened = OpenRecord::reopen(settled_ticket(), &saved).snapshot();
    assert_eq!(reopened.values, saved.values);
    assert_eq!(
        reopened.arrival,
        Some(vec!["team".into(), "summary".into()])
    );
    assert_eq!(reopened.settled_at, DAY);
    assert_eq!(
        reopened.settlements,
        BTreeMap::from([("team".to_owned(), SettlementKind::Enter)])
    );

    let unordered = Record {
        arrival: None,
        settlements: BTreeMap::new(),
        ..saved
    };
    assert_eq!(
        OpenRecord::reopen(settled_ticket(), &unordered)
            .snapshot()
            .arrival,
        Some(vec!["summary".into(), "team".into()])
    );
}

fn paper() -> Schema {
    let mut topics = attribute("topics", AttributeRole::Decided);
    topics.multiple = true;
    topics.key_threshold = Some(0.3);
    Schema {
        name: "paper".into(),
        attributes: vec![
            attribute("area", AttributeRole::Observed),
            topics,
            attribute("lead", AttributeRole::Decided),
        ],
    }
}

fn paper_record(id: &str, area: &str, topics: &[&str], lead: &str) -> Record {
    Record {
        sets: BTreeMap::from([(
            "topics".to_owned(),
            topics.iter().map(|t| (*t).to_owned()).collect(),
        )]),
        ..record(id, &[("area", area), ("lead", lead)])
    }
}

#[test]
fn a_set_stays_open_once_settled_and_its_chosen_values_back_the_rest() {
    let schema = paper();
    let mut decider = Decider::new(KeyMemory::new(60, 1.0), None);
    decider.rebuild(
        &schema,
        &[
            paper_record("d1", "ai", &["nlp", "vision"], "kim"),
            paper_record("d2", "ai", &["nlp"], "lee"),
            paper_record("d3", "bio", &["genomics", "nlp"], "park"),
        ],
    );
    let mut open = OpenRecord::new(schema, "d4", 4 * MINUTE);
    let observed = open.observe(&mut decider, "area", Some("ai"));
    let topics = observed.iter().find(|a| a.attribute == "topics").unwrap();
    assert_eq!(
        topics
            .candidates
            .iter()
            .map(|c| (c.value.as_str(), c.trusted))
            .collect::<Vec<_>>(),
        [("nlp", true), ("vision", false), ("genomics", false)]
    );

    let now = DAY;
    let after = open.settle(&mut decider, "topics", &Settlement::set(["nlp"]), now);
    assert_eq!(
        after
            .iter()
            .map(|a| a.attribute.as_str())
            .collect::<Vec<_>>(),
        ["topics", "lead"]
    );
    assert_eq!(
        after[0]
            .candidates
            .iter()
            .map(|c| (c.value.as_str(), c.trusted))
            .collect::<Vec<_>>(),
        [("vision", true), ("genomics", true)]
    );

    let saved = open.snapshot();
    assert_eq!(saved.sets["topics"], ["nlp"]);
    assert!(!saved.values.contains_key("topics"));
    assert_eq!(saved.arrival, Some(vec!["area".into(), "topics".into()]));
    assert_eq!(saved.settled_at, now);
    assert_eq!(
        open.answer(&decider)
            .iter()
            .map(|a| a.attribute.as_str())
            .collect::<Vec<_>>(),
        ["topics", "lead"]
    );
}

#[test]
fn settling_a_set_again_replaces_it_and_an_empty_set_clears_it() {
    let mut decider = Decider::new(KeyMemory::new(60, 1.0), None);
    let mut open = OpenRecord::new(paper(), "d4", 0);
    open.settle(
        &mut decider,
        "topics",
        &Settlement::set(["vision", "nlp", "nlp"]),
        1,
    );
    assert_eq!(open.snapshot().sets["topics"], ["nlp", "vision"]);
    open.settle(&mut decider, "topics", &Settlement::set(["genomics"]), 2);
    assert_eq!(open.snapshot().sets["topics"], ["genomics"]);
    open.settle(&mut decider, "topics", &Settlement::set([]), 3);
    assert!(!open.snapshot().sets.contains_key("topics"));
    assert_eq!(open.snapshot().arrival, Some(vec![]));
    assert!(open.snapshot().settlements.is_empty()); // no values left: nothing settled
}

#[test]
#[should_panic(expected = "'topics' takes several values")]
fn a_set_is_not_settled_with_one_value() {
    let mut decider = Decider::new(KeyMemory::default(), None);
    OpenRecord::new(paper(), "d4", 0).settle(&mut decider, "topics", &Settlement::accept("nlp"), 1);
}

#[test]
#[should_panic(expected = "'lead' takes one value")]
fn a_single_value_is_not_settled_as_a_set() {
    let mut decider = Decider::new(KeyMemory::default(), None);
    OpenRecord::new(paper(), "d4", 0).settle(&mut decider, "lead", &Settlement::set(["kim"]), 1);
}

#[test]
fn an_attribute_that_is_off_is_never_answered() {
    let mut schema = settled_ticket();
    schema.attributes[1].policy = AttributePolicy::Off;
    let mut decider = Decider::new(KeyMemory::default(), None);
    let mut open = OpenRecord::new(schema.clone(), "d1", 0);
    assert!(
        open.observe(&mut decider, "summary", Some("vpn drops"))
            .is_empty()
    );
    assert!(open.answer(&decider).is_empty());
    let values = BTreeMap::from([("summary".to_owned(), "vpn drops".to_owned())]);
    assert!(
        decider
            .answer(&schema, "d2", &values, &BTreeMap::new())
            .is_empty()
    );
}
