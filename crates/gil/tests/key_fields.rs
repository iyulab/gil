//! Choosing the attributes that key a decided attribute in the settled key memory by replaying its key layer, held to what
//! the reference implementation does: where a few attributes decide an attribute with many values and the others only
//! blur it, keying it by those few answers more often at the same precision, while the lookup still reads every attribute
//! the attribute rests on. The choice is the replay `select_key_threshold` makes with the attributes chosen; key
//! attributes narrow the key layer exactly as resting on them did; the attribute's own key attributes are ignored and its
//! dependencies bound the attributes tried; an attribute naming what is described is kept though a broad one alone answers
//! more often; no gain leaves the key attributes unset; an attribute that is not evidence is never tried nor accepted as a
//! key attribute; only a decided attribute is asked about; and counting typing brings the typed thresholds for the choice,
//! but not for an attribute that takes several values.

use std::collections::BTreeMap;

use gil::key_memory::KeyMemory;
use gil::replay::{
    Choice, ThresholdChoice, ThresholdReplay, select_key_concentrations, select_key_fields,
    select_key_threshold, select_typed_key_thresholds,
};
use gil::schema::{Attribute, AttributeRole, Record, Schema};

const MINUTE: i64 = 60 * 10_000_000;
const PARTS: [&str; 8] = [
    "valve", "pump", "seal", "hose", "gauge", "filter", "fan", "relay",
];
const PLACES: [&str; 3] = ["left", "right", "aft"];

/// A small deterministic generator (SplitMix64), so the records are the same on every run.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn names(names: &[&str]) -> Option<Vec<String>> {
    Some(names.iter().map(|n| (*n).to_owned()).collect())
}

/// The code follows from the part. The model is drawn from many values, each seen a few times: a key seen rarely lends
/// its few codes most of their strength, so it pushes wrong codes up. The place and the shift are drawn from a few.
fn report(key_fields: Option<Vec<String>>, depends_on: Option<Vec<String>>) -> Schema {
    let mut code = Attribute::new("code", AttributeRole::Decided);
    code.key_fields = key_fields;
    code.depends_on = depends_on;
    Schema {
        name: "report".into(),
        attributes: vec![
            Attribute::new("part", AttributeRole::Observed),
            Attribute::new("place", AttributeRole::Observed),
            Attribute::new("model", AttributeRole::Observed),
            Attribute::new("shift", AttributeRole::Observed),
            code,
        ],
    }
}

fn record(i: usize, values: &[(&str, String)]) -> Record {
    Record {
        id: format!("d{i}"),
        values: values
            .iter()
            .map(|(a, v)| ((*a).to_owned(), v.clone()))
            .collect(),
        settled_at: i as i64 * MINUTE,
        ..Record::default()
    }
}

fn reports(count: usize, seed: u64) -> Vec<Record> {
    let mut draws = Draws(seed);
    (0..count)
        .map(|i| {
            let part = draws.below(PARTS.len());
            let place = draws.below(PLACES.len());
            // A tenth of the codes are miscoded at random, so no key is perfectly pure.
            let code = if draws.unit() < 0.1 {
                draws.below(PARTS.len())
            } else {
                part
            };
            record(
                i,
                &[
                    ("part", PARTS[part].to_owned()),
                    ("place", PLACES[place].to_owned()),
                    ("model", format!("m{}", draws.below(300))),
                    ("shift", format!("s{}", draws.below(3))),
                    ("code", format!("c{code}")),
                ],
            )
        })
        .collect()
}

fn choice(minimum_answered: usize) -> Choice<'static> {
    Choice {
        target_precision: 0.8,
        minimum_answered,
        counting: None,
    }
}

fn key_threshold(schema: &Schema, attribute: &str, records: &[Record]) -> ThresholdReplay {
    select_key_threshold(
        &mut KeyMemory::default(),
        schema,
        attribute,
        records,
        choice(30),
    )
}

fn answered(replay: &ThresholdReplay) -> usize {
    replay.chosen.as_ref().map_or(0, |c| c.answered)
}

#[test]
fn the_attributes_that_decide_the_attribute_are_chosen_and_answer_more_often_than_every_attribute()
{
    let records = reports(1500, 5);

    let chosen = select_key_fields(&report(None, None), "code", &records, choice(30), 0);

    let key_fields = chosen.key_fields.as_ref().expect("key attributes chosen");
    assert_eq!(key_fields[0], "part");
    assert!(!key_fields.iter().any(|k| k == "model"));
    let best = chosen.chosen.chosen.as_ref().expect("a threshold");
    assert!(best.answered > answered(&chosen.all_fields));
    assert!(best.precision >= 0.8);
    // Every attribute it rests on alone, in the schema's order, then the additions tried.
    let alone: Vec<&str> = chosen
        .trials
        .iter()
        .take(4)
        .map(|t| {
            assert_eq!(t.key_fields.len(), 1);
            t.key_fields[0].as_str()
        })
        .collect();
    assert_eq!(alone, ["part", "place", "model", "shift"]);
}

