//! An open record and the external judgement round trip, held to what the reference implementation's session and model
//! layer do (its `FormSession`, `IFieldModel` and settlement origin tests): which attributes are answered after each event
//! — only the open ones whose evidence changed, none once every decided attribute has a value — how settlements travel
//! with the snapshot (a check's probability survives reopening, a correction clears it; restoring keeps the time, reverting
//! clears), and where an external judgement's values land. Memory follows the record's current values: a correction
//! leaves only itself, a live fill teaches nothing until it is entered, a restored saved record does not count twice, and
//! the live memories answer as memories rebuilt from the snapshot do. One attribute is answered alone while typed into,
//! and only while it is open.

use std::collections::BTreeMap;

use gil::decider::{Candidate, Decider, Evidence, ExternalCandidate, Source};
use gil::decision_record::DecisionRecord;
use gil::key_memory::{Key, KeyMemory};
use gil::lexical_memory::LexicalMemory;
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
    Schema::new(
        "ticket",
        vec![
            reporter,
            attribute("component", AttributeRole::Observed),
            attribute("summary", AttributeRole::Observed),
            attribute("team", AttributeRole::Decided),
        ],
    )
    .expect("a valid schema")
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
    assert_eq!(keyed[0].source, Source::KeyMemory);
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
            ("network", Source::KeyMemory)
        ]
    );
}

