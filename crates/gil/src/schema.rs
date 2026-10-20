//! Schemas and settled records, in the decision core's vocabulary.

use std::collections::BTreeMap;

/// Whether an attribute is read (observed) or decided (decided).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeRole {
    Observed,
    Decided,
}

/// One attribute of a schema, with the settings that govern how it is suggested.
#[derive(Clone, Debug)]
pub struct Attribute {
    pub name: String,
    pub role: AttributeRole,
    /// A set of values rather than one.
    pub multiple: bool,
    /// False keeps the attribute out of every other attribute's evidence.
    pub use_as_evidence: bool,
    /// The attributes this attribute's suggestions may rest on; `None` is every other attribute that is evidence.
    pub depends_on: Option<Vec<String>>,
    /// The attributes that key this attribute in the key memory; `None` is every attribute it rests on.
    pub key_fields: Option<Vec<String>>,
    /// How readily each key attribute's keys are believed; an attribute not named takes 1.
    pub key_concentrations: BTreeMap<String, f64>,
    /// The score the first keyed value must reach to be trusted; `None` trusts none.
    pub key_threshold: Option<f64>,
    /// How many of the most similar records vote on the attribute's value; at least 1.
    pub similar_votes: usize,
    /// The vote's score at or above which the similar record layer is trusted; `None` trusts none.
    pub memory_threshold: Option<f64>,
    /// The similarity the nearest record voting for the value must also reach when more than one voted.
    pub memory_floor: Option<f64>,
    /// The attribute's coarse level, offered when neither layer answers the value.
    pub coarse: Option<CoarseLevel>,
    /// The values the attribute may take; `None` is any. A value settled outside them is remembered but never offered.
    pub candidates: Option<Vec<String>>,
    /// Values the attribute must not miss: the key layer does not answer for a record whose keys have ever had one.
    pub must_catch: Option<Vec<String>>,
    /// The score the first keyed value that begins with the typed text must reach, for one, two, … characters typed;
    /// `None` (or a length beyond the list) trusts none.
    pub typed_key_thresholds: Option<Vec<Option<f64>>>,
}

impl Attribute {
    pub fn new(name: &str, role: AttributeRole) -> Self {
        Self {
            name: name.to_owned(),
            role,
            multiple: false,
            use_as_evidence: true,
            depends_on: None,
            key_fields: None,
            key_concentrations: BTreeMap::new(),
            key_threshold: None,
            similar_votes: 10,
            memory_threshold: None,
            memory_floor: None,
            coarse: None,
            candidates: None,
            must_catch: None,
            typed_key_thresholds: None,
        }
    }

    /// Whether `value` lies in the attribute's domain: among its candidates when it has them. Domains taken from another
    /// attribute's values are not yet supported by this implementation.
    pub fn admits(&self, value: &str) -> bool {
        self.candidates
            .as_ref()
            .is_none_or(|c| c.iter().any(|v| v == value))
    }

    /// The key threshold with `typed` characters typed into the attribute: the key threshold before typing, then the typed
    /// thresholds.
    pub fn key_threshold_for(&self, typed: usize) -> Option<f64> {
        if typed == 0 {
            return self.key_threshold;
        }

        self.typed_key_thresholds
            .as_ref()
            .and_then(|t| t.get(typed - 1).copied().flatten())
    }

    /// The concentration of `key_field`'s keys for this attribute — 1 unless set.
    pub fn concentration(&self, key_field: &str) -> f64 {
        self.key_concentrations
            .get(key_field)
            .copied()
            .unwrap_or(1.0)
    }
}

/// The coarse level of a decided attribute: the class of each of its values — its first `prefix` characters, or what a
/// table of `classes` says — and the thresholds at which the class the two layers back is trusted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoarseLevel {
    /// How many leading characters (Unicode scalar values) of a value name its class; 0 for a level made from a table.
    pub prefix: usize,
    /// Value to class, for a level made from a table; a value it does not list is a class of its own.
    pub classes: Option<BTreeMap<String, String>>,
    /// The summed key scores of a class's values at or above which the keys' class is trusted; `None` trusts none.
    pub key_threshold: Option<f64>,
    /// The vote's margin over classes at or above which the similar records' class is trusted; `None` trusts none.
    pub memory_threshold: Option<f64>,
    /// The similarity the nearest record voting for the class must also reach when more than one voted.
    pub memory_floor: Option<f64>,
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
    /// first `prefix` Unicode scalar values, or all of it when shorter.
    pub fn of(&self, value: &str) -> String {
        if let Some(table) = &self.classes {
            return table
                .get(value)
                .cloned()
                .unwrap_or_else(|| value.to_owned());
        }

        crate::text::prefix(value, self.prefix).to_owned()
    }
}

