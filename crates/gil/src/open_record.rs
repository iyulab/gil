//! One record of a schema while its values arrive: observed values and settlements come in as events, and after each the
//! record is put into memory again and the decided attributes whose evidence changed are answered afresh.
//!
//! The state is the record's own — its values, the order they arrived in, how each decided value was settled — and the
//! core keeps no clock: a settlement that makes the record newer carries the time it happened. A settled value can be
//! settled again: accepting an answer and correcting it later leaves only the correction in memory, because the record's
//! contribution is always what its current values imply. Accepting, correcting, entering, filling, checking or setting
//! moves the record's settlement time to the time given; observing, restoring, rejecting and reverting do not. A rejected
//! attribute is not answered again until it is reverted or settled. An attribute that takes several values stays open once
//! settled: the values chosen so far are evidence for the rest, which go on being answered — settling it again with more
//! or fewer values replaces them.

use std::collections::{BTreeMap, BTreeSet};

use crate::decider::{Answer, Decider, ExternalCandidate};
use crate::decision_record::DecisionRecord;
use crate::key_memory::Lookup;
use crate::schema::{
    Attribute, AttributePolicy, AttributeRole, Record, Schema, Settlement, SettlementKind,
};

/// A call [`OpenRecord::observe`] or [`OpenRecord::settle`] would refuse, found before it changes anything: an
/// attribute the schema does not have, or one of the other role, or a settlement of the other kind of attribute. A
/// binding asks first ([`OpenRecord::check_observe`], [`OpenRecord::check_settle`]) so that it can tell this refusal —
/// after which the record and the decider are as they were — from a defect that stopped a change halfway.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractViolation(String);

impl std::fmt::Display for ContractViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ContractViolation {}

/// A record of a schema whose values are arriving. Not thread-safe; the decider it is used with is passed to each call.
#[derive(Clone, Debug)]
pub struct OpenRecord {
    schema: Schema,
    id: String,
    values: BTreeMap<String, String>,
    sets: BTreeMap<String, Vec<String>>,
    rejected: BTreeSet<String>,
    /// The attributes with a value, in the order their current values arrived.
    arrival: Vec<String>,
    settlements: BTreeMap<String, SettlementKind>,
    probabilities: BTreeMap<String, f64>,
    settled_at: i64,
}

impl OpenRecord {
    /// A new record `id` of `schema`, starting at `settled_at` (100-nanosecond ticks since the Unix epoch, UTC) — the
    /// present for a new record, the saved time when the host puts a saved record's values back one by one (prefer
    /// [`OpenRecord::reopen`] for that: it keeps the order the values arrived in and how they were settled).
    ///
    /// # Panics
    /// When `id` is blank.
    pub fn new(schema: Schema, id: &str, settled_at: i64) -> Self {
        assert!(!id.trim().is_empty(), "a record needs an id");
        Self {
            schema,
            id: id.to_owned(),
            values: BTreeMap::new(),
            sets: BTreeMap::new(),
            rejected: BTreeSet::new(),
            arrival: Vec::new(),
            settlements: BTreeMap::new(),
            probabilities: BTreeMap::new(),
            settled_at,
        }
    }

    /// A saved record reopened as its snapshot left it: its values, the order they arrived in, its settlement time and
    /// how each decided value was settled. Memory is not touched until a value changes — the saved record is already
    /// there once memory is rebuilt from the saved records.
    ///
    /// # Panics
    /// When the record's id is blank, or it keeps an attribute where its kind does not belong: a value for an attribute
    /// that takes several, or a set for one that takes one.
    pub fn reopen(schema: Schema, saved: &Record) -> Self {
        if let Err(misplaced) = schema.check_placement(&saved.values, &saved.sets) {
            panic!("{misplaced}");
        }

        let mut open = Self::new(schema, &saved.id, saved.settled_at);
        for (attribute, value) in &saved.values {
            open.values.insert(attribute.clone(), value.clone());
        }

        for (attribute, values) in saved.sets.iter().filter(|(_, v)| !v.is_empty()) {
            open.sets.insert(attribute.clone(), values.clone());
        }

        // The saved order, any attribute it leaves out counting as there from the start, as a replay takes it; with no
        // order saved, the observed values from the start and the decided ones settled in the schema's order.
        let arrival: Vec<String> = saved.arrival.clone().unwrap_or_else(|| {
            let by_role = |role| {
                open.schema
                    .attributes
                    .iter()
                    .filter(move |a| a.role == role)
                    .map(|a| a.name.clone())
            };
            by_role(AttributeRole::Observed)
                .chain(by_role(AttributeRole::Decided))
                .collect()
        });
        let mut present: Vec<String> = Vec::new();
        for name in arrival {
            if open.has(&name) && !present.contains(&name) {
                present.push(name);
            }
        }
        let mut before: Vec<String> = open
            .values
            .keys()
            .chain(open.sets.keys())
            .filter(|a| !present.contains(a))
            .cloned()
            .collect();
        before.sort();
        before.dedup();
        open.arrival.extend(before);
        open.arrival.extend(present);

        for (attribute, kind) in &saved.settlements {
            let decided = open
                .schema
                .attributes
                .iter()
                .any(|a| &a.name == attribute && a.role == AttributeRole::Decided);
            if open.has(attribute) && decided {
                open.settlements.insert(attribute.clone(), *kind);
                if *kind == SettlementKind::Check
                    && let Some(p) = saved.probabilities.get(attribute)
                {
                    open.probabilities.insert(attribute.clone(), *p);
                }
            }
        }

        open
    }

