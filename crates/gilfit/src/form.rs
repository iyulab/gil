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
}

/// A document's values as last settled. `settled_at` is in 100-nanosecond ticks since the Unix epoch, UTC.
#[derive(Clone, Debug)]
pub struct Document {
    pub id: String,
    pub values: BTreeMap<String, String>,
    pub sets: BTreeMap<String, Vec<String>>,
    pub settled_at: i64,
}
