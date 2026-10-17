//! Forms and settled documents, in the decision core's vocabulary.

use std::collections::BTreeMap;

/// Whether a field is read (observed) or decided (judged).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Observed,
    Judged,
}

/// One field of a form, with the settings that govern how it is suggested.
#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub role: Role,
    /// A set of values rather than one.
    pub multiple: bool,
    /// False keeps the field out of every other field's evidence.
    pub use_as_evidence: bool,
    /// The fields this field's suggestions may rest on; `None` is every other field that is evidence.
    pub depends_on: Option<Vec<String>>,
    /// The fields that key this field in the field memory; `None` is every field it rests on.
    pub key_fields: Option<Vec<String>>,
    /// How readily each key field's keys are believed; a field not named takes 1.
    pub key_concentrations: BTreeMap<String, f64>,
    /// The score the first keyed value must reach to be trusted; `None` trusts none.
    pub key_threshold: Option<f64>,
}

impl Field {
    pub fn new(name: &str, role: Role) -> Self {
        Self {
            name: name.to_owned(),
            role,
            multiple: false,
            use_as_evidence: true,
            depends_on: None,
            key_fields: None,
            key_concentrations: BTreeMap::new(),
            key_threshold: None,
        }
    }

    /// The concentration of `key_field`'s keys for this field — 1 unless set.
    pub fn concentration(&self, key_field: &str) -> f64 {
        self.key_concentrations
            .get(key_field)
            .copied()
            .unwrap_or(1.0)
    }
}

/// A form: its name scopes memory, its fields are in order.
#[derive(Clone, Debug)]
pub struct Form {
    pub name: String,
    pub fields: Vec<Field>,
}

impl Form {
    /// The field named `name`.
    ///
    /// # Panics
    /// When the form has no such field.
    pub fn field(&self, name: &str) -> &Field {
        self.fields
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("form '{}' has no field '{name}'", self.name))
    }

    /// Whether `evidence`'s value may support suggestions for `field`: another field that is evidence and, when the field
    /// declares its dependencies, one of them.
    pub fn supports(&self, evidence: &str, field: &str) -> bool {
        let (source, target) = (self.field(evidence), self.field(field));
        source.name != target.name
            && source.use_as_evidence
            && target
                .depends_on
                .as_ref()
                .is_none_or(|d| d.iter().any(|n| n == evidence))
    }

    /// Whether `evidence`'s value keys `field` in the field memory: a field that supports it and, when the field names
    /// its key fields, one of them.
    pub fn keys(&self, evidence: &str, field: &str) -> bool {
        self.supports(evidence, field)
            && self
                .field(field)
                .key_fields
                .as_ref()
                .is_none_or(|k| k.iter().any(|n| n == evidence))
    }

    /// The `name: value` lines of the fields that support `field`, in the form's order — what a similar document is
    /// remembered and looked up by. A field that takes several values gives a line for each, once each and in scalar
    /// order, so the same set reads the same; a field that takes several is supported by its own values chosen so far.
    pub fn evidence(
        &self,
        field: &str,
        values: &BTreeMap<String, String>,
        sets: &BTreeMap<String, Vec<String>>,
    ) -> String {
        let target = self.field(field);
        let mut lines: Vec<String> = Vec::new();
        for f in self
            .fields
            .iter()
            .filter(|f| self.supports(&f.name, field) || (f.name == field && target.multiple))
        {
            if f.multiple {
                let mut elements: Vec<&String> = sets.get(&f.name).into_iter().flatten().collect();
                elements.sort();
                elements.dedup();
                lines.extend(elements.into_iter().map(|e| format!("{}: {e}", f.name)));
            } else if let Some(value) = values.get(&f.name) {
                lines.push(format!("{}: {value}", f.name));
            }
        }

        lines.join("\n")
    }
}

/// What happened to a judged field when it was settled — the names the reference implementation uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettlementKind {
    /// The suggested value was taken as it was.
    Accept,
    /// A value other than the suggested one was settled.
    Correct,
    /// The suggestion was turned down and no value settled.
    Reject,
    /// The field's earlier settlement was undone.
    Revert,
    /// A saved value put back when a document is reopened.
    Restore,
    /// The values of a field that takes several were settled.
    Set,
    /// A person settled a value with no suggestion in view.
    Enter,
    /// The application settled the suggested value on its own: memory does not learn it, and choosing thresholds does
    /// not ask about it.
    Fill,
    /// A value settled again by someone who did not see the answer, drawn at random with a known probability; choosing
    /// thresholds counts it as one ÷ that probability decisions.
    Check,
}

impl SettlementKind {
    /// The kind named `name`, as the reference implementation spells it; `None` for any other name.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "Accept" => Self::Accept,
            "Correct" => Self::Correct,
            "Reject" => Self::Reject,
            "Revert" => Self::Revert,
            "Restore" => Self::Restore,
            "Set" => Self::Set,
            "Enter" => Self::Enter,
            "Fill" => Self::Fill,
            "Check" => Self::Check,
            _ => return None,
        })
    }
}

/// A document's values as last settled. `settled_at` is in 100-nanosecond ticks since the Unix epoch, UTC.
#[derive(Clone, Debug, Default)]
pub struct Document {
    pub id: String,
    pub values: BTreeMap<String, String>,
    pub sets: BTreeMap<String, Vec<String>>,
    pub settled_at: i64,
    /// How each judged field was settled, where known.
    pub settlements: BTreeMap<String, SettlementKind>,
    /// For a field settled as a check, the probability it was drawn with.
    pub probabilities: BTreeMap<String, f64>,
    /// The fields with a value, in the order their current values arrived, oldest first; a field the list leaves out
    /// counts as there from the start. `None` when the order is unknown: the observed values are then taken as there from
    /// the start and the judged ones as settled in the form's order.
    pub arrival: Option<Vec<String>>,
}

impl Document {
    /// The document as memory remembers it: the values (and sets) the application filled ([`SettlementKind::Fill`]) are
    /// left out, with their settlements.
    pub fn remembered(&self) -> Document {
        let filled: Vec<&String> = self
            .settlements
            .iter()
            .filter(|(_, kind)| **kind == SettlementKind::Fill)
            .map(|(field, _)| field)
            .collect();
        if filled.is_empty() {
            return self.clone();
        }

        let kept = |field: &String| !filled.contains(&field);
        Document {
            values: self
                .values
                .iter()
                .filter(|(f, _)| kept(f))
                .map(|(f, v)| (f.clone(), v.clone()))
                .collect(),
            sets: self
                .sets
                .iter()
                .filter(|(f, _)| kept(f))
                .map(|(f, v)| (f.clone(), v.clone()))
                .collect(),
            settlements: self
                .settlements
                .iter()
                .filter(|(f, _)| kept(f))
                .map(|(f, k)| (f.clone(), *k))
                .collect(),
            ..self.clone()
        }
    }
}