#[test]
fn the_choice_is_the_replay_select_key_threshold_makes_with_those_attributes() {
    let records = reports(1500, 5);
    let chosen = select_key_fields(&report(None, None), "code", &records, choice(30), 0);

    let again = key_threshold(&report(chosen.key_fields.clone(), None), "code", &records);
    let all = key_threshold(&report(None, None), "code", &records);

    assert_eq!(chosen.chosen, again);
    assert_eq!(chosen.all_fields, all);
}

#[test]
fn keying_by_a_few_attributes_replays_the_key_layer_as_resting_on_them_did() {
    // Key attributes narrow the key layer exactly as dependencies did: the same keys, the same replay.
    let records = reports(1500, 5);

    let keyed = key_threshold(&report(names(&["part", "place"]), None), "code", &records);
    let resting = key_threshold(&report(None, names(&["part", "place"])), "code", &records);

    assert_eq!(keyed, resting);
}

#[test]
fn key_attributes_narrow_only_the_key_layer_and_the_lookup_still_reads_every_attribute_rested_on() {
    let schema = report(names(&["part"]), None);
    let values: BTreeMap<String, String> = [
        ("part", "valve"),
        ("place", "aft"),
        ("model", "m7"),
        ("shift", "s1"),
    ]
    .into_iter()
    .map(|(a, v)| (a.to_owned(), v.to_owned()))
    .collect();

    assert!(schema.keys("part", "code"));
    assert!(!schema.keys("model", "code"));
    assert!(schema.supports("model", "code"));
    let read: Vec<String> = schema
        .evidence_values("code", &values, &BTreeMap::new())
        .into_iter()
        .map(|(a, _)| a)
        .collect();
    assert_eq!(read, ["part", "place", "model", "shift"]);
}

#[test]
fn the_attributes_own_key_attributes_are_ignored_and_its_dependencies_bound_the_attributes_tried() {
    let records = reports(1500, 5);

    let unset = select_key_fields(&report(None, None), "code", &records, choice(30), 0);
    let set = select_key_fields(
        &report(names(&["model"]), None),
        "code",
        &records,
        choice(30),
        0,
    );
    let bounded_schema = report(None, names(&["part", "model"]));
    let bounded = select_key_fields(&bounded_schema, "code", &records, choice(30), 0);

    assert!(unset.key_fields.is_some());
    assert_eq!(set.key_fields, unset.key_fields);
    assert_eq!(set.all_fields, unset.all_fields);
    assert!(
        bounded
            .trials
            .iter()
            .flat_map(|t| &t.key_fields)
            .all(|k| k == "part" || k == "model")
    );
    assert_eq!(
        bounded.all_fields,
        key_threshold(&bounded_schema, "code", &records)
    );
}

#[test]
fn key_attributes_must_be_attributes_the_attribute_rests_on() {
    // The attribute itself, not an attribute of the schema, and one outside those it rests on.
    for key in ["code", "missing", "place"] {
        let error = report(names(&[key]), names(&["part", "model"]))
            .check()
            .expect_err("a key attribute it does not rest on");
        assert!(
            error.contains(&format!("keyed by '{key}'")),
            "{key}: {error}"
        );
    }
}

#[test]
fn an_attribute_that_is_not_evidence_cannot_key_another() {
    let mut reporter = Attribute::new("reporter", AttributeRole::Observed);
    reporter.use_as_evidence = false;
    let mut code = Attribute::new("code", AttributeRole::Decided);
    code.key_fields = names(&["reporter"]);
    let schema = Schema {
        name: "report".into(),
        attributes: vec![
            Attribute::new("part", AttributeRole::Observed),
            reporter,
            code,
        ],
    };

    let error = schema
        .check()
        .expect_err("a key attribute that is not evidence");
    assert!(error.contains("keyed by 'reporter'"), "{error}");
}

#[test]
fn an_attribute_naming_what_is_described_is_kept_though_a_broad_attribute_alone_answers_more_often()
{
    // Each outlet keeps its own risk, and the kind of visit mostly follows it; an inspector says nothing. The kind alone
    // answers about every visit, the outlet only the outlets that come again — but there rightly, where the kind alone is
    // wrong. Answers alone would key the risk by the kind; the gain over the target keeps the outlet.
    let mut draws = Draws(3);
    let kinds = ["routine", "complaint", "license"];
    let risks: Vec<usize> = (0..300).map(|_| draws.below(3)).collect();
    let records: Vec<Record> = (0..800)
        .map(|i| {
            let outlet = draws.below(risks.len());
            let kind = if draws.unit() < 0.85 {
                kinds[risks[outlet]]
            } else {
                kinds[draws.below(kinds.len())]
            };
            record(
                i,
                &[
                    ("outlet", format!("o{outlet}")),
                    ("kind", kind.to_owned()),
                    ("inspector", format!("i{}", draws.below(40))),
                    ("risk", format!("r{}", risks[outlet])),
                ],
            )
        })
        .collect();

    let inspection = |concentrations: BTreeMap<String, f64>| {
        let mut risk = Attribute::new("risk", AttributeRole::Decided);
        risk.key_concentrations = concentrations;
        Schema {
            name: "inspection".into(),
            attributes: vec![
                Attribute::new("outlet", AttributeRole::Observed),
                Attribute::new("kind", AttributeRole::Observed),
                Attribute::new("inspector", AttributeRole::Observed),
                risk,
            ],
        }
    };
    let schema = inspection(select_key_concentrations(
        &mut KeyMemory::default(),
        &inspection(BTreeMap::new()),
        "risk",
        &records,
    ));
    let gain = |c: &ThresholdChoice| (c.precision - 0.8) * c.answered_weight;

    let chosen = select_key_fields(&schema, "risk", &records, choice(10), 0);

    let mut key_fields = chosen.key_fields.clone().expect("key attributes chosen");
    key_fields.sort();
    assert_eq!(key_fields, ["kind", "outlet"]);
    let kind_alone = chosen
        .trials
        .iter()
        .find(|t| t.key_fields == ["kind"])
        .and_then(|t| t.replay.chosen.as_ref())
        .expect("the kind alone answers");
    let best = chosen.chosen.chosen.as_ref().expect("a threshold");
    let all = chosen.all_fields.chosen.as_ref().expect("a threshold");
    assert!(kind_alone.answered >= best.answered); // an addition answering no more stops answers alone
    assert!(kind_alone.answered > all.answered);
    assert!(gain(best) > gain(kind_alone));
    assert!(gain(best) > gain(all));
}