#[test]
fn a_judgement_offered_where_memory_had_evidence_changes_nothing_and_one_outside_the_domain_is_left_out()
 {
    let mut attributes = ticket().attributes().to_vec();
    attributes[3].candidates = Some(vec!["network".into(), "security".into()]);
    let schema = Schema::new("ticket", attributes).expect("a valid schema");
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
    Schema::new(
        "ticket",
        vec![attribute("summary", AttributeRole::Observed), team],
    )
    .expect("a valid schema")
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
    Schema::new(
        "paper",
        vec![
            attribute("area", AttributeRole::Observed),
            topics,
            attribute("lead", AttributeRole::Decided),
        ],
    )
    .expect("a valid schema")
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
fn the_checks_tell_beforehand_what_observe_and_settle_refuse() {
    let open = OpenRecord::new(paper(), "d4", 0);
    let refused = |r: Result<(), gil::open_record::ContractViolation>| r.unwrap_err().to_string();

    assert_eq!(
        refused(open.check_settle("topics", &Settlement::accept("nlp"))),
        "'topics' takes several values; settle it with Settlement::set"
    );
    assert_eq!(
        refused(open.check_settle("lead", &Settlement::set(["kim"]))),
        "'lead' takes one value; settle it with a single value"
    );
    assert!(
        open.check_settle("lead", &Settlement::accept("kim"))
            .is_ok()
    );
    assert!(open.check_settle("topics", &Settlement::reject()).is_ok());
    assert!(refused(open.check_observe("nowhere")).contains("has no attribute 'nowhere'"));
    assert!(
        refused(open.check_settle("nowhere", &Settlement::reject()))
            .contains("has no attribute 'nowhere'")
    );
    assert_eq!(
        refused(open.check_observe("lead")),
        "'lead' is decided; settle it instead"
    );
}

#[test]
fn an_attribute_that_is_off_is_never_answered() {
    let schema = settled_ticket();
    let mut attributes = schema.attributes().to_vec();
    attributes[1].policy = AttributePolicy::Off;
    let schema = Schema::new(schema.name(), attributes).expect("a valid schema");
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

#[test]
fn a_value_kept_or_changed_over_another_suggestion_is_remembered_and_moves_the_time() {
    let monday = 20_000 * DAY;
    let mut decider = Decider::new(KeyMemory::default(), None);
    let mut open = OpenRecord::new(settled_ticket(), "d1", monday - 3 * DAY);
    open.observe(&mut decider, "summary", Some("vpn drops"));

    open.settle(
        &mut decider,
        "team",
        &Settlement::accept_other("network"),
        monday,
    );
    let snapshot = open.snapshot();
    assert_eq!(snapshot.settlements["team"], SettlementKind::AcceptOther);
    assert_eq!(snapshot.settled_at, monday);
    // Unlike a fill, a person settled it: memory keeps the value.
    assert_eq!(snapshot.remembered().values["team"], "network");

    open.settle(
        &mut decider,
        "team",
        &Settlement::correct_other("security"),
        monday + DAY,
    );
    let snapshot = open.snapshot();
    assert_eq!(snapshot.settlements["team"], SettlementKind::CorrectOther);
    assert_eq!(snapshot.settled_at, monday + DAY);
    assert_eq!(snapshot.remembered().values["team"], "security");
}

// The reference session's own ticket: two decided attributes answered from memory, one of them resting on the component
// alone, and a third that is off.
fn intake() -> Schema {
    let mut reporter = attribute("reporter", AttributeRole::Observed);
    reporter.use_as_evidence = false;
    let mut severity = attribute("severity", AttributeRole::Decided);
    severity.depends_on = Some(vec!["component".into()]);
    severity.key_threshold = Some(0.5);
    let mut team = attribute("team", AttributeRole::Decided);
    team.memory_threshold = Some(0.3);
    team.key_threshold = Some(0.5);
    let mut assessment = attribute("assessment", AttributeRole::Decided);
    assessment.policy = AttributePolicy::Off;
    Schema::new(
        "ticket",
        vec![
            reporter,
            attribute("component", AttributeRole::Observed),
            attribute("summary", AttributeRole::Observed),
            severity,
            team,
            assessment,
        ],
    )
    .expect("a valid schema")
}

fn intake_history() -> Vec<Record> {
    vec![
        record(
            "d1",
            &[
                ("reporter", "Kim"),
                ("component", "vpn"),
                ("summary", "vpn drops every ten minutes"),
                ("severity", "high"),
                ("team", "network"),
            ],
        ),
        record(
            "d2",
            &[
                ("reporter", "Lee"),
                ("component", "printer"),
                ("summary", "printer on floor 3 is jammed"),
                ("severity", "low"),
                ("team", "facilities"),
            ],
        ),
        record(
            "d3",
            &[
                ("reporter", "Lee"),
                ("component", "printer"),
                ("summary", "toner is empty"),
                ("severity", "low"),
                ("team", "facilities"),
            ],
        ),
    ]
}

fn named(answers: &[gil::decider::Answer]) -> Vec<&str> {
    answers.iter().map(|a| a.attribute.as_str()).collect()
}

fn values_of(answer: &gil::decider::Answer) -> Vec<&str> {
    answer.candidates.iter().map(|c| c.value.as_str()).collect()
}

fn known(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(a, v)| ((*a).to_owned(), (*v).to_owned()))
        .collect()
}

/// What a record that is not open would be offered for `attribute` given `pairs` — how a test reads what memory holds.
fn probe(
    decider: &Decider,
    schema: &Schema,
    attribute: &str,
    pairs: &[(&str, &str)],
) -> gil::decider::Answer {
    decider.answer_one(
        schema,
        "probe",
        attribute,
        &known(pairs),
        &BTreeMap::new(),
        None,
    )
}

#[test]
fn answers_come_from_values_settled_alongside_the_known_ones_without_an_external_judgement() {
    let schema = intake();
    let mut decider = Decider::new(KeyMemory::default(), None);
    decider.rebuild(&schema, &intake_history());
    let mut open = OpenRecord::new(schema, "d4", 4 * MINUTE);

    let after = open.observe(&mut decider, "component", Some("printer"));

    // severity rests on the component and team declares nothing, so both are answered afresh; assessment is off.
    assert_eq!(named(&after), ["severity", "team"]);
    let severity = &after[0];
    assert_eq!(
        (severity.source, severity.policy, &severity.needs_external),
        (Source::KeyMemory, AttributePolicy::Suggest, &None)
    );
    let best = &severity.candidates[0];
    assert_eq!(
        (best.value.as_str(), best.source, best.trusted),
        ("low", Source::KeyMemory, true)
    );
    assert!((best.score - (1.0 + 0.95) / (1.0 + 0.95 + 1.0)).abs() < 1e-12);
    assert_eq!(
        best.evidence,
        Some(Evidence::Key(Key {
            attribute: "component".into(),
            value: "printer".into(),
        }))
    );
    assert_eq!(named(&open.answer(&decider)), ["severity", "team"]);
}

#[test]
fn only_open_attributes_whose_evidence_changed_are_answered_again() {
    let schema = intake();
    let mut decider = Decider::new(KeyMemory::default(), None);
    decider.rebuild(&schema, &intake_history());
    let mut open = OpenRecord::new(schema, "d4", 4 * MINUTE);

    // The summary is not something severity rests on; the reporter is evidence for nothing.
    assert_eq!(
        named(&open.observe(&mut decider, "summary", Some("the vpn is slow"))),
        ["team"]
    );
    assert!(
        open.observe(&mut decider, "reporter", Some("Kim"))
            .is_empty()
    );

    // Settling severity changes team's evidence; severity itself is no longer open.
    assert_eq!(
        named(&open.settle(&mut decider, "severity", &Settlement::accept("high"), DAY)),
        ["team"]
    );

    // A rejected attribute is not answered again, but reverting reopens it.
    assert!(
        open.settle(&mut decider, "team", &Settlement::reject(), DAY)
            .is_empty()
    );
    assert!(open.answer(&decider).is_empty());
    assert_eq!(
        named(&open.settle(&mut decider, "team", &Settlement::revert(), DAY)),
        ["team"]
    );
}

#[test]
fn correcting_an_accepted_value_leaves_only_the_correction_in_memory() {
    let schema = intake();
    let mut decider = Decider::new(KeyMemory::default(), None);
    let mut open = OpenRecord::new(schema.clone(), "d1", 0);
    open.observe(&mut decider, "component", Some("vpn"));

    open.settle(&mut decider, "severity", &Settlement::accept("low"), MINUTE);
    open.settle(
        &mut decider,
        "severity",
        &Settlement::correct("high"),
        2 * MINUTE,
    );

    // One settlement of "high" under the key, and nothing of "low": a single value scores 1 / (1 + 1).
    let after = probe(&decider, &schema, "severity", &[("component", "vpn")]);
    assert_eq!(
        after
            .candidates
            .iter()
            .map(|c| (c.value.as_str(), c.score))
            .collect::<Vec<_>>(),
        [("high", 0.5)]
    );
}

/// An answer's candidates as compared across two memories: the similarity of a similar record follows the lexical
/// memory's own weighting schedule, which counts how its rows arrived, so only the record and the value are compared.
fn comparable(answer: &gil::decider::Answer) -> Vec<(String, Source, String, Option<u64>)> {
    answer
        .candidates
        .iter()
        .map(|c| match &c.evidence {
            Some(Evidence::Record { id, .. }) => (c.value.clone(), c.source, id.clone(), None),
            other => (
                c.value.clone(),
                c.source,
                other.as_ref().map(ToString::to_string).unwrap_or_default(),
                Some(c.score.to_bits()),
            ),
        })
        .collect()
}

#[test]
fn an_open_record_reaches_the_state_a_rebuild_from_its_snapshot_reaches() {
    let schema = intake();
    let mut live = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    let mut open = OpenRecord::new(schema.clone(), "d1", 0);
    open.observe(&mut live, "summary", Some("vpn drops"));
    open.settle(&mut live, "team", &Settlement::accept("facilities"), MINUTE);
    open.observe(&mut live, "component", Some("vpn"));
    open.observe(&mut live, "summary", Some("vpn drops every ten minutes"));
    open.settle(
        &mut live,
        "team",
        &Settlement::correct("network"),
        2 * MINUTE,
    );
    open.settle(
        &mut live,
        "severity",
        &Settlement::accept("high"),
        3 * MINUTE,
    );

    let snapshot = open.snapshot();
    let mut rebuilt = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    rebuilt.rebuild(&schema, std::slice::from_ref(&snapshot));

    assert_eq!(
        snapshot.values,
        known(&[
            ("summary", "vpn drops every ten minutes"),
            ("component", "vpn"),
            ("team", "network"),
            ("severity", "high"),
        ])
    );
    // A changed value moves to the end.
    assert_eq!(
        snapshot.arrival,
        Some(vec![
            "component".into(),
            "summary".into(),
            "team".into(),
            "severity".into()
        ])
    );

    for pairs in [
        &[("component", "vpn"), ("summary", "vpn drops")][..],
        &[("summary", "vpn drops")][..],
    ] {
        let values = known(pairs);
        let ask = |decider: &Decider| decider.answer(&schema, "q", &values, &BTreeMap::new());
        let (from_live, from_rebuilt) = (ask(&live), ask(&rebuilt));
        assert_eq!(named(&from_live), named(&from_rebuilt));
        for (l, r) in from_live.iter().zip(&from_rebuilt) {
            assert_eq!(comparable(l), comparable(r), "{pairs:?}");
        }
    }

    // With no key to go on, the similar record is the snapshot, with its corrected value.
    let team = probe(&live, &schema, "team", &[("summary", "vpn drops")]);
    assert_eq!(team.source, Source::SimilarRecord);
    assert_eq!(team.candidates[0].value, "network");
    assert!(
        matches!(&team.candidates[0].evidence, Some(Evidence::Record { id, .. }) if id == "d1")
    );
}

#[test]
fn restoring_a_saved_record_after_a_rebuild_does_not_count_it_twice() {
    let schema = intake();
    let history = intake_history();
    let mut decider = Decider::new(KeyMemory::default(), None);
    decider.rebuild(&schema, &history);
    let printer = [("component", "printer")];
    let before = [
        probe(&decider, &schema, "team", &printer),
        probe(&decider, &schema, "severity", &printer),
    ];

    let saved = &history[1];
    let mut open = OpenRecord::new(schema.clone(), &saved.id, saved.settled_at);
    for (name, value) in &saved.values {
        if schema.attribute(name).role == AttributeRole::Observed {
            open.observe(&mut decider, name, Some(value));
        } else {
            open.settle(&mut decider, name, &Settlement::restore(value), DAY);
        }
    }

    assert_eq!(open.snapshot().settled_at, saved.settled_at);
    assert_eq!(
        [
            probe(&decider, &schema, "team", &printer),
            probe(&decider, &schema, "severity", &printer),
        ],
        before
    );
}

#[test]
fn a_similar_record_outranks_a_guess_but_not_a_value_under_a_strong_enough_key() {
    let schema = intake();
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    decider.rebuild(&schema, &intake_history());
    let mut open = OpenRecord::new(schema, "d4", 4 * MINUTE);

    // No key matches: the similar record leads, the overall frequency follows.
    let guessed = open.observe(
        &mut decider,
        "summary",
        Some("printer on floor 2 is jammed"),
    );
    assert_eq!(named(&guessed), ["team"]);
    let team = &guessed[0];
    assert_eq!(team.source, Source::SimilarRecord);
    assert_eq!(
        team.candidates
            .iter()
            .map(|c| (
                c.value.as_str(),
                c.source,
                c.evidence.as_ref().map(ToString::to_string)
            ))
            .collect::<Vec<_>>(),
        [
            ("facilities", Source::SimilarRecord, Some("d2".to_owned())),
            ("network", Source::KeyMemory, None),
        ]
    );

    // Each answer's decision record names the similar record it rests on and the value answered.
    let mut records: Vec<DecisionRecord> = Vec::new();
    open.answer_with(&decider, &mut |r| records.push(r.clone()));
    let team_record = records.iter().find(|r| r.attribute == "team").unwrap();
    assert_eq!(team_record.record, "d4");
    assert_eq!(team_record.similar.as_ref().unwrap().record, "d2");
    let answered = team_record.answered.as_ref().unwrap();
    assert_eq!(
        (answered.layer, answered.value.as_str()),
        (Source::SimilarRecord, "facilities")
    );

    // Once a key strong enough matches, its values lead.
    let keyed = open.observe(&mut decider, "component", Some("vpn"));
    let team = keyed.iter().find(|a| a.attribute == "team").unwrap();
    assert_eq!(
        team.candidates[0],
        Candidate {
            value: "network".into(),
            score: 0.5,
            source: Source::KeyMemory,
            evidence: Some(Evidence::Key(Key {
                attribute: "component".into(),
                value: "vpn".into(),
            })),
            trusted: true,
        }
    );
}

#[test]
fn with_nothing_settled_a_person_decides() {
    let schema = intake();
    let decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    let open = OpenRecord::new(schema, "d1", 0);

    let mut records: Vec<DecisionRecord> = Vec::new();
    let answers = open.answer_with(&decider, &mut |r| records.push(r.clone()));

    assert_eq!(named(&answers), ["severity", "team"]);
    assert!(
        answers
            .iter()
            .all(|a| a.source == Source::None && a.candidates.is_empty() && a.coarse.is_none())
    );
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .all(|r| r.answered.is_none() && r.key.is_none() && r.similar.is_none())
    );
}

