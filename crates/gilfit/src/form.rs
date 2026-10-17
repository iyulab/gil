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
    /// How many of the most similar documents vote on the field's value; at least 1.
    pub similar_document_votes: usize,
    /// The vote's score at or above which the similar document layer is trusted; `None` trusts none.
    pub memory_threshold: Option<f64>,
    /// The similarity the nearest document voting for the value must also reach when more than one voted.
    pub memory_similarity_floor: Option<f64>,
    /// The field's coarse level, offered when neither layer answers the value.
    pub coarse: Option<CoarseLevel>,
    /// The values the field may take; `None` is any. A value settled outside them is remembered but never offered.
    pub candidates: Option<Vec<String>>,
    /// Values the field must not miss: the key layer does not answer for a document whose keys have ever had one.
    pub must_catch: Option<Vec<String>>,
    /// The score the first keyed value that begins with the typed text must reach, for one, two, … characters typed;
    /// `None` (or a length beyond the list) trusts none.
    pub typed_key_thresholds: Option<Vec<Option<f64>>>,
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
            similar_document_votes: 10,
            memory_threshold: None,
            memory_similarity_floor: None,
            coarse: None,
            candidates: None,
            must_catch: None,
            typed_key_thresholds: None,
        }
    }

    /// Whether `value` lies in the field's domain: among its candidates when it has them. Domains taken from another
    /// field's values are not yet supported by this implementation.
    pub fn admits(&self, value: &str) -> bool {
        self.candidates
            .as_ref()
            .is_none_or(|c| c.iter().any(|v| v == value))
    }

    /// The key threshold with `typed` characters typed into the field: the key threshold before typing, then the typed
    /// thresholds.
    pub fn key_threshold_for(&self, typed: usize) -> Option<f64> {
        if typed == 0 {
            return self.key_threshold;
        }

        self.typed_key_thresholds
            .as_ref()
            .and_then(|t| t.get(typed - 1).copied().flatten())
    }

    /// The concentration of `key_field`'s keys for this field — 1 unless set.
    pub fn concentration(&self, key_field: &str) -> f64 {
        self.key_concentrations
            .get(key_field)
            .copied()
            .unwrap_or(1.0)
    }
}

/// The coarse level of a judged field: the class of each of its values — its first `prefix` characters, or what a
/// table of `classes` says — and the thresholds at which the class the two layers back is trusted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoarseLevel {
    /// How many leading characters of a value name its class, counted in UTF-16 code units as the reference counts
    /// them; 0 for a level made from a table.
    pub prefix: usize,
    /// Value to class, for a level made from a table; a value it does not list is a class of its own.
    pub classes: Option<BTreeMap<String, String>>,
    /// The summed key scores of a class's values at or above which the keys' class is trusted; `None` trusts none.
    pub key_threshold: Option<f64>,
    /// The vote's margin over classes at or above which the similar documents' class is trusted; `None` trusts none.
    pub memory_threshold: Option<f64>,
    /// The similarity the nearest document voting for the class must also reach when more than one voted.
    pub memory_similarity_floor: Option<f64>,
}

impl CoarseLevel {
    /// A level whose classes are the values' first `prefix` characters.
    pub fn by_prefix(prefix: usize) -> Self {
        Self {
            prefix,
            ..Self::default()
        }
    }

    /// The class of `value`: what the table says, the value itself when it does not list it; without a table, its
    /// first `prefix` UTF-16 code units, or all of it when shorter. A character beyond U+FFFF that the prefix would cut
    /// in half is left out — the reference keeps its first half, which is not text.
    pub fn of(&self, value: &str) -> String {
        if let Some(table) = &self.classes {
            return table
                .get(value)
                .cloned()
                .unwrap_or_else(|| value.to_owned());
        }

        crate::text::utf16_prefix(value, self.prefix).to_owned()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_class_by_prefix_counts_utf16_code_units() {
        let level = |prefix| CoarseLevel::by_prefix(prefix);
        assert_eq!(level(2).of("25-40"), "25");
        assert_eq!(level(2).of("가나다"), "가나");
        assert_eq!(level(9).of("short"), "short");
        assert_eq!(level(2).of("😀a"), "😀"); // one character, two code units
        assert_eq!(level(1).of("😀a"), ""); // the reference would keep half a character
    }

    #[test]
    fn a_class_by_table_is_the_value_itself_when_not_listed() {
        let level = CoarseLevel {
            classes: Some(BTreeMap::from([("K123".to_owned(), "panel".to_owned())])),
            ..CoarseLevel::default()
        };
        assert_eq!(level.of("K123"), "panel");
        assert_eq!(level.of("K999"), "K999");
    }
}
