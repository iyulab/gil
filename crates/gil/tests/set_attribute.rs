//! An attribute that takes several values, held to what the reference implementation's set-valued field, selection and
//! session tests expect: each value of a set is settled under the record's keys and under the set's other values, the
//! order a set is listed in carries no weight and does not change a replay, a record put again replaces every value it
//! settled and a record left out takes all of them along, each value of a set keys other attributes, and a single-valued
//! attribute is replayed with the values of a set as its keys. A set is settled only as a set, is never looked up in the
//! similar record memory, and its chosen values are evidence an external judgement reads line by line.

use std::collections::BTreeMap;

use gil::decider::{Decider, ExternalCandidate};
use gil::decision_record::DecisionRecord;
use gil::key_memory::{Candidate, KeyMemory, Lookup};
use gil::lexical_memory::LexicalMemory;
use gil::open_record::OpenRecord;
use gil::replay::{Choice, ThresholdReplay, select_key_threshold};
use gil::schema::{Attribute, AttributeRole, Record, Schema, Settlement};

const MINUTE: i64 = 60 * 10_000_000;

/// An observed area, decided topics that take several values, and a decided lead.
fn paper() -> Schema {
    paper_with(None)
}

fn paper_with(topics_key_threshold: Option<f64>) -> Schema {
    let mut topics = Attribute::new("topics", AttributeRole::Decided);
    topics.multiple = true;
    topics.key_threshold = topics_key_threshold;
    Schema {
        name: "paper".into(),
        attributes: vec![
            Attribute::new("area", AttributeRole::Observed),
            topics,
            Attribute::new("lead", AttributeRole::Decided),
        ],
    }
}

/// Record `d<n>`, settled `n` minutes after the epoch: records settle in the order of their numbers, a minute apart.
fn doc(number: i64, area: Option<&str>, topics: &[&str], lead: Option<&str>) -> Record {
    let mut values = BTreeMap::new();
    if let Some(area) = area {
        values.insert("area".to_owned(), area.to_owned());
    }
    if let Some(lead) = lead {
        values.insert("lead".to_owned(), lead.to_owned());
    }
    Record {
        id: format!("d{number}"),
        values,
        sets: chosen(topics),
        settled_at: number * MINUTE,
        ..Record::default()
    }
}

fn known(area: &str) -> BTreeMap<String, String> {
    BTreeMap::from([("area".to_owned(), area.to_owned())])
}

fn chosen(topics: &[&str]) -> BTreeMap<String, Vec<String>> {
    BTreeMap::from([(
        "topics".to_owned(),
        topics.iter().map(|t| (*t).to_owned()).collect(),
    )])
}

fn three_papers(schema: &Schema) -> KeyMemory {
    let mut memory = KeyMemory::new(60, 1.0);
    memory.put(schema, &doc(1, Some("ai"), &["nlp", "vision"], None));
    memory.put(schema, &doc(2, Some("ai"), &["nlp"], None));
    memory.put(schema, &doc(3, Some("bio"), &["genomics", "nlp"], None));
    memory
}

fn scored(candidates: &[Candidate]) -> Vec<(&str, f64, Option<String>)> {
    candidates
        .iter()
        .map(|c| {
            (
                c.value.as_str(),
                c.score,
                c.evidence.as_ref().map(ToString::to_string),
            )
        })
        .collect()
}

fn ranked_values(candidates: &[Candidate]) -> Vec<&str> {
    candidates.iter().map(|c| c.value.as_str()).collect()
}

fn trusted(candidates: &[Candidate]) -> Vec<(&str, bool)> {
    candidates
        .iter()
        .map(|c| (c.value.as_str(), c.trusted))
        .collect()
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-12,
        "{actual} is not {expected}"
    );
}