#[test]
fn observed_and_decided_attributes_take_their_own_events() {
    let open = OpenRecord::new(intake(), "d1", 0);
    let refused = |r: Result<(), gil::open_record::ContractViolation>| r.unwrap_err().to_string();

    assert_eq!(
        refused(open.check_observe("team")),
        "'team' is decided; settle it instead"
    );
    assert_eq!(
        refused(open.check_settle("component", &Settlement::accept("vpn"))),
        "'component' is observed; observe it instead"
    );
    assert!(
        refused(open.check_settle("owner", &Settlement::accept("x")))
            .contains("has no attribute 'owner'")
    );
}

#[test]
#[should_panic(expected = "'component' is observed; observe it instead")]
fn settling_an_observed_attribute_is_refused() {
    let mut decider = Decider::new(KeyMemory::default(), None);
    OpenRecord::new(intake(), "d1", 0).settle(
        &mut decider,
        "component",
        &Settlement::accept("vpn"),
        1,
    );
}

#[test]
#[should_panic(expected = "has no attribute 'owner'")]
fn observing_an_unknown_attribute_is_refused() {
    let mut decider = Decider::new(KeyMemory::default(), None);
    OpenRecord::new(intake(), "d1", 0).observe(&mut decider, "owner", Some("x"));
}