/// A schema: its name scopes memory, its attributes are in order.
#[derive(Clone, Debug)]
pub struct Schema {
    pub name: String,
    pub attributes: Vec<Attribute>,
}

impl Schema {
    /// The attribute named `name`.
    ///
    /// # Panics
    /// When the schema has no such attribute.
    pub fn attribute(&self, name: &str) -> &Attribute {
        self.attributes
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("schema '{}' has no attribute '{name}'", self.name))
    }

    /// Whether `evidence`'s value may support suggestions for `attribute`: another attribute that is evidence and, when the attribute
    /// declares its dependencies, one of them.
    pub fn supports(&self, evidence: &str, attribute: &str) -> bool {
        let (source, target) = (self.attribute(evidence), self.attribute(attribute));
        source.name != target.name
            && source.use_as_evidence
            && target
                .depends_on
                .as_ref()
                .is_none_or(|d| d.iter().any(|n| n == evidence))
    }

    /// Whether `evidence`'s value keys `attribute` in the key memory: an attribute that supports it and, when the attribute names
    /// its key attributes, one of them.
    pub fn keys(&self, evidence: &str, attribute: &str) -> bool {
        self.supports(evidence, attribute)
            && self
                .attribute(attribute)
                .key_fields
                .as_ref()
                .is_none_or(|k| k.iter().any(|n| n == evidence))
    }

    /// The `name: value` lines of the attributes that support `attribute`, in the schema's order — what a similar record is
    /// remembered and looked up by. An attribute that takes several values gives a line for each, once each and in scalar
    /// order, so the same set reads the same; an attribute that takes several is supported by its own values chosen so far.
    pub fn evidence(
        &self,
        attribute: &str,
        values: &BTreeMap<String, String>,
        sets: &BTreeMap<String, Vec<String>>,
    ) -> String {
        let target = self.attribute(attribute);
        let mut lines: Vec<String> = Vec::new();
        for f in self.attributes.iter().filter(|f| {
            self.supports(&f.name, attribute) || (f.name == attribute && target.multiple)
        }) {
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

/// What happened to a decided attribute when it was settled — the names the reference implementation uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettlementKind {
    /// The suggested value was taken as it was.
    Accept,
    /// A value other than the suggested one was settled.
    Correct,
    /// The suggestion was turned down and no value settled.
    Reject,
    /// The attribute's earlier settlement was undone.
    Revert,
    /// A saved value put back when a record is reopened.
    Restore,
    /// The values of an attribute that takes several were settled.
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

/// A record's values as last settled. `settled_at` is in 100-nanosecond ticks since the Unix epoch, UTC.
#[derive(Clone, Debug, Default)]
pub struct Record {
    pub id: String,
    pub values: BTreeMap<String, String>,
    pub sets: BTreeMap<String, Vec<String>>,
    pub settled_at: i64,
    /// How each decided attribute was settled, where known.
    pub settlements: BTreeMap<String, SettlementKind>,
    /// For an attribute settled as a check, the probability it was drawn with.
    pub probabilities: BTreeMap<String, f64>,
    /// The attributes with a value, in the order their current values arrived, oldest first; an attribute the list leaves out
    /// counts as there from the start. `None` when the order is unknown: the observed values are then taken as there from
    /// the start and the decided ones as settled in the schema's order.
    pub arrival: Option<Vec<String>>,
}

impl Record {
    /// The record as memory remembers it: the values (and sets) the application filled ([`SettlementKind::Fill`]) are
    /// left out, with their settlements.
    pub fn remembered(&self) -> Record {
        let filled: Vec<&String> = self
            .settlements
            .iter()
            .filter(|(_, kind)| **kind == SettlementKind::Fill)
            .map(|(attribute, _)| attribute)
            .collect();
        if filled.is_empty() {
            return self.clone();
        }

        let kept = |attribute: &String| !filled.contains(&attribute);
        Record {
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
    fn a_class_by_prefix_counts_scalars() {
        let level = |prefix| CoarseLevel::by_prefix(prefix);
        assert_eq!(level(2).of("25-40"), "25");
        assert_eq!(level(2).of("가나다"), "가나");
        assert_eq!(level(9).of("short"), "short");
        assert_eq!(level(1).of("😀a"), "😀"); // one character beyond U+FFFF is one scalar
        assert_eq!(level(2).of("a😀b"), "a😀");
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