#[test]
fn each_value_of_a_set_is_settled_under_the_record_keys() {
    let schema = paper();
    let memory = three_papers(&schema);
    let area = known("ai");

    // Under area = ai: nlp twice and vision once of three settled values — strengths 2 / 4 and 1 / 4. Genomics was never
    // settled under it and follows as the attribute's other values do, by its share overall (1 of 5).
    assert_eq!(
        scored(&memory.rank(&Lookup::new(&schema, "topics", &area), 3)),
        [
            ("nlp", 0.5, Some("area: ai".to_owned())),
            ("vision", 0.25, Some("area: ai".to_owned())),
            ("genomics", 0.2, None),
        ]
    );
}

#[test]
fn values_already_chosen_are_evidence_for_the_rest_and_are_not_offered_again() {
    let schema = paper();
    let memory = three_papers(&schema);
    let (area, sets) = (known("ai"), chosen(&["nlp"]));

    // With nlp chosen it is a key too: settled alongside nlp were vision (d1) and genomics (d3), a third each. Vision adds
    // its quarter under area = ai and rests most on nlp.
    let ranked = memory.rank(&Lookup::new(&schema, "topics", &area).sets(&sets), 3);

    assert_eq!(ranked_values(&ranked), ["vision", "genomics"]);
    assert_close(ranked[0].score, 0.25 + 1.0 / 3.0);
    assert_eq!(
        ranked[0].evidence.as_ref().map(ToString::to_string),
        Some("topics: nlp".to_owned())
    );
    assert_close(ranked[1].score, 1.0 / 3.0);
}

#[test]
fn each_offered_value_of_a_set_is_trusted_on_its_own_score() {
    let schema = paper_with(Some(0.3));
    let mut memory = KeyMemory::new(60, 1.0);
    memory.put(&schema, &doc(1, Some("ai"), &["nlp", "vision"], None));
    memory.put(&schema, &doc(2, Some("ai"), &["nlp", "vision"], None));
    memory.put(&schema, &doc(3, Some("ai"), &["nlp", "robotics"], None));
    let (area, sets) = (known("ai"), chosen(&["nlp"]));

    // Under area = ai: nlp 3, vision 2, robotics 1 of 6 — 3/7, 2/7 and 1/7. Two reach 0.3; a single-valued attribute would
    // have trusted every keyed value on the first one's score.
    assert_eq!(
        trusted(&memory.rank(&Lookup::new(&schema, "topics", &area), 3)),
        [("nlp", true), ("vision", false), ("robotics", false)]
    );
    assert_eq!(
        trusted(&memory.rank(&Lookup::new(&schema, "topics", &area).sets(&sets), 3)),
        [("vision", true), ("robotics", true)]
    );
}

#[test]
fn the_values_of_one_record_weigh_alike_whatever_order_they_are_listed_in() {
    let schema = paper();
    let area = known("ai");
    for listed in [["b", "a"], ["a", "b"]] {
        let mut memory = KeyMemory::new(60, 0.5);
        memory.put(&schema, &doc(1, Some("ai"), &listed, None));
        memory.put(&schema, &doc(2, Some("ai"), &["c"], None));

        // One settlement after d1 halves both of its values alike: a = b = 0.5, c = 1, total 2 — strengths over 3.
        let ranked = memory.rank(&Lookup::new(&schema, "topics", &area), 3);
        assert_eq!(ranked_values(&ranked), ["c", "a", "b"], "listed {listed:?}");
        for (candidate, expected) in ranked.iter().zip([1.0 / 3.0, 1.0 / 6.0, 1.0 / 6.0]) {
            assert_close(candidate.score, expected);
        }
    }
}

#[test]
fn each_value_of_a_set_is_a_key_of_its_own_for_another_attribute() {
    let schema = paper();
    let mut memory = KeyMemory::new(60, 1.0);
    memory.put(&schema, &doc(1, None, &["nlp", "vision"], Some("kim")));
    memory.put(&schema, &doc(2, None, &["nlp"], Some("lee")));
    let none = BTreeMap::new();

    let vision = chosen(&["vision"]);
    assert_eq!(
        scored(&memory.rank(&Lookup::new(&schema, "lead", &none).sets(&vision), 2)),
        [
            ("kim", 0.5, Some("topics: vision".to_owned())),
            ("lee", 0.5, None),
        ]
    );
    // nlp backs both alike, vision only kim.
    let both = chosen(&["nlp", "vision"]);
    assert_eq!(
        ranked_values(&memory.rank(&Lookup::new(&schema, "lead", &none).sets(&both), 2)),
        ["kim", "lee"]
    );
}