#[test]
fn no_attribute_is_answered_once_every_decided_one_has_a_value() {
    let schema = intake();
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    decider.rebuild(&schema, &intake_history());
    let mut open = OpenRecord::new(schema.clone(), "d4", 0);
    open.observe(&mut decider, "component", Some("vpn"));
    open.settle(&mut decider, "severity", &Settlement::accept("high"), DAY);
    assert!(
        open.settle(&mut decider, "team", &Settlement::enter("network"), DAY)
            .is_empty()
    );
    assert!(open.answer(&decider).is_empty());

    let all = known(&[
        ("component", "badge"),
        ("severity", "low"),
        ("team", "network"),
    ]);
    assert!(
        decider
            .answer(&schema, "d5", &all, &BTreeMap::new())
            .is_empty()
    );
}

#[test]
fn a_checked_value_keeps_its_probability_when_reopened_and_a_correction_clears_it() {
    let mut decider = Decider::new(KeyMemory::default(), None);
    let mut open = OpenRecord::new(settled_ticket(), "d1", 0);
    open.observe(&mut decider, "summary", Some("vpn drops"));
    open.settle(
        &mut decider,
        "team",
        &Settlement::check("network", 0.25),
        DAY,
    );

    let saved = open.snapshot();
    assert_eq!(saved.settlements["team"], SettlementKind::Check);
    let mut reopened = OpenRecord::reopen(settled_ticket(), &saved);
    assert_eq!(reopened.snapshot().probabilities, saved.probabilities);
    assert_eq!(reopened.snapshot().probabilities["team"], 0.25);

    reopened.settle(
        &mut decider,
        "team",
        &Settlement::correct("security"),
        2 * DAY,
    );
    assert_eq!(
        reopened.snapshot().settlements["team"],
        SettlementKind::Correct
    );
    assert!(reopened.snapshot().probabilities.is_empty());

    // A probability saved beside a settlement that is not a check is not brought back.
    let mismatched = Record {
        settlements: BTreeMap::from([("team".to_owned(), SettlementKind::Accept)]),
        ..saved
    };
    assert!(
        OpenRecord::reopen(settled_ticket(), &mismatched)
            .snapshot()
            .probabilities
            .is_empty()
    );
}

