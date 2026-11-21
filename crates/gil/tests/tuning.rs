//! A tuning: captured from a schema, put back on a fresh one, and stale exactly when the choice no longer stands.

use std::collections::BTreeMap;

use gil::decider::Source;
use gil::replay::{
    Choice, LayerThresholds, ThresholdChoice, ThresholdReplay, ThresholdRule,
    select_score_threshold,
};
use gil::schema::{Attribute, AttributeRole, CoarseLevel, KeyCombine, Schema, SettlementKind};
use gil::tuning::{SCALE, Stale, Tuning, VERSION, choice_fingerprint, fingerprint};

/// What a choice is asked with: `target` and the defaults of a host that sets nothing else.
fn asked(target_precision: f64) -> Choice<'static> {
    Choice {
        target_precision,
        minimum_answered: 10,
        counting: None,
        uncounted_batches: &[],
    }
}

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

/// A replay that chose `threshold`. A replay is what the replays return (it grows fields), so it is taken from one that
/// had nothing to replay and set to what the test needs.
fn choice(threshold: f64, promise: f64) -> ThresholdReplay {
    let mut replay = select_score_threshold(&[], asked(0.9));
    replay.absence = None;
    replay.chosen = Some(ThresholdChoice {
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
    });
    replay.most_precise = None;
    replay.lookups = 200;
    replay.candidates = 150;
    replay.keys = None;
    replay
}