#[test]
fn a_record_put_again_replaces_every_value_it_settled_and_is_left_out_whole() {
    let schema = paper();
    let mut memory = three_papers(&schema);
    memory.put(&schema, &doc(1, Some("ai"), &["vision"], None));
    let area = known("ai");

    // Under area = ai now: vision (d1) and nlp (d2), a third each, the later first.
    assert_eq!(
        ranked_values(&memory.rank(&Lookup::new(&schema, "topics", &area), 2)),
        ["nlp", "vision"]
    );

    // Leaving d1 out leaves out all it settled: only d2's nlp under the key, and overall nlp and genomics from d2 and d3.
    let ranked = memory.rank(
        &Lookup::new(&schema, "topics", &area).excluding(Some("d1")),
        3,
    );
    assert_eq!(ranked_values(&ranked), ["nlp", "genomics"]);
    assert_close(ranked[0].score, 0.5);
    assert_close(ranked[1].score, 1.0 / 3.0);
}

const CHOICE: Choice = Choice {
    target_precision: 0.9,
    minimum_answered: 5,
    counting: None,
};

/// Areas alternate: ai papers cover nlp and vision, bio papers genomics.
fn alternating(reversed: bool) -> Vec<Record> {
    (1..=20)
        .map(|n| {
            if n % 2 == 0 {
                let topics = if reversed {
                    ["vision", "nlp"]
                } else {
                    ["nlp", "vision"]
                };
                doc(n, Some("ai"), &topics, None)
            } else {
                doc(n, Some("bio"), &["genomics"], None)
            }
        })
        .collect()
}

fn replay(attribute: &str, records: &[Record]) -> ThresholdReplay {
    select_key_threshold(
        &mut KeyMemory::default(),
        &paper(),
        attribute,
        records,
        CHOICE,
    )
}

#[test]
fn a_set_is_replayed_value_by_value_and_counts_the_values_sought() {
    let replayed = replay("topics", &alternating(false));

    // Every record after the first is asked about: ten ai papers, nine bio ones after d1. An ai paper is asked with nothing
    // chosen (two values sought) and after one is chosen (one more), a bio paper once (one).
    assert_eq!(replayed.lookups, 10 * 3 + 9);
    // Under these keys only the settled values were ever offered.
    assert_eq!(replayed.chosen.expect("a threshold").precision, 1.0);
    assert!(replayed.keys.is_none());
}

#[test]
fn the_order_a_set_is_listed_in_does_not_change_the_replay() {
    assert_eq!(
        replay("topics", &alternating(true)),
        replay("topics", &alternating(false))
    );
}

#[test]
fn a_single_valued_attribute_is_replayed_with_the_values_of_a_set_as_its_keys() {
    // Only the topics tell lead apart; the schema's order puts topics before lead, so the replay asks with them.
    let records: Vec<Record> = (1..=20)
        .map(|n| {
            if n % 2 == 0 {
                doc(n, None, &["nlp"], Some("kim"))
            } else {
                doc(n, None, &["genomics"], Some("park"))
            }
        })
        .collect();

    let replayed = replay("lead", &records);

    // Of 19 lookups, all but d2 found a value under its topic — nlp was new there.
    assert_eq!(replayed.lookups, 19);
    assert_eq!(replayed.candidates, 18);
    assert_eq!(replayed.chosen.expect("a threshold").precision, 1.0);
}