#[test]
fn entering_then_reverting_forgets_how_the_attribute_was_settled() {
    let mut decider = Decider::new(KeyMemory::default(), None);
    let mut open = OpenRecord::new(settled_ticket(), "d1", 0);
    open.observe(&mut decider, "summary", Some("vpn drops"));
    open.settle(&mut decider, "team", &Settlement::enter("network"), DAY);
    open.settle(&mut decider, "team", &Settlement::revert(), 2 * DAY);
    let snapshot = open.snapshot();
    assert!(snapshot.settlements.is_empty());
    assert!(!snapshot.values.contains_key("team"));
    assert_eq!(snapshot.settled_at, DAY); // reverting does not move the time
}

#[test]
fn a_value_filled_live_stays_in_the_snapshot_but_does_not_teach_the_decider() {
    let schema = settled_ticket();
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    let mut filled = OpenRecord::new(schema.clone(), "d1", 0);
    filled.observe(&mut decider, "summary", Some("vpn drops"));
    filled.settle(&mut decider, "team", &Settlement::fill("network"), MINUTE);
    assert_eq!(filled.snapshot().values["team"], "network");

    let mut next = OpenRecord::new(schema.clone(), "d2", 0);
    let answers = next.observe(&mut decider, "summary", Some("vpn drops"));
    assert!(answers[0].candidates.is_empty()); // nothing a person settled to go on

    let mut accepted = OpenRecord::new(schema.clone(), "d3", 0);
    accepted.observe(&mut decider, "summary", Some("vpn drops"));
    accepted.settle(
        &mut decider,
        "team",
        &Settlement::accept("network"),
        2 * MINUTE,
    );
    let mut after = OpenRecord::new(schema, "d4", 0);
    let answers = after.observe(&mut decider, "summary", Some("vpn drops"));
    assert!(values_of(&answers[0]).contains(&"network"));
}

