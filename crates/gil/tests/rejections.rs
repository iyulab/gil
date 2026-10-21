//! Arguments refused at the entry points, held to what the reference implementation's threshold selection, coarse
//! level, typed suggestion, set-valued field, field suggestion, ask planner, key memory and lexical memory tests refuse:
//! the similar record layer's selections (`select_layers`, by either rule, and `select_coarse`) and the typed key
//! thresholds refuse an attribute that takes several values, one that is not decided and one the schema does not have;
//! the coarse level's selection an attribute without one; every selection a target precision of 0 or not a number; the
//! typed selection a longest length of 0. A single answer refuses an attribute that is not open — not decided, off, or
//! already holding its one value — typed into or not, and so does an external judgement offered for it. A planner refuses
//! an attribute that is off or unknown, an askable attribute the schema does not have and a budget that is not a number,
//! and advice from another attribute's answer. Memories refuse a key length of 0, a decay that is not a number, n-grams
//! shorter than one scalar or longer ones shorter than the shorter, and a lookup for no matches.

use std::collections::BTreeMap;

use gil::ask_planner::AskPlanner;
use gil::decider::{Decider, ExternalCandidate};
use gil::key_memory::{KeyMemory, Lookup};
use gil::lexical_memory::LexicalMemory;
use gil::replay::{
    Choice, ThresholdRule, select_coarse, select_layers, select_layers_by_rule,
    select_typed_key_thresholds,
};
use gil::schema::{Attribute, AttributePolicy, AttributeRole, CoarseLevel, Record, Schema};

fn choice(target_precision: f64) -> Choice<'static> {
    Choice {
        target_precision,
        minimum_answered: 1,
        counting: None,
    }
}

/// An observed channel and component, a decided team with a coarse level, decided topics that take several values, a
/// decided code with no coarse level and a decided internal note that is off.
fn ticket() -> Schema {
    let mut team = Attribute::new("team", AttributeRole::Decided);
    team.coarse = Some(CoarseLevel::by_prefix(2));
    let mut topics = Attribute::new("topics", AttributeRole::Decided);
    topics.multiple = true;
    let mut internal = Attribute::new("internal", AttributeRole::Decided);
    internal.policy = AttributePolicy::Off;
    Schema::new(
        "ticket",
        vec![
            Attribute::new("channel", AttributeRole::Observed),
            Attribute::new("component", AttributeRole::Observed),
            team,
            topics,
            Attribute::new("code", AttributeRole::Decided),
            internal,
        ],
    )
    .expect("a valid schema")
}

fn records() -> Vec<Record> {
    (0..3)
        .map(|n| Record {
            id: format!("d{n}"),
            values: values(&[("channel", "phone"), ("team", "network")]),
            settled_at: n,
            ..Record::default()
        })
        .collect()
}

fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(a, v)| ((*a).to_owned(), (*v).to_owned()))
        .collect()
}

fn layers(attribute: &str, choice: Choice) {
    select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &ticket(),
        attribute,
        &records(),
        choice,
    );
}

fn coarse(attribute: &str) {
    select_coarse(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &ticket(),
        attribute,
        &records(),
        choice(0.9),
    );
}

fn typed(attribute: &str, longest: usize) {
    select_typed_key_thresholds(
        &mut KeyMemory::default(),
        &ticket(),
        attribute,
        &records(),
        choice(0.9),
        longest,
    );
}

/// The single answer for `attribute` of a record with `known`, typed into or not, from empty memories.
fn answer_one(attribute: &str, known: &[(&str, &str)], typed: Option<&str>) {
    let decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    decider.answer_one(
        &ticket(),
        "d9",
        attribute,
        &values(known),
        &BTreeMap::new(),
        typed,
    );
}

// --- The similar record layer's selections ---

#[test]
#[should_panic(
    expected = "'topics' takes several values; the similar record layer does not answer it"
)]
fn the_layers_are_not_chosen_for_an_attribute_that_takes_several_values() {
    layers("topics", choice(0.9));
}

#[test]
#[should_panic(
    expected = "'topics' takes several values; the similar record layer does not answer it"
)]
fn the_pooled_layers_are_not_chosen_for_an_attribute_that_takes_several_values() {
    select_layers_by_rule(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &ticket(),
        "topics",
        &records(),
        choice(0.9),
        ThresholdRule::Pooled,
    );
}

#[test]
#[should_panic(expected = "'channel' is not a decided attribute")]
fn the_layers_are_not_chosen_for_an_observed_attribute() {
    layers("channel", choice(0.9));
}

#[test]
#[should_panic(expected = "schema 'ticket' has no attribute 'nowhere'")]
fn the_layers_are_not_chosen_for_an_attribute_the_schema_does_not_have() {
    layers("nowhere", choice(0.9));
}

#[test]
#[should_panic(expected = "target_precision must be in (0, 1]")]
fn a_target_of_zero_is_refused() {
    layers("team", choice(0.0));
}

#[test]
#[should_panic(expected = "target_precision must be in (0, 1]")]
fn a_target_that_is_not_a_number_is_refused() {
    layers("team", choice(f64::NAN));
}

#[test]
#[should_panic(
    expected = "'topics' takes several values; the similar record layer does not answer it"
)]
fn the_coarse_level_is_not_chosen_for_an_attribute_that_takes_several_values() {
    coarse("topics");
}

#[test]
#[should_panic(expected = "'code' has no coarse level to choose thresholds for")]
fn the_coarse_level_is_not_chosen_for_an_attribute_without_one() {
    coarse("code");
}