#[test]
fn the_checks_tell_that_a_set_is_neither_entered_nor_filled_with_one_value() {
    let open = OpenRecord::new(paper(), "d4", 0);
    for settlement in [Settlement::enter("nlp"), Settlement::fill("nlp")] {
        assert_eq!(
            open.check_settle("topics", &settlement)
                .unwrap_err()
                .to_string(),
            "'topics' takes several values; settle it with Settlement::set"
        );
    }
}

#[test]
#[should_panic(expected = "'topics' takes several values")]
fn a_set_is_not_entered_with_one_value() {
    let mut decider = Decider::new(KeyMemory::default(), None);
    OpenRecord::new(paper(), "d4", 0).settle(&mut decider, "topics", &Settlement::enter("nlp"), 1);
}

#[test]
#[should_panic(expected = "'topics' takes several values")]
fn a_set_is_not_filled_with_one_value() {
    let mut decider = Decider::new(KeyMemory::default(), None);
    OpenRecord::new(paper(), "d4", 0).settle(&mut decider, "topics", &Settlement::fill("nlp"), 1);
}

fn history() -> Vec<Record> {
    vec![
        doc(1, Some("ai"), &["nlp", "vision"], Some("kim")),
        doc(2, Some("ai"), &["nlp"], Some("lee")),
        doc(3, Some("bio"), &["genomics", "nlp"], Some("park")),
    ]
}

#[test]
fn a_stateless_answer_takes_the_values_chosen_so_far() {
    let schema = paper_with(Some(0.3));
    let mut decider = Decider::new(KeyMemory::new(60, 1.0), None);
    decider.rebuild(&schema, &history());
    let values = BTreeMap::from([
        ("area".to_owned(), "ai".to_owned()),
        ("lead".to_owned(), "kim".to_owned()),
    ]);

    let answers = decider.answer(&schema, "d4", &values, &chosen(&["nlp"]));

    // Lead has a value; the set is open with one chosen.
    assert_eq!(
        answers
            .iter()
            .map(|a| a.attribute.as_str())
            .collect::<Vec<_>>(),
        ["topics"]
    );
    let offered: Vec<&str> = answers[0]
        .candidates
        .iter()
        .map(|c| c.value.as_str())
        .collect();
    assert!(!offered.contains(&"nlp"));
    assert_eq!(offered.first(), Some(&"vision"));
}

#[test]
fn an_external_judgement_reads_each_chosen_value_as_a_line_and_is_not_offered_them_again() {
    let schema = paper_with(Some(0.3));
    let mut decider = Decider::new(KeyMemory::default(), None);
    let mut open = OpenRecord::new(schema.clone(), "d1", 0);

    let after = open.settle(
        &mut decider,
        "topics",
        &Settlement::set(["vision", "nlp"]),
        MINUTE,
    );
    let topics = after
        .iter()
        .find(|a| a.attribute == "topics")
        .expect("the set stays open");

    assert_eq!(
        topics.needs_external,
        Some(vec![
            ("topics".to_owned(), "nlp".to_owned()),
            ("topics".to_owned(), "vision".to_owned()),
        ])
    );

    let snapshot = open.snapshot();
    let q = Lookup::new(&schema, "topics", &snapshot.values)
        .sets(&snapshot.sets)
        .excluding(Some("d1"));
    let ranked = ["nlp", "robotics"].map(|value| ExternalCandidate {
        value: value.to_owned(),
        probability: 0.9,
        trusted: true,
    });
    assert_eq!(
        decider
            .offer_external(&q, &ranked)
            .candidates
            .iter()
            .map(|c| c.value.as_str())
            .collect::<Vec<_>>(),
        ["robotics"]
    );
}