#[test]
fn a_tuning_puts_back_on_a_fresh_schema_exactly_what_was_chosen() {
    let chosen = tuned();
    let mut tuning = Tuning::capture(&chosen, 1_000, &asked(0.8), ThresholdRule::EveryBand);
    tuning.promise(
        "team",
        &LayerThresholds {
            key: choice(1.25, 0.86),
            memory: choice(0.6, 0.82),
            batches: None,
            value_tails: None,
            calibration: None,
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
    let tuning = Tuning::capture(&chosen, 1_000, &asked(0.8), ThresholdRule::EveryBand);
    assert_eq!(
        tuning.stale(&chosen, 1_000, &asked(0.8), ThresholdRule::EveryBand),
        None
    );
    assert_eq!(
        tuning.stale(&chosen, 1_099, &asked(0.8), ThresholdRule::EveryBand),
        None
    );
    assert_eq!(
        tuning.stale(&chosen, 1_100, &asked(0.8), ThresholdRule::EveryBand),
        Some(Stale::Settled {
            chosen: 1_000,
            now: 1_100
        })
    );
    assert!(matches!(
        tuning.stale(&chosen, 900, &asked(0.8), ThresholdRule::EveryBand),
        Some(Stale::Settled { .. })
    ));

    // Few records: any change is a tenth.
    let early = Tuning::capture(&chosen, 5, &asked(0.8), ThresholdRule::EveryBand);
    assert_eq!(
        early.stale(&chosen, 5, &asked(0.8), ThresholdRule::EveryBand),
        None
    );
    assert!(
        early
            .stale(&chosen, 6, &asked(0.8), ThresholdRule::EveryBand)
            .is_some()
    );

    let mut other = tuning.clone();
    other.version = "0.0.1".into();
    assert_eq!(
        other.stale(&chosen, 1_000, &asked(0.8), ThresholdRule::EveryBand),
        Some(Stale::Version {
            chosen: "0.0.1".into()
        })
    );

    let narrowed = changed(&chosen, |a| a[2].candidates = Some(vec!["network".into()]));
    assert_eq!(
        tuning.stale(&narrowed, 1_000, &asked(0.8), ThresholdRule::EveryBand),
        Some(Stale::Schema)
    );
}

#[test]
fn the_fingerprint_ignores_what_the_replays_choose_and_is_the_same_in_every_run() {
    assert_eq!(fingerprint(&schema()), fingerprint(&tuned()));

    let votes = changed(&schema(), |a| a[2].similar_votes = 5);
    let depends = changed(&schema(), |a| {
        a[2].depends_on = Some(vec!["product".into()])
    });
    let from = changed(&schema(), |a| a[3].candidates_from = Some("team".into()));
    // How the keys' strengths combine changes every key score, so a choice made under one rule does not stand under another.
    let combine = changed(&schema(), |a| a[2].key_combine = KeyCombine::Max);
    for changed in [votes, depends, from, combine] {
        assert_ne!(fingerprint(&changed), fingerprint(&schema()));
    }

    // Pinned: a change to the fingerprint makes every stored tuning stale at once, so it must be deliberate.
    assert_eq!(fingerprint(&schema()), 0x90ce_eea9_3c01_9a43);
}

#[test]
fn a_tuning_that_would_break_the_schema_is_refused() {
    let tuning = Tuning::capture(&tuned(), 1_000, &asked(0.8), ThresholdRule::EveryBand);
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
    let mut replay = choice(0.0, 0.0);
    replay.chosen = None;
    replay
}

#[test]
fn what_a_layers_replay_chose_goes_on_the_attribute_and_an_unchosen_layer_is_unset() {
    let layers = LayerThresholds {
        key: choice(1.25, 0.86),
        memory: floored(0.6, 0.3, 0.82),
        batches: None,
        value_tails: None,
        calibration: None,
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
                value_tails: None,
                calibration: None,
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
        value_tails: None,
        calibration: None,
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
        value_tails: None,
        calibration: None,
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
    let mut tuning = Tuning::capture(&schema(), 1_000, &asked(0.8), ThresholdRule::EveryBand);
    assert_eq!(tuning.promise_of("team", Source::KeyMemory), None);
    tuning.promise(
        "team",
        &LayerThresholds {
            key: choice(1.25, 0.86),
            memory: choice(0.6, 0.82),
            batches: None,
            value_tails: None,
            calibration: None,
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
    let mut tuning = Tuning::capture(&schema(), 1_000, &asked(0.8), ThresholdRule::EveryBand);
    tuning.promise(
        "team",
        &LayerThresholds {
            key: choice(1.25, 0.86),
            memory: choice(0.6, 0.82),
            batches: None,
            value_tails: None,
            calibration: None,
        },
    );
    assert_eq!(tuning.coarse_promise_of("team", Source::KeyMemory), None);
    tuning.promise_coarse(
        "team",
        &LayerThresholds {
            key: choice(2.0, 0.93),
            memory: choice(0.4, 0.9),
            batches: None,
            value_tails: None,
            calibration: None,
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

#[test]
fn a_tuning_carried_from_elsewhere_is_stale_for_its_scale_and_settings_only() {
    let chosen = tuned();
    let tuning = Tuning::capture(&chosen, 1_000, &asked(0.9), ThresholdRule::EveryBand);
    assert_eq!(tuning.scale, SCALE);
    assert_eq!(tuning.stale_carried(&chosen), None);

    // Another crate version, and any count of records here, are no reason: the records it was chosen on are not here.
    let mut other = tuning.clone();
    other.version = "0.0.1".into();
    assert_eq!(other.stale_carried(&chosen), None);
    assert!(
        other
            .stale(&chosen, 1_000, &asked(0.9), ThresholdRule::EveryBand)
            .is_some()
    );
    assert_eq!(tuning.stale_carried(&chosen), None);
    assert!(
        tuning
            .stale(&chosen, 5_000, &asked(0.9), ThresholdRule::EveryBand)
            .is_some()
    );

    // What it was asked with is no reason either: the records to ask again are not here, and a host that carries a
    // tuning does not choose with the settings its sender did.
    let mut elsewhere = tuning.clone();
    elsewhere.choice = choice_fingerprint(&asked(0.5), ThresholdRule::Pooled);
    assert_eq!(elsewhere.stale_carried(&chosen), None);
    elsewhere.choice = 0;
    assert_eq!(elsewhere.stale_carried(&chosen), None);

    // Another score scale is, and so is a tuning that did not record one.
    let mut older = tuning.clone();
    older.scale = SCALE + 1;
    assert_eq!(
        older.stale_carried(&chosen),
        Some(Stale::Scale {
            chosen: SCALE + 1,
            now: SCALE
        })
    );
    older.scale = 0;
    assert_eq!(
        older.stale_carried(&chosen),
        Some(Stale::Scale {
            chosen: 0,
            now: SCALE
        })
    );

    // A changed setting the choice rests on is, as for a tuning of the host's own.
    let narrowed = changed(&chosen, |a| a[2].candidates = Some(vec!["network".into()]));
    assert_eq!(tuning.stale_carried(&narrowed), Some(Stale::Schema));
}

#[test]
fn the_choice_stands_until_what_it_was_asked_with_changes() {
    let chosen = tuned();
    let every = ThresholdRule::EveryBand;
    let tuning = Tuning::capture(&chosen, 1_000, &asked(0.8), every);
    assert_eq!(tuning.target_precision, 0.8);
    assert_eq!(tuning.stale(&chosen, 1_000, &asked(0.8), every), None);

    // Each setting the choice was asked with, changed on its own.
    let accepted = [SettlementKind::Accept];
    let bulk = ["bulk".to_owned()];
    let changes: [(&str, Choice, ThresholdRule); 6] = [
        ("target", asked(0.85), every),
        (
            "fewest answers",
            Choice {
                minimum_answered: 20,
                ..asked(0.8)
            },
            every,
        ),
        (
            "kinds counted",
            Choice {
                counting: Some(&accepted),
                ..asked(0.8)
            },
            every,
        ),
        (
            "batches left out",
            Choice {
                uncounted_batches: &bulk,
                ..asked(0.8)
            },
            every,
        ),
        ("rule", asked(0.8), ThresholdRule::Pooled),
        ("rule least cost", asked(0.8), ThresholdRule::LeastCost),
    ];
    for (name, choice, rule) in changes {
        assert_eq!(
            tuning.stale(&chosen, 1_000, &choice, rule),
            Some(Stale::Choice),
            "{name}"
        );
    }

    // The order the reasons are given in: the version, then the schema, then what it was asked with, then the records.
    let narrowed = changed(&chosen, |a| a[2].candidates = Some(vec!["network".into()]));
    assert_eq!(
        tuning.stale(&narrowed, 5_000, &asked(0.85), every),
        Some(Stale::Schema)
    );
    assert_eq!(
        tuning.stale(&chosen, 5_000, &asked(0.85), every),
        Some(Stale::Choice)
    );
    let mut other = tuning.clone();
    other.version = "0.0.1".into();
    assert!(matches!(
        other.stale(&narrowed, 5_000, &asked(0.85), every),
        Some(Stale::Version { .. })
    ));
}

#[test]
fn the_fingerprint_of_a_choice_reads_kinds_and_batches_as_sets_and_keeps_them_apart() {
    let every = ThresholdRule::EveryBand;
    let kinds = |counting: Option<&[SettlementKind]>| {
        choice_fingerprint(
            &Choice {
                counting,
                ..asked(0.8)
            },
            every,
        )
    };
    let batches = |uncounted_batches: &[String]| {
        choice_fingerprint(
            &Choice {
                uncounted_batches,
                ..asked(0.8)
            },
            every,
        )
    };
    let s = |text: &str| text.to_owned();

    // The kinds are a set: order and repeats say nothing.
    use SettlementKind::{Accept, Correct};
    assert_eq!(
        kinds(Some(&[Accept, Correct])),
        kinds(Some(&[Correct, Accept, Accept]))
    );
    assert_ne!(kinds(Some(&[Accept])), kinds(Some(&[Accept, Correct])));
    // Every settlement counted is not the same as naming every kind: the kinds a later version adds would count.
    assert_ne!(kinds(None), kinds(Some(&[])));

    // So are the batches, and one name is not two: the texts are the host's own.
    assert_eq!(
        batches(&[s("b"), s("a")]),
        batches(&[s("a"), s("b"), s("a")])
    );
    assert_ne!(batches(&[s("a:b")]), batches(&[s("a"), s("b")]));
    assert_ne!(batches(&[s("ab")]), batches(&[s("a"), s("b")]));
    assert_ne!(batches(&[]), batches(&[s("")]));

    // Pinned: a change makes every stored tuning stale at once, so it must be deliberate.
    assert_eq!(
        choice_fingerprint(&asked(0.8), every),
        0x2cf6_a319_8b77_2b0e
    );
}

#[test]
fn a_tuning_that_recorded_no_choice_does_not_stand() {
    // Written before what it was asked with was recorded: nothing says it still stands, as for a scale.
    let chosen = tuned();
    let mut old = Tuning::capture(&chosen, 1_000, &asked(0.8), ThresholdRule::EveryBand);
    old.choice = 0;
    assert_eq!(
        old.stale(&chosen, 1_000, &asked(0.8), ThresholdRule::EveryBand),
        Some(Stale::Choice)
    );
}

#[cfg(feature = "serde")]
#[test]
fn the_fingerprints_are_hex_text_in_json_and_survive_a_reader_that_holds_numbers_as_doubles() {
    let chosen = tuned();
    let tuning = Tuning::capture(&chosen, 1_000, &asked(0.8), ThresholdRule::EveryBand);
    let written = serde_json::to_value(&tuning).expect("a tuning serializes");

    // 16 lowercase hexadecimal digits, so a reader that reads a number into a double cannot round them.
    assert_eq!(
        written["schema"],
        format!("{:016x}", fingerprint(&chosen)).as_str()
    );
    assert_eq!(
        written["choice"],
        format!(
            "{:016x}",
            choice_fingerprint(&asked(0.8), ThresholdRule::EveryBand)
        )
        .as_str()
    );
    assert!(
        fingerprint(&chosen) > (1 << 53),
        "the pinned value is one a double would round"
    );
    let again: Tuning = serde_json::from_value(written).expect("a tuning reads");
    assert_eq!(again, tuning);
    assert_eq!(
        again.stale_carried(&chosen),
        None,
        "the choice still stands after the round trip"
    );

    // A fingerprint written as a number — before they were text — still reads, as does 0 for the one not recorded.
    let mut old = serde_json::to_value(&tuning).unwrap();
    old["schema"] = serde_json::json!(fingerprint(&chosen));
    old["choice"] = serde_json::json!(0);
    let old: Tuning = serde_json::from_value(old).expect("numbers still read");
    assert_eq!(old.schema, fingerprint(&chosen));
    assert_eq!(old.choice, 0);

    // The text is not guessed at: a text that is not up to 16 hexadecimal digits does not read.
    for bad in ["", "xyz", "+1", "-1", "0x10", "10000000000000000", " 1"] {
        let mut broken = serde_json::to_value(&tuning).unwrap();
        broken["schema"] = serde_json::json!(bad);
        assert!(
            serde_json::from_value::<Tuning>(broken).is_err(),
            "{bad:?} reads as a fingerprint"
        );
    }
    let mut negative = serde_json::to_value(&tuning).unwrap();
    negative["choice"] = serde_json::json!(-1);
    assert!(serde_json::from_value::<Tuning>(negative).is_err());
}
