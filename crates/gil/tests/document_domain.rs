//! An attribute with `candidates_from` is offered only values another attribute of the same record holds, by every layer
//! and every replay, once that attribute has a value — held to what the reference implementation's document domain tests
//! expect.

use std::collections::BTreeMap;

use gil::decider::{Answer, Decider, ExternalCandidate, Source};
use gil::key_memory::{KeyMemory, Lookup};
use gil::lexical_memory::LexicalMemory;
use gil::replay::{Choice, select_key_threshold};
use gil::schema::{Attribute, AttributeRole, Record, Schema};

const MINUTE: i64 = 60 * 10_000_000;

fn paper(main_key_threshold: Option<f64>, main_memory_threshold: Option<f64>) -> Schema {
    let mut topics = Attribute::new("topics", AttributeRole::Decided);
    topics.multiple = true;
    topics.key_threshold = Some(0.3);
    let mut main = Attribute::new("main", AttributeRole::Decided);
    main.candidates_from = Some("topics".into());
    main.key_threshold = main_key_threshold;
    main.memory_threshold = main_memory_threshold;
    main.depends_on = Some(vec!["area".into(), "abstract".into()]);
    Schema {
        name: "paper".into(),
        attributes: vec![
            Attribute::new("area", AttributeRole::Observed),
            Attribute::new("abstract", AttributeRole::Observed),
            topics,
            main,
        ],
    }
}

fn record(number: i64, area: &str, topics: &[&str], main: &str, text: Option<&str>) -> Record {
    Record {
        id: format!("d{number}"),
        values: BTreeMap::from([
            ("area".to_owned(), area.to_owned()),
            (
                "abstract".to_owned(),
                text.map_or_else(|| format!("paper {number}"), str::to_owned),
            ),
            ("main".to_owned(), main.to_owned()),
        ]),
        sets: BTreeMap::from([(
            "topics".to_owned(),
            topics.iter().map(|t| (*t).to_owned()).collect(),
        )]),
        settled_at: number * MINUTE,
        ..Record::default()
    }
}

fn history() -> Vec<Record> {
    vec![
        record(1, "ai", &["nlp", "vision"], "nlp", None),
        record(2, "ai", &["nlp"], "nlp", None),
        record(3, "ai", &["nlp", "speech"], "nlp", None),
        record(4, "ai", &["vision", "robotics"], "vision", None),
    ]
}

fn values(area: &str, text: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("area".to_owned(), area.to_owned()),
        ("abstract".to_owned(), text.to_owned()),
    ])
}

fn topics(chosen: &[&str]) -> BTreeMap<String, Vec<String>> {
    BTreeMap::from([(
        "topics".to_owned(),
        chosen.iter().map(|t| (*t).to_owned()).collect(),
    )])
}

fn main_answer(
    decider: &Decider,
    schema: &Schema,
    known: &BTreeMap<String, String>,
    sets: &BTreeMap<String, Vec<String>>,
) -> Answer {
    decider
        .answer(schema, "d9", known, sets)
        .into_iter()
        .find(|a| a.attribute == "main")
        .expect("main is open")
}

fn values_of(answer: &Answer) -> Vec<&str> {
    answer.candidates.iter().map(|c| c.value.as_str()).collect()
}

#[test]
fn the_domain_narrows_to_the_other_attributes_values_once_it_has_any() {
    let mut main = Attribute::new("main", AttributeRole::Decided);
    main.candidates_from = Some("topics".into());
    let mut closed = main.clone();
    closed.candidates = Some(vec!["nlp".into(), "vision".into()]);
    let mut category = Attribute::new("subcategory", AttributeRole::Decided);
    category.candidates_from = Some("category".into());
    let none = BTreeMap::new();
    let no_sets = BTreeMap::new();

    assert!(main.admits_in("speech", &none, &no_sets));
    assert!(main.admits_in("speech", &none, &topics(&[])));
    assert!(main.admits_in("speech", &none, &topics(&["nlp", "speech"])));
    assert!(!main.admits_in("vision", &none, &topics(&["nlp", "speech"])));
    assert!(!main.admits_in("Speech", &none, &topics(&["nlp", "speech"])));
    assert!(!closed.admits_in("speech", &none, &topics(&["nlp", "speech"])));

    let housing = BTreeMap::from([("category".to_owned(), "housing".to_owned())]);
    let empty = BTreeMap::from([("category".to_owned(), String::new())]);
    assert!(category.admits_in("housing", &housing, &no_sets));
    assert!(!category.admits_in("debt", &housing, &no_sets));
    assert!(category.admits_in("debt", &empty, &no_sets));
}