#[test]
fn a_filled_value_settled_again_as_entered_is_learned_in_its_place() {
    // A host that fills an attribute itself can have the value checked later with no answer in view, and settle the same
    // attribute of the same record again with what the check found.
    let schema = settled_ticket();
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    let mut open = OpenRecord::new(schema.clone(), "d1", 0);
    open.observe(&mut decider, "summary", Some("vpn drops"));
    open.settle(
        &mut decider,
        "team",
        &Settlement::fill("facilities"),
        MINUTE,
    );
    open.settle(
        &mut decider,
        "team",
        &Settlement::enter("network"),
        2 * MINUTE,
    );
    assert_eq!(open.snapshot().settlements["team"], SettlementKind::Enter);

    let mut next = OpenRecord::new(schema, "d2", 0);
    let answers = next.observe(&mut decider, "summary", Some("vpn drops"));
    let offered = values_of(&answers[0]);
    assert!(offered.contains(&"network"));
    assert!(!offered.contains(&"facilities")); // the checked value replaced the filled one
}

#[test]
fn a_value_kept_or_changed_over_another_suggestion_teaches_the_decider() {
    let schema = settled_ticket();
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    let mut kept = OpenRecord::new(schema.clone(), "d1", 0);
    kept.observe(&mut decider, "summary", Some("vpn drops"));
    kept.settle(
        &mut decider,
        "team",
        &Settlement::accept_other("network"),
        MINUTE,
    );
    let mut changed = OpenRecord::new(schema.clone(), "d2", 0);
    changed.observe(&mut decider, "summary", Some("printer jams"));
    changed.settle(
        &mut decider,
        "team",
        &Settlement::correct_other("facilities"),
        2 * MINUTE,
    );

    let mut next = OpenRecord::new(schema, "d3", 0);
    let answers = next.observe(&mut decider, "summary", Some("vpn drops"));
    assert!(values_of(&answers[0]).contains(&"network")); // learned, unlike a fill
}

#[test]
fn a_set_is_settled_as_a_set_and_its_restore_keeps_the_time_while_a_revert_clears_it() {
    let mut decider = Decider::new(KeyMemory::new(60, 1.0), None);
    let mut open = OpenRecord::new(paper(), "d4", 0);
    open.observe(&mut decider, "area", Some("ai"));
    open.settle(
        &mut decider,
        "topics",
        &Settlement::set(["vision", "nlp"]),
        DAY,
    );
    assert_eq!(open.snapshot().settlements["topics"], SettlementKind::Set);
    assert_eq!(open.snapshot().settled_at, DAY);

    // Putting saved values back keeps the saved time and is not known to have been settled any way.
    open.settle(
        &mut decider,
        "topics",
        &Settlement::restore_set(["genomics"]),
        2 * DAY,
    );
    let restored = open.snapshot();
    assert_eq!(restored.sets["topics"], ["genomics"]);
    assert_eq!(restored.settled_at, DAY);
    assert!(restored.settlements.is_empty());

    let reverted = open.settle(&mut decider, "topics", &Settlement::revert(), 3 * DAY);
    assert!(named(&reverted).contains(&"topics")); // open again, and answered afresh
    let snapshot = open.snapshot();
    assert!(!snapshot.sets.contains_key("topics"));
    assert_eq!(snapshot.arrival, Some(vec!["area".into()]));
    assert_eq!(snapshot.settled_at, DAY);
}