#[test]
fn a_set_is_not_looked_up_in_the_similar_record_memory_and_its_decision_record_answers_a_trusted_value()
 {
    let schema = paper_with(Some(0.3));
    let mut decider = Decider::new(KeyMemory::new(60, 1.0), Some(LexicalMemory::default()));
    decider.rebuild(&schema, &history());

    let mut looked: Vec<DecisionRecord> = Vec::new();
    decider.answer_with(&schema, "d4", &known("ai"), &BTreeMap::new(), &mut |r| {
        looked.push(r.clone())
    });
    let record_of = |looked: &[DecisionRecord], attribute: &str| {
        looked
            .iter()
            .find(|r| r.attribute == attribute)
            .cloned()
            .expect("a decision record")
    };
    let topics = record_of(&looked, "topics");
    let lead = record_of(&looked, "lead");

    // No similar record lookup is made for the set; the single-valued attribute is looked up there.
    assert!(topics.similar.is_none());
    assert!(lead.similar.is_some());
    let answered = topics.answered.expect("the set answered");
    assert_eq!(answered.value, "nlp");
    assert_eq!(answered.threshold, Some(0.3));

    // With nlp chosen, vision and genomics are both trusted on their own scores; the decision record names the first of
    // them — what the answer leads with.
    looked.clear();
    let answers = decider.answer_with(&schema, "d4", &known("ai"), &chosen(&["nlp"]), &mut |r| {
        looked.push(r.clone())
    });
    let topics_answer = answers
        .iter()
        .find(|a| a.attribute == "topics")
        .expect("the set stays open");
    assert_eq!(
        topics_answer
            .candidates
            .iter()
            .map(|c| (c.value.as_str(), c.trusted))
            .collect::<Vec<_>>(),
        [("vision", true), ("genomics", true)]
    );
    assert_eq!(
        record_of(&looked, "topics")
            .answered
            .expect("the set answered")
            .value,
        "vision"
    );
}

#[test]
fn values_kept_where_their_kind_does_not_belong_are_told_apart_from_values_in_place() {
    let schema = paper();
    let mut misplaced_set = known("ai");
    misplaced_set.insert("topics".into(), "nlp".into());
    let misplaced_single = BTreeMap::from([("lead".to_owned(), vec!["ada".to_owned()])]);

    assert!(
        schema
            .check_placement(&known("ai"), &chosen(&["nlp"]))
            .is_ok()
    );
    assert!(
        schema
            .check_placement(&misplaced_set, &BTreeMap::new())
            .unwrap_err()
            .contains("'topics' takes several values")
    );
    assert!(
        schema
            .check_placement(&known("ai"), &misplaced_single)
            .unwrap_err()
            .contains("'lead' takes one value")
    );
}

#[test]
#[should_panic(expected = "'topics' takes several values")]
fn a_record_keeping_a_set_as_one_value_is_not_put_into_memory() {
    let schema = paper();
    let mut record = doc(1, Some("ai"), &[], None);
    record.sets.clear();
    record.values.insert("topics".into(), "nlp".into());
    KeyMemory::default().put(&schema, &record);
}

#[test]
#[should_panic(expected = "'lead' takes one value")]
fn a_record_keeping_one_value_as_a_set_is_not_put_into_memory() {
    let schema = paper();
    let mut record = doc(1, Some("ai"), &["nlp"], None);
    record.sets.insert("lead".into(), vec!["ada".into()]);
    KeyMemory::default().put(&schema, &record);
}

#[test]
#[should_panic(expected = "'topics' takes several values")]
fn a_stateless_answer_refuses_a_set_given_as_one_value() {
    let schema = paper();
    let mut decider = Decider::new(KeyMemory::new(60, 1.0), None);
    decider.rebuild(&schema, &history());
    let mut values = known("ai");
    values.insert("topics".into(), "nlp".into());
    decider.answer(&schema, "d9", &values, &BTreeMap::new());
}

#[test]
#[should_panic(expected = "'lead' takes one value")]
fn answering_one_attribute_refuses_one_value_given_as_a_set() {
    let schema = paper();
    let decider = Decider::new(KeyMemory::new(60, 1.0), None);
    let sets = BTreeMap::from([("lead".to_owned(), vec!["ada".to_owned()])]);
    decider.answer_one(&schema, "d9", "topics", &known("ai"), &sets, None);
}