    /// The record's schema.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// The record's id — the key its contribution to memory is kept under.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Sets an observed attribute's value, or clears it with `None`, puts the record into `decider`'s memory again and
    /// returns fresh answers for the open decided attributes it is evidence for.
    ///
    /// # Panics
    /// When the attribute is not an observed attribute of the schema — what [`OpenRecord::check_observe`] tells beforehand.
    pub fn observe(
        &mut self,
        decider: &mut Decider,
        attribute: &str,
        value: Option<&str>,
    ) -> Vec<Answer> {
        if let Err(refused) = self.check_observe(attribute) {
            panic!("{refused}");
        }
        self.set(attribute, value);
        self.changed(decider, attribute, false)
    }

    /// Settles a decided attribute at `now` (100-nanosecond ticks since the Unix epoch, UTC — used only by the kinds that
    /// make the record newer), puts the record into `decider`'s memory again and returns fresh answers for the open
    /// decided attributes whose evidence changed — and for the attribute itself when a revert reopened it.
    ///
    /// # Panics
    /// When the attribute is not a decided attribute of the schema, or the settlement is of the other kind of attribute:
    /// values of a set for a single-valued attribute, or a single value for an attribute that takes several — what
    /// [`OpenRecord::check_settle`] tells beforehand.
    pub fn settle(
        &mut self,
        decider: &mut Decider,
        attribute: &str,
        settlement: &Settlement,
        now: i64,
    ) -> Vec<Answer> {
        if let Err(refused) = self.check_settle(attribute, settlement) {
            panic!("{refused}");
        }
        let clears = matches!(
            settlement.kind,
            SettlementKind::Reject | SettlementKind::Revert
        );
        let multiple = self.schema.attribute(attribute).multiple;

        self.rejected.remove(attribute);
        if clears {
            self.clear(attribute);
            if settlement.kind == SettlementKind::Reject {
                self.rejected.insert(attribute.to_owned());
            }
        } else {
            if multiple {
                let values = settlement.values.clone().unwrap_or_default();
                self.set_many(attribute, values);
            } else {
                self.set(attribute, settlement.value.as_deref());
            }

            if settlement.kind != SettlementKind::Restore {
                self.settled_at = now; // a saved value keeps the saved record's time
            }
        }

        self.probabilities.remove(attribute);
        if clears || settlement.kind == SettlementKind::Restore || !self.has(attribute) {
            self.settlements.remove(attribute); // nothing settled it, or nothing known did
        } else {
            self.settlements
                .insert(attribute.to_owned(), settlement.kind);
            if settlement.kind == SettlementKind::Check {
                self.probabilities
                    .insert(attribute.to_owned(), settlement.probability);
            }
        }

        self.changed(
            decider,
            attribute,
            settlement.kind == SettlementKind::Revert,
        )
    }

    /// Answers for every open decided attribute: not settled (unless it takes several), not rejected, and not off.
    pub fn answer(&self, decider: &Decider) -> Vec<Answer> {
        self.answer_with(decider, &mut |_| {})
    }

    /// As [`OpenRecord::answer`], handing `sink` the decision record of each attribute's lookup, in the answers' order —
    /// what the key layer and the similar record layer found whatever the thresholds, and what was answered — for a host
    /// that keeps them to recount its thresholds later.
    pub fn answer_with(
        &self,
        decider: &Decider,
        sink: &mut dyn FnMut(&DecisionRecord),
    ) -> Vec<Answer> {
        self.schema
            .attributes
            .iter()
            .filter(|a| self.is_open(a))
            .map(|a| decider.answer_one_with(&self.lookup(&a.name, None), sink))
            .collect()
    }

    /// The answer for one open decided attribute alone — the attribute shown, or the one a person is typing into —
    /// without the work of answering the rest. `partial` is the value seen so far (the text typed): the attribute stays
    /// open, as a partial value settles nothing, and the answer offers only values that begin with it (ignoring case),
    /// the key layer held to the threshold for that many characters.
    ///
    /// # Panics
    /// When the attribute is not open: not decided, off, settled or rejected.
    pub fn answer_partial(
        &self,
        decider: &Decider,
        attribute: &str,
        partial: Option<&str>,
    ) -> Answer {
        self.assert_open(attribute);
        decider.answer_one_with(&self.lookup(attribute, partial), &mut |_| {})
    }