#[test]
fn the_key_layer_offers_only_the_records_topics_and_trusts_on_the_best_of_them() {
    let schema = paper(Some(0.3), None);
    let mut decider = Decider::new(KeyMemory::default(), None);
    decider.rebuild(&schema, &history());
    let ai = values("ai", "new paper");

    let open = main_answer(&decider, &schema, &ai, &topics(&[]));
    let narrowed = main_answer(&decider, &schema, &ai, &topics(&["vision", "robotics"]));

    assert_eq!(open.candidates[0].value, "nlp");
    assert!(open.answered());
    // Under area=ai "nlp" scores 0.6 and "vision" 0.2: "vision" leads the narrowed domain without borrowing nlp's trust.
    assert_eq!(values_of(&narrowed), ["vision"]);
    assert!(!narrowed.answered());
}

#[test]
fn a_value_settled_outside_its_records_domain_is_remembered_and_offered_where_it_fits() {
    let schema = paper(None, None);
    let mut decider = Decider::new(KeyMemory::default(), None);
    let mut settled = history();
    settled.push(record(5, "bio", &["genomics"], "proteins", None));
    decider.rebuild(&schema, &settled);
    let bio = values("bio", "new paper");

    let outside = main_answer(&decider, &schema, &bio, &topics(&["genomics"]));
    let inside = main_answer(&decider, &schema, &bio, &topics(&["proteins", "genomics"]));

    assert!(!values_of(&outside).contains(&"proteins"));
    assert_eq!(inside.candidates[0].value, "proteins");
}

#[test]
fn an_external_value_outside_the_records_domain_is_dropped() {
    let schema = paper(Some(0.3), None);
    let decider = Decider::new(KeyMemory::default(), None);
    let ai = values("ai", "new paper");
    let chosen = topics(&["vision"]);
    let q = Lookup::new(&schema, "main", &ai)
        .sets(&chosen)
        .excluding(Some("d9"));
    let ranked = ["nlp", "vision"].map(|v| ExternalCandidate {
        value: v.into(),
        probability: 0.5,
        trusted: false,
    });

    let answer = decider.offer_external(&q, &ranked);

    assert_eq!(values_of(&answer), ["vision"]);
}

#[test]
fn a_similar_record_whose_value_lies_outside_the_records_domain_does_not_vote() {
    let schema = paper(None, Some(0.3));
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    decider.rebuild(
        &schema,
        &[
            record(
                1,
                "ai",
                &["nlp", "vision"],
                "nlp",
                Some("parsing sentences with a neural network"),
            ),
            record(
                2,
                "ai",
                &["vision"],
                "vision",
                Some("segmenting images of cells"),
            ),
        ],
    );
    let asked = values("ai", "parsing sentences with a neural network again");

    let open = main_answer(&decider, &schema, &asked, &topics(&[]));
    let narrowed = main_answer(&decider, &schema, &asked, &topics(&["vision"]));

    assert!(
        open.candidates
            .iter()
            .any(|c| c.source == Source::SimilarDocument && c.value == "nlp" && c.trusted)
    );
    // Narrowed to vision, d1 does not vote; d2 alone does, on its own similarity.
    assert!(!values_of(&narrowed).contains(&"nlp"));
    let similar: Vec<_> = narrowed
        .candidates
        .iter()
        .filter(|c| c.source == Source::SimilarDocument)
        .collect();
    assert_eq!(similar.len(), 1);
    assert_eq!(similar[0].evidence.as_deref(), Some("d2"));
}

#[test]
fn a_key_threshold_replay_narrows_as_answers_do() {
    // The main topic is the area's usual one only when the record has it: a replay that ignored the domain would count
    // "nlp" against every record without it.
    let records: Vec<Record> = (1..=40)
        .map(|i| {
            if i % 4 == 0 {
                record(i, "ai", &["vision", "robotics"], "vision", None)
            } else {
                record(i, "ai", &["nlp", "vision"], "nlp", None)
            }
        })
        .collect();
    let schema = paper(Some(0.3), None);
    let mut open = schema.clone();
    for a in &mut open.attributes {
        a.candidates_from = None;
    }
    let choice = Choice {
        target_precision: 0.95,
        minimum_answered: 5,
        counting: None,
    };

    let narrowed =
        select_key_threshold(&mut KeyMemory::default(), &schema, "main", &records, choice);
    let unnarrowed =
        select_key_threshold(&mut KeyMemory::default(), &open, "main", &records, choice);

    assert_eq!(narrowed.chosen.expect("a threshold").precision, 1.0);
    assert!(unnarrowed.chosen.is_none());
}
