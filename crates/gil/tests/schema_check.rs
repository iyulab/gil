//! `Schema::check` refuses what the reference implementation refuses when a form is defined — names, dependencies, the
//! settings only a decided single-valued attribute takes, those a set-valued attribute does not, and a coarse level's
//! prefix or table — and names the attribute and the rule it breaks.

use gil::schema::{Attribute, AttributeRole, CoarseLevel, Schema};

fn schema(attributes: Vec<Attribute>) -> Schema {
    Schema {
        name: "form".into(),
        attributes,
    }
}

fn refused(attributes: Vec<Attribute>) -> String {
    schema(attributes).check().expect_err("a refused schema")
}

fn decided(name: &str) -> Attribute {
    Attribute::new(name, AttributeRole::Decided)
}

fn observed(name: &str) -> Attribute {
    Attribute::new(name, AttributeRole::Observed)
}

fn set(name: &str) -> Attribute {
    let mut a = decided(name);
    a.multiple = true;
    a
}

fn with(mut a: Attribute, edit: impl FnOnce(&mut Attribute)) -> Attribute {
    edit(&mut a);
    a
}

fn table(entries: &[(&str, &str)]) -> CoarseLevel {
    CoarseLevel {
        classes: Some(
            entries
                .iter()
                .map(|(v, c)| ((*v).to_owned(), (*c).to_owned()))
                .collect(),
        ),
        ..CoarseLevel::default()
    }
}

#[test]
fn contradictory_or_dangling_declarations_are_refused() {
    assert!(
        refused(vec![decided("team"), decided("team")]).contains("'team' appears more than once")
    );
    assert!(refused(vec![observed("component")]).contains("at least one decided attribute"));

    let depending = |on: &str| with(decided("team"), |a| a.depends_on = Some(vec![on.into()]));
    assert!(
        refused(vec![depending("component")])
            .contains("'team' depends on 'component', which is not another attribute")
    );
    assert!(
        refused(vec![depending("team")])
            .contains("'team' depends on 'team', which is not another attribute")
    );
    let reporter = with(observed("reporter"), |a| a.use_as_evidence = false);
    assert!(
        refused(vec![reporter, depending("reporter")])
            .contains("'team' depends on 'reporter', which is not evidence")
    );
    assert_eq!(
        schema(vec![observed("component"), depending("component")]).check(),
        Ok(())
    );
}

#[test]
fn a_schema_and_each_of_its_attributes_need_a_name() {
    let mut unnamed = schema(vec![decided("team")]);
    unnamed.name = " ".into();
    assert_eq!(unnamed.check(), Err("a schema needs a name".into()));

    for blank in ["", "  "] {
        assert_eq!(
            schema(vec![observed(blank), decided("team")]).check(),
            Err("an attribute needs a name".into()),
            "{blank:?}"
        );
    }
}

#[test]
fn only_a_decided_attribute_takes_several_values() {
    let tags = with(observed("tags"), |a| a.multiple = true);
    assert!(refused(vec![tags, decided("lead")]).contains("'tags' is observed"));
    assert_eq!(schema(vec![set("tags"), decided("lead")]).check(), Ok(()));
}

#[test]
fn a_set_takes_no_memory_threshold_and_no_typed_key_thresholds() {
    let remembered = with(set("tags"), |a| a.memory_threshold = Some(0.5));
    let error = refused(vec![remembered]);
    assert!(
        error.contains("'tags' takes several values") && error.contains("no memory threshold"),
        "{error}"
    );

    let typed = |a: Attribute| with(a, |a| a.typed_key_thresholds = Some(vec![Some(0.3)]));
    assert!(refused(vec![typed(set("tags"))]).contains("takes no typed key thresholds"));
    assert_eq!(schema(vec![typed(decided("lead"))]).check(), Ok(()));
}

#[test]
fn a_class_needs_one_character_and_a_single_valued_decided_attribute() {
    let coarse =
        |a: Attribute, prefix: usize| with(a, |a| a.coarse = Some(CoarseLevel::by_prefix(prefix)));
    assert_eq!(schema(vec![coarse(decided("code"), 2)]).check(), Ok(()));

    assert!(
        refused(vec![coarse(decided("code"), 0)])
            .contains("'code' gives its coarse level a prefix of 0 characters; at least one")
    );
    assert!(refused(vec![coarse(set("t"), 1)]).contains(
        "'t' takes a coarse level, which only a decided attribute with a single value has"
    ));
    assert!(
        refused(vec![coarse(observed("o"), 1), decided("j")]).contains(
            "'o' takes a coarse level, which only a decided attribute with a single value has"
        )
    );
}

#[test]
fn a_table_of_classes_needs_a_value_and_no_blank_class() {
    let coarse = |level: CoarseLevel| with(decided("code"), |a| a.coarse = Some(level));
    // A table needs no prefix.
    assert_eq!(
        schema(vec![coarse(table(&[("2710", "flight controls")]))]).check(),
        Ok(())
    );

    assert!(
        refused(vec![coarse(table(&[]))])
            .contains("'code' gives its coarse level a table of no classes; at least one value")
    );
    for blank in ["", " "] {
        let error = refused(vec![coarse(table(&[
            ("2700", "flight controls"),
            ("2710", blank),
        ]))]);
        assert!(
            error.contains("'code' gives value '2710' a blank class"),
            "{blank:?}: {error}"
        );
    }
}

#[test]
fn only_a_single_valued_decided_attribute_names_values_to_catch() {
    let catching = |a: Attribute| with(a, |a| a.must_catch = Some(vec!["3".into()]));
    let message =
        "names values it must not miss, which only a decided attribute with a single value has";

    assert!(refused(vec![catching(observed("template")), decided("risk")]).contains(message));
    assert!(refused(vec![observed("template"), catching(set("tags"))]).contains(message));
    assert_eq!(
        schema(vec![observed("template"), catching(decided("risk"))]).check(),
        Ok(())
    );
    // Naming no value is naming none.
    let none = with(observed("template"), |a| a.must_catch = Some(vec![]));
    assert_eq!(schema(vec![none, decided("risk")]).check(), Ok(()));
}
