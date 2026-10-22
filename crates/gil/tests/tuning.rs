//! A tuning: captured from a schema, put back on a fresh one, and stale exactly when the choice no longer stands.

use std::collections::BTreeMap;

use gil::decider::Source;
use gil::replay::{LayerThresholds, ThresholdChoice, ThresholdReplay};
use gil::schema::{Attribute, AttributeRole, CoarseLevel, Schema};
use gil::tuning::{Stale, Tuning, VERSION, fingerprint};

fn schema() -> Schema {
    let mut team = Attribute::new("team", AttributeRole::Decided);
    team.candidates = Some(vec!["network".into(), "security".into()]);
    let mut code = Attribute::new("code", AttributeRole::Decided);
    code.coarse = Some(CoarseLevel::by_prefix(2));
    Schema::new(
        "ticket",
        vec![
            Attribute::new("product", AttributeRole::Observed),
            Attribute::new("summary", AttributeRole::Observed),
            team,
            code,
        ],
    )
    .expect("a valid schema")
}

/// `schema` with `edit` made to its attributes.
fn changed(schema: &Schema, edit: impl FnOnce(&mut [Attribute])) -> Schema {
    let mut attributes = schema.attributes().to_vec();
    edit(&mut attributes);
    Schema::new(schema.name(), attributes).expect("a valid schema")
}

fn tuned() -> Schema {
    changed(&schema(), |s| {
        let team = &mut s[2];
        team.key_fields = Some(vec!["product".into()]);
        team.key_concentrations = BTreeMap::from([("product".to_owned(), 0.5)]);
        team.key_threshold = Some(1.25);
        team.typed_key_thresholds = Some(vec![None, Some(2.0)]);
        team.memory_threshold = Some(0.6);
        team.memory_floor = Some(0.3);
        let level = s[3].coarse.as_mut().unwrap();
        level.key_threshold = Some(1.5);
        level.memory_threshold = Some(0.4);
    })
}

fn choice(threshold: f64, promise: f64) -> ThresholdReplay {
    ThresholdReplay {
        chosen: Some(ThresholdChoice {
            threshold,
            precision: 0.9,
            answer_rate: 0.5,
            answered: 100,
            lookups: 200,
            answered_weight: 100.0,
            effective_answered: 100.0,
            similarity_floor: None,
            promise,
            promise_answered: 100.0,
        }),
        most_precise: None,
        lookups: 200,
        candidates: 150,
        keys: None,
    }
}

#[test]
fn a_tuning_puts_back_on_a_fresh_schema_exactly_what_was_chosen() {
    let chosen = tuned();
    let mut tuning = Tuning::capture(&chosen, 1_000, 0.8);
    tuning.promise(
        "team",
        &LayerThresholds {
            key: choice(1.25, 0.86),
            memory: choice(0.6, 0.82),
            batches: None,
        },
    );
    assert_eq!(tuning.version, VERSION);
    assert_eq!(
        tuning
            .attributes
            .iter()
            .map(|a| a.attribute.as_str())
            .collect::<Vec<_>>(),
        ["team", "code"]
    );
    let team = &tuning.attributes[0];
    assert_eq!(
        (team.key_promise, team.memory_promise),
        (Some(0.86), Some(0.82))
    );

    let fresh = tuning.apply(&schema()).expect("a valid schema");
    for (a, b) in fresh.attributes().iter().zip(chosen.attributes()) {
        assert_eq!(a.key_fields, b.key_fields);
        assert_eq!(a.key_concentrations, b.key_concentrations);
        assert_eq!(a.key_threshold, b.key_threshold);
        assert_eq!(a.typed_key_thresholds, b.typed_key_thresholds);
        assert_eq!(a.memory_threshold, b.memory_threshold);
        assert_eq!(a.memory_floor, b.memory_floor);
        assert_eq!(a.coarse, b.coarse);
    }
}

#[test]
fn the_choice_stands_until_the_version_the_schema_or_the_settled_records_move() {
    let chosen = tuned();
    let tuning = Tuning::capture(&chosen, 1_000, 0.8);
    assert_eq!(tuning.stale(&chosen, 1_000), None);
    assert_eq!(tuning.stale(&chosen, 1_099), None);
    assert_eq!(
        tuning.stale(&chosen, 1_100),
        Some(Stale::Settled {
            chosen: 1_000,
            now: 1_100
        })
    );
    assert!(matches!(
        tuning.stale(&chosen, 900),
        Some(Stale::Settled { .. })
    ));

    // Few records: any change is a tenth.
    let early = Tuning::capture(&chosen, 5, 0.8);
    assert_eq!(early.stale(&chosen, 5), None);
    assert!(early.stale(&chosen, 6).is_some());

    let mut other = tuning.clone();
    other.version = "0.0.1".into();
    assert_eq!(
        other.stale(&chosen, 1_000),
        Some(Stale::Version {
            chosen: "0.0.1".into()
        })
    );

    let narrowed = changed(&chosen, |a| a[2].candidates = Some(vec!["network".into()]));
    assert_eq!(tuning.stale(&narrowed, 1_000), Some(Stale::Schema));
}