#[test]
fn an_external_judgement_offered_for_a_set_leaves_out_the_values_already_chosen() {
    let mut decider = Decider::new(KeyMemory::default(), None);
    let mut open = OpenRecord::new(paper(), "d1", 0);
    let after = open.settle(&mut decider, "topics", &Settlement::set(["nlp"]), MINUTE);
    assert!(
        after
            .iter()
            .any(|a| a.attribute == "topics" && a.needs_external.is_some())
    );

    let ranked = ["nlp", "robotics", "vision"].map(|value| ExternalCandidate {
        value: value.to_owned(),
        probability: 0.9,
        trusted: true,
    });
    let offered = open.offer_external(&decider, "topics", &ranked);
    assert_eq!(values_of(&offered), ["robotics", "vision"]);
    assert_eq!(offered.source, Source::External);
}

fn typed_ticket() -> Schema {
    let mut team = attribute("team", AttributeRole::Decided);
    team.key_threshold = Some(0.9);
    team.typed_key_thresholds = Some(vec![Some(0.25)]);
    team.depends_on = Some(vec!["channel".into()]);
    Schema::new(
        "ticket",
        vec![
            attribute("channel", AttributeRole::Observed),
            attribute("summary", AttributeRole::Observed),
            team,
        ],
    )
    .expect("a valid schema")
}

fn phone_history() -> Vec<Record> {
    // Under channel=phone: "network" three times, "billing" twice, "backend" once.
    [
        "network", "network", "billing", "network", "billing", "backend",
    ]
    .iter()
    .enumerate()
    .map(|(i, team)| {
        let id = format!("d{}", i + 1);
        let summary = format!("ticket {}", i + 1);
        record(
            &id,
            &[("channel", "phone"), ("summary", &summary), ("team", team)],
        )
    })
    .collect()
}

#[test]
fn an_open_record_answers_with_typed_text_and_keeps_the_attribute_open() {
    let schema = typed_ticket();
    let mut decider = Decider::new(KeyMemory::default(), None);
    decider.rebuild(&schema, &phone_history());
    let mut open = OpenRecord::new(schema, "d9", 9 * MINUTE);
    open.observe(&mut decider, "channel", Some("phone"));

    let typed = open.answer_partial(&decider, "team", Some("b"));
    let untyped = open.answer_partial(&decider, "team", None);
    let after = open.answer(&decider);

    assert_eq!(typed.candidates[0].value, "billing");
    assert!(typed.answered());
    assert!(typed.candidates.iter().all(|c| c.value.starts_with('b')));
    assert_eq!(typed.needs_external, None); // no external judgement while a person types
    assert_eq!(untyped.candidates[0].value, "network");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0], untyped); // one attribute alone is answered as the whole record would answer it
    assert!(!open.snapshot().values.contains_key("team"));
}

#[test]
#[should_panic(expected = "'team' is not open")]
fn text_is_not_typed_into_an_attribute_already_settled() {
    let schema = typed_ticket();
    let mut decider = Decider::new(KeyMemory::default(), None);
    decider.rebuild(&schema, &phone_history());
    let mut open = OpenRecord::new(schema, "d9", 9 * MINUTE);
    open.observe(&mut decider, "channel", Some("phone"));
    open.settle(&mut decider, "team", &Settlement::accept("network"), DAY);
    open.answer_partial(&decider, "team", Some("n"));
}

#[test]
#[should_panic(expected = "'team' is not open")]
fn a_rejected_attribute_is_not_asked_about_alone() {
    let mut decider = Decider::new(KeyMemory::default(), None);
    let mut open = OpenRecord::new(typed_ticket(), "d9", 0);
    open.settle(&mut decider, "team", &Settlement::reject(), DAY);
    open.answer_partial(&decider, "team", None);
}

#[test]
#[should_panic(expected = "'channel' is not open")]
fn an_observed_attribute_is_not_asked_about() {
    let decider = Decider::new(KeyMemory::default(), None);
    OpenRecord::new(typed_ticket(), "d9", 0).answer_partial(&decider, "channel", Some("ph"));
}