#[test]
fn where_no_narrower_set_gains_more_the_key_attributes_stay_unset() {
    // One attribute decides the code on its own and the other carries the same information: narrowing gains nothing.
    let schema = Schema {
        name: "pair".into(),
        attributes: vec![
            Attribute::new("part", AttributeRole::Observed),
            Attribute::new("alias", AttributeRole::Observed),
            Attribute::new("code", AttributeRole::Decided),
        ],
    };
    let records: Vec<Record> = (0..400)
        .map(|i| {
            let n = i % PARTS.len();
            record(
                i,
                &[
                    ("part", PARTS[n].to_owned()),
                    ("alias", format!("a{n}")),
                    ("code", format!("c{n}")),
                ],
            )
        })
        .collect();

    let chosen = select_key_fields(&schema, "code", &records, choice(30), 0);

    assert_eq!(chosen.key_fields, None);
    assert_eq!(chosen.chosen, chosen.all_fields);
}

#[test]
fn an_attribute_that_is_not_evidence_is_never_tried() {
    let mut reporter = Attribute::new("reporter", AttributeRole::Observed);
    reporter.use_as_evidence = false;
    let schema = Schema {
        name: "report".into(),
        attributes: vec![
            Attribute::new("part", AttributeRole::Observed),
            reporter,
            Attribute::new("code", AttributeRole::Decided),
        ],
    };
    let records: Vec<Record> = reports(200, 5)
        .into_iter()
        .map(|mut r| {
            r.values.retain(|a, _| a == "part" || a == "code");
            r.values.insert("reporter".into(), "someone".into());
            r
        })
        .collect();

    let chosen = select_key_fields(&schema, "code", &records, choice(10), 0);

    assert!(!chosen.trials.is_empty());
    assert!(
        chosen
            .trials
            .iter()
            .flat_map(|t| &t.key_fields)
            .all(|k| k != "reporter")
    );
}

#[test]
#[should_panic(expected = "'part' is not a decided attribute")]
fn only_a_decided_attribute_can_be_asked_about() {
    select_key_fields(&report(None, None), "part", &reports(10, 5), choice(5), 0);
}

#[test]
fn counting_typing_brings_the_typed_thresholds_for_the_choice() {
    let records = reports(1500, 5);

    let untyped = select_key_fields(&report(None, None), "code", &records, choice(30), 0);
    let typed = select_key_fields(&report(None, None), "code", &records, choice(30), 2);

    assert!(untyped.typed.is_none());
    assert!(untyped.trials.iter().all(|t| t.typed.is_none()));
    let thresholds = typed.typed.as_ref().expect("typed thresholds");
    assert!(
        typed
            .trials
            .iter()
            .all(|t| t.typed.as_ref().is_some_and(|t| t.by_length.len() == 2))
    );

    // The typed thresholds that come with the choice are those chosen on the schema keyed by it, with its key threshold.
    let mut keyed = report(typed.key_fields.clone(), None);
    for a in keyed.attributes.iter_mut().filter(|a| a.name == "code") {
        a.key_threshold = typed.chosen.chosen.as_ref().map(|c| c.threshold);
    }
    let again = select_typed_key_thresholds(
        &mut KeyMemory::default(),
        &keyed,
        "code",
        &records,
        choice(30),
        2,
    );
    assert_eq!(thresholds.thresholds(), again.thresholds());
}

#[test]
#[should_panic(expected = "'topics' takes several values")]
fn typing_is_not_counted_for_an_attribute_that_takes_several_values() {
    let mut topics = Attribute::new("topics", AttributeRole::Decided);
    topics.multiple = true;
    let schema = Schema {
        name: "paper".into(),
        attributes: vec![Attribute::new("venue", AttributeRole::Observed), topics],
    };

    select_key_fields(&schema, "topics", &[], choice(5), 1);
}