#[test]
fn the_fingerprint_ignores_what_the_replays_choose_and_is_the_same_in_every_run() {
    assert_eq!(fingerprint(&schema()), fingerprint(&tuned()));

    let votes = changed(&schema(), |a| a[2].similar_votes = 5);
    let depends = changed(&schema(), |a| {
        a[2].depends_on = Some(vec!["product".into()])
    });
    let from = changed(&schema(), |a| a[3].candidates_from = Some("team".into()));
    for changed in [votes, depends, from] {
        assert_ne!(fingerprint(&changed), fingerprint(&schema()));
    }

    // Pinned: a change to the fingerprint makes every stored tuning stale at once, so it must be deliberate.
    assert_eq!(fingerprint(&schema()), 0x90ce_eea9_3c01_9a43);
}

#[test]
fn a_tuning_that_would_break_the_schema_is_refused() {
    let tuning = Tuning::capture(&tuned(), 1_000, 0.8);
    let elsewhere = changed(&schema(), |a| a[0].name = "model".into());

    assert_eq!(
        tuning.apply(&elsewhere).map(|_| ()),
        Err("attribute 'team' is keyed by 'product', which is not an attribute it rests on".into())
    );
}

fn floored(threshold: f64, floor: f64, promise: f64) -> ThresholdReplay {
    let mut replay = choice(threshold, promise);
    replay.chosen.as_mut().unwrap().similarity_floor = Some(floor);
    replay
}

fn none() -> ThresholdReplay {
    ThresholdReplay {
        chosen: None,
        ..choice(0.0, 0.0)
    }
}

#[test]
fn what_a_layers_replay_chose_goes_on_the_attribute_and_an_unchosen_layer_is_unset() {
    let layers = LayerThresholds {
        key: choice(1.25, 0.86),
        memory: floored(0.6, 0.3, 0.82),
        batches: None,
    };
    let schema = schema().with_layers("team", &layers).unwrap();
    let team = schema.attribute("team");
    assert_eq!(
        (team.key_threshold, team.memory_threshold, team.memory_floor),
        (Some(1.25), Some(0.6), Some(0.3))
    );

    let without_memory = schema
        .with_layers(
            "team",
            &LayerThresholds {
                key: choice(2.0, 0.9),
                memory: none(),
                batches: None,
            },
        )
        .unwrap();
    let team = without_memory.attribute("team");
    assert_eq!(
        (team.key_threshold, team.memory_threshold, team.memory_floor),
        (Some(2.0), None, None)
    );
}

#[test]
fn what_a_coarse_replay_chose_goes_on_the_coarse_level() {
    let layers = LayerThresholds {
        key: choice(1.5, 0.9),
        memory: floored(0.4, 0.2, 0.85),
        batches: None,
    };
    let schema = schema().with_coarse("code", &layers).unwrap();
    let level = schema.attribute("code").coarse.clone().unwrap();
    assert_eq!(
        (
            level.key_threshold,
            level.memory_threshold,
            level.memory_floor
        ),
        (Some(1.5), Some(0.4), Some(0.2))
    );
    assert_eq!(schema.attribute("code").key_threshold, None);
}

#[test]
fn putting_a_choice_on_an_attribute_that_cannot_take_it_is_refused() {
    let layers = LayerThresholds {
        key: choice(1.0, 0.9),
        memory: choice(0.5, 0.9),
        batches: None,
    };
    let schema = schema();
    assert!(schema.with_layers("nothing", &layers).is_err());
    assert!(
        schema
            .with_layers("product", &layers)
            .unwrap_err()
            .contains("not decided")
    );
    assert!(
        schema
            .with_coarse("team", &layers)
            .unwrap_err()
            .contains("no coarse level")
    );

    let mut tags = Attribute::new("tags", AttributeRole::Decided);
    tags.multiple = true;
    let several = Schema::new(
        "t",
        vec![Attribute::new("summary", AttributeRole::Observed), tags],
    )
    .unwrap();
    assert!(several.with_layers("tags", &layers).is_err());
}

#[test]
fn an_answer_reads_its_promise_from_the_layer_it_came_from() {
    let mut tuning = Tuning::capture(&schema(), 1_000, 0.8);
    assert_eq!(tuning.promise_of("team", Source::KeyMemory), None);
    tuning.promise(
        "team",
        &LayerThresholds {
            key: choice(1.25, 0.86),
            memory: choice(0.6, 0.82),
            batches: None,
        },
    );
    assert_eq!(tuning.promise_of("team", Source::KeyMemory), Some(0.86));
    assert_eq!(tuning.promise_of("team", Source::SimilarRecord), Some(0.82));
    assert_eq!(tuning.promise_of("team", Source::None), None);
    assert_eq!(tuning.promise_of("team", Source::External), None);
    assert_eq!(tuning.promise_of("nothing", Source::KeyMemory), None);
}

#[test]
fn a_coarse_class_reads_its_promise_from_the_level_not_the_value() {
    let mut tuning = Tuning::capture(&schema(), 1_000, 0.8);
    tuning.promise(
        "team",
        &LayerThresholds {
            key: choice(1.25, 0.86),
            memory: choice(0.6, 0.82),
            batches: None,
        },
    );
    assert_eq!(tuning.coarse_promise_of("team", Source::KeyMemory), None);
    tuning.promise_coarse(
        "team",
        &LayerThresholds {
            key: choice(2.0, 0.93),
            memory: choice(0.4, 0.9),
            batches: None,
        },
    );
    assert_eq!(
        tuning.coarse_promise_of("team", Source::KeyMemory),
        Some(0.93)
    );
    assert_eq!(
        tuning.coarse_promise_of("team", Source::SimilarRecord),
        Some(0.9)
    );
    assert_eq!(tuning.coarse_promise_of("team", Source::External), None);
    assert_eq!(tuning.promise_of("team", Source::KeyMemory), Some(0.86));
}