    /// The answer for an open decided attribute with an external judgement's ranked values in it — what the host's model
    /// made of the evidence an earlier answer asked for ([`Answer::needs_external`]).
    ///
    /// # Panics
    /// When the attribute is not open: not decided, off, settled or rejected.
    pub fn offer_external(
        &self,
        decider: &Decider,
        attribute: &str,
        ranked: &[ExternalCandidate],
    ) -> Answer {
        self.assert_open(attribute);
        decider.offer_external(&self.lookup(attribute, None), ranked)
    }

    /// The record's values as they stand — observed and settled — which is what the host saves, with the time an
    /// attribute was last settled (or the time the record was opened with, if none was since), the order the values
    /// arrived in, by which choosing thresholds replays the record, and how each decided value was settled — with the
    /// probability a checked one was drawn with.
    pub fn snapshot(&self) -> Record {
        Record {
            id: self.id.clone(),
            values: self.values.clone(),
            sets: self.sets.clone(),
            settled_at: self.settled_at,
            settlements: self.settlements.clone(),
            probabilities: self.probabilities.clone(),
            arrival: Some(self.arrival.clone()),
        }
    }

    fn lookup<'a>(&'a self, attribute: &'a str, partial: Option<&'a str>) -> Lookup<'a> {
        Lookup::new(&self.schema, attribute, &self.values)
            .sets(&self.sets)
            .typed(partial)
            .excluding(Some(&self.id))
            .arrival(Some(&self.arrival))
    }

    /// Whether [`OpenRecord::observe`] would take `attribute`: one the schema has, and observed.
    pub fn check_observe(&self, attribute: &str) -> Result<(), ContractViolation> {
        let definition = self.defined(attribute)?;
        if definition.role != AttributeRole::Observed {
            return Err(ContractViolation(format!(
                "'{attribute}' is decided; settle it instead"
            )));
        }
        Ok(())
    }

    /// Whether [`OpenRecord::settle`] would take `settlement` for `attribute`: one the schema has, decided, and settled
    /// with the kind of value it takes — values of a set for an attribute that takes several, a single value for one
    /// that takes one (rejecting and reverting carry neither).
    pub fn check_settle(
        &self,
        attribute: &str,
        settlement: &Settlement,
    ) -> Result<(), ContractViolation> {
        let definition = self.defined(attribute)?;
        if definition.role != AttributeRole::Decided {
            return Err(ContractViolation(format!(
                "'{attribute}' is observed; observe it instead"
            )));
        }
        let clears = matches!(
            settlement.kind,
            SettlementKind::Reject | SettlementKind::Revert
        );
        if !clears && settlement.is_set() != definition.multiple {
            return Err(ContractViolation(if definition.multiple {
                format!("'{attribute}' takes several values; settle it with Settlement::set")
            } else {
                format!("'{attribute}' takes one value; settle it with a single value")
            }));
        }
        Ok(())
    }

    fn defined(&self, attribute: &str) -> Result<&Attribute, ContractViolation> {
        self.schema
            .find(attribute)
            .ok_or_else(|| ContractViolation(self.schema.no_attribute(attribute)))
    }

    fn assert_open(&self, attribute: &str) {
        assert!(
            self.is_open(self.schema.attribute(attribute)),
            "'{attribute}' is not open: only a decided attribute that is not off, settled or rejected is answered"
        );
    }

    fn is_open(&self, attribute: &Attribute) -> bool {
        attribute.role == AttributeRole::Decided
            && attribute.policy != AttributePolicy::Off
            && (attribute.multiple || !self.values.contains_key(&attribute.name))
            && !self.rejected.contains(&attribute.name)
    }

    fn has(&self, attribute: &str) -> bool {
        self.values.contains_key(attribute) || self.sets.contains_key(attribute)
    }

    fn clear(&mut self, attribute: &str) {
        self.sets.remove(attribute);
        self.set(attribute, None);
    }

    /// A set's values: they arrive together, as one value would — the attribute holds one place in the arrival order.
    fn set_many(&mut self, attribute: &str, values: Vec<String>) {
        self.arrival.retain(|a| a != attribute);
        if values.is_empty() {
            self.sets.remove(attribute);
            return;
        }

        self.sets.insert(attribute.to_owned(), values);
        self.arrival.push(attribute.to_owned());
    }

    fn set(&mut self, attribute: &str, value: Option<&str>) {
        self.arrival.retain(|a| a != attribute);
        match value {
            None => {
                self.values.remove(attribute);
            }
            Some(value) => {
                self.values.insert(attribute.to_owned(), value.to_owned());
                self.arrival.push(attribute.to_owned()); // a changed value is new history: it moves to the end
            }
        }
    }

    fn changed(&self, decider: &mut Decider, changed: &str, reopened: bool) -> Vec<Answer> {
        decider.rebuild(&self.schema, std::slice::from_ref(&self.snapshot()));
        self.schema
            .attributes
            .iter()
            .filter(|a| self.is_open(a))
            // A set's own chosen values are evidence for the rest of it.
            .filter(|a| {
                self.schema.supports(changed, &a.name)
                    || (a.name == changed && (reopened || a.multiple))
            })
            .map(|a| decider.answer_one_with(&self.lookup(&a.name, None), &mut |_| {}))
            .collect()
    }
}