#[test]
#[should_panic(expected = "'channel' is not a decided attribute")]
fn the_coarse_level_is_not_chosen_for_an_observed_attribute() {
    coarse("channel");
}

// --- The typed key thresholds ---

#[test]
#[should_panic(
    expected = "'topics' takes several values; typed key thresholds are for an attribute that takes one"
)]
fn typed_thresholds_are_not_chosen_for_an_attribute_that_takes_several_values() {
    typed("topics", 3);
}

#[test]
#[should_panic(expected = "longest must be at least 1")]
fn typed_thresholds_are_chosen_for_at_least_one_character() {
    typed("team", 0);
}

#[test]
#[should_panic(expected = "'channel' is not a decided attribute")]
fn typed_thresholds_are_not_chosen_for_an_observed_attribute() {
    typed("channel", 3);
}

// --- A single answer ---

#[test]
#[should_panic(expected = "'channel' is not a decided attribute")]
fn an_observed_attribute_is_not_answered() {
    answer_one("channel", &[("component", "vpn")], None);
}

#[test]
#[should_panic(expected = "'internal' is off and is not answered")]
fn an_attribute_that_is_off_is_not_answered() {
    answer_one("internal", &[("component", "vpn")], None);
}

#[test]
#[should_panic(expected = "'team' already has a value")]
fn an_attribute_with_its_value_is_not_answered() {
    answer_one("team", &[("component", "vpn"), ("team", "billing")], None);
}

#[test]
#[should_panic(expected = "schema 'ticket' has no attribute 'nothing'")]
fn an_attribute_the_schema_does_not_have_is_not_answered() {
    answer_one("nothing", &[("component", "vpn")], None);
}

#[test]
#[should_panic(expected = "'channel' is not a decided attribute")]
fn text_is_not_typed_into_an_observed_attribute() {
    answer_one("channel", &[("component", "vpn")], Some("ph"));
}

#[test]
#[should_panic(expected = "'team' already has a value")]
fn text_is_not_typed_into_an_attribute_with_its_value() {
    answer_one("team", &[("team", "network")], Some("n"));
}

#[test]
#[should_panic(expected = "'team' already has a value")]
fn an_external_judgement_is_not_offered_for_an_attribute_with_its_value() {
    let (schema, known) = (ticket(), values(&[("team", "network")]));
    let decider = Decider::new(KeyMemory::default(), None);
    decider.offer_external(
        &Lookup::new(&schema, "team", &known),
        &[ExternalCandidate {
            value: "billing".into(),
            probability: 0.9,
            trusted: true,
        }],
    );
}

// --- The ask planner ---

#[test]
fn a_planner_refuses_an_attribute_it_cannot_plan_for_and_a_budget_that_is_not_a_number() {
    let schema = ticket();
    let refused = |attribute: &str, askable: &[&str], budget: f64| {
        AskPlanner::new(&schema, attribute, askable.iter().copied(), budget)
            .err()
            .expect("refused")
    };

    assert_eq!(
        refused("internal", &["channel"], 0.5),
        "'internal' is not a decided attribute that takes one value and is suggested"
    );
    assert_eq!(
        refused("nowhere", &["channel"], 0.5),
        "schema 'ticket' has no attribute 'nowhere'"
    );
    assert_eq!(
        refused("team", &["nowhere"], 0.5),
        "'nowhere' is not an observed attribute"
    );
    assert_eq!(
        refused("team", &["channel"], f64::NAN),
        "the budget is a share above 0 and at most 1"
    );
}

#[test]
#[should_panic(expected = "the answer is for 'code', not 'team'")]
fn a_planner_advises_only_on_its_own_attribute_s_answer() {
    let schema = ticket();
    let planner = AskPlanner::new(&schema, "team", ["channel"], 0.5).expect("a valid planner");
    let decider = Decider::new(KeyMemory::default(), None);
    let known = BTreeMap::new();
    let other = decider.answer_one(&schema, "d9", "code", &known, &BTreeMap::new(), None);

    planner.advise(&other, &known);
}

// --- The memories ---

#[test]
#[should_panic(expected = "max_key_length must be at least 1")]
fn a_key_length_of_zero_is_refused() {
    let _ = KeyMemory::new(0, 0.95);
}

#[test]
#[should_panic(expected = "recency_decay must be in (0, 1]")]
fn a_decay_that_is_not_a_number_is_refused() {
    let _ = KeyMemory::new(60, f64::NAN);
}

#[test]
#[should_panic(expected = "min_gram must be at least 1")]
fn n_grams_of_no_scalars_are_refused() {
    let _ = LexicalMemory::new(0, 3);
}

#[test]
#[should_panic(expected = "max_gram must be at least min_gram")]
fn longer_n_grams_shorter_than_the_shorter_are_refused() {
    let _ = LexicalMemory::new(3, 2);
}

#[test]
#[should_panic(expected = "count must be at least 1")]
fn a_lookup_for_no_matches_is_refused() {
    let _ = LexicalMemory::default().nearest("ticket/team", "router", 0);
}

#[test]
#[should_panic(expected = "'nonexistent'")]
fn a_lookup_of_an_unknown_attribute_is_refused_while_memory_is_empty() {
    let schema = Schema::new(
        "ticket",
        vec![
            Attribute::new("component", AttributeRole::Observed),
            Attribute::new("team", AttributeRole::Decided),
        ],
    )
    .expect("a valid schema");
    let known = BTreeMap::new();
    let _ = Lookup::new(&schema, "nonexistent", &known);
}
