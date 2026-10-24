//! Schemas and settled records, in the decision core's vocabulary.

use std::collections::BTreeMap;

use crate::replay::LayerThresholds;

/// Whether an attribute is read (observed) or decided (decided).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum AttributeRole {
    Observed,
    Decided,
}

/// How answers for a decided attribute are offered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum AttributePolicy {
    /// Answer when a layer is confident enough.
    #[default]
    Suggest,
    /// Answer, but mark the attribute so that the host never fills it on its own — for attributes where a wrong value
    /// accepted without a look costs more than typing it.
    ConfirmRequired,
    /// Never answer — for attributes a model or a memory should not decide, such as an assessment of a person.
    Off,
}

/// One attribute of a schema, with the settings that govern how it is answered.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Attribute {
    pub name: String,
    pub role: AttributeRole,
    /// A set of values rather than one.
    #[cfg_attr(feature = "serde", serde(default))]
    pub multiple: bool,
    /// False keeps the attribute out of every other attribute's evidence.
    #[cfg_attr(feature = "serde", serde(default = "crate::schema::yes"))]
    pub use_as_evidence: bool,
    /// The attributes this attribute's suggestions may rest on; `None` is every other attribute that is evidence.
    #[cfg_attr(feature = "serde", serde(default))]
    pub depends_on: Option<Vec<String>>,
    /// The attributes that key this attribute in the key memory; `None` is every attribute it rests on.
    #[cfg_attr(feature = "serde", serde(default))]
    pub key_fields: Option<Vec<String>>,
    /// How readily each key attribute's keys are believed; an attribute not named takes 1.
    #[cfg_attr(feature = "serde", serde(default))]
    pub key_concentrations: BTreeMap<String, f64>,
    /// The score the first keyed value must reach to be trusted; `None` trusts none.
    #[cfg_attr(feature = "serde", serde(default))]
    pub key_threshold: Option<f64>,
    /// How many of the most similar records vote on the attribute's value; at least 1.
    #[cfg_attr(feature = "serde", serde(default = "crate::schema::ten"))]
    pub similar_votes: usize,
    /// The vote's score at or above which the similar record layer is trusted; `None` trusts none.
    #[cfg_attr(feature = "serde", serde(default))]
    pub memory_threshold: Option<f64>,
    /// The similarity the nearest record voting for the value must also reach when more than one voted.
    #[cfg_attr(feature = "serde", serde(default))]
    pub memory_floor: Option<f64>,
    /// The attribute's coarse level, offered when neither layer answers the value.
    #[cfg_attr(feature = "serde", serde(default))]
    pub coarse: Option<CoarseLevel>,
    /// Experimental. The value layers' thresholds for the first record of a batch — one whose batch has no settled record
    /// yet ([`Record::batch`]) — chosen apart from the rest ([`crate::keeper::Keep::first_of_batch`]); `None` answers
    /// every record at the thresholds above. [`Schema::for_first_of_batch`] puts them in place.
    #[cfg_attr(feature = "serde", serde(default))]
    pub first_of_batch: Option<FirstOfBatch>,
    /// The values the attribute may take; `None` is any. A value settled outside them is remembered but never offered.
    #[cfg_attr(feature = "serde", serde(default))]
    pub candidates: Option<Vec<String>>,
    /// Another attribute of the schema whose value in the same record bounds this attribute's domain — for an attribute
    /// that takes several values, the values chosen in it: the main topic among a record's topics, a subcategory within
    /// its category. Once that attribute has a value, no layer offers this one a value outside it, as with `candidates`,
    /// which still applies; while it is empty, the domain is not narrowed. A value settled outside it is still remembered,
    /// and offered for records whose domain admits it.
    #[cfg_attr(feature = "serde", serde(default))]
    pub candidates_from: Option<String>,
    /// Values the attribute must not miss: the key layer does not answer for a record whose keys have ever had one.
    #[cfg_attr(feature = "serde", serde(default))]
    pub must_catch: Option<Vec<String>>,
    /// The score the first keyed value that begins with the typed text must reach, for one, two, … characters typed;
    /// `None` (or a length beyond the list) trusts none.
    #[cfg_attr(feature = "serde", serde(default))]
    pub typed_key_thresholds: Option<Vec<Option<f64>>>,
    /// How answers for the attribute are offered; an open record never answers an attribute that is off.
    #[cfg_attr(feature = "serde", serde(default))]
    pub policy: AttributePolicy,
}

#[cfg(feature = "serde")]
fn yes() -> bool {
    true
}

#[cfg(feature = "serde")]
fn ten() -> usize {
    10
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
            first_of_batch: None,
            candidates: None,
            candidates_from: None,
            must_catch: None,
            typed_key_thresholds: None,
            policy: AttributePolicy::Suggest,
        }
    }

    /// Whether `value` lies in the attribute's domain whatever the record: among its candidates when it has them.
    pub fn admits(&self, value: &str) -> bool {
        self.candidates
            .as_ref()
            .is_none_or(|c| c.iter().any(|v| v == value))
    }

    /// Whether `value` lies in the attribute's domain for a record with `values` and `sets` so far: among its candidates
    /// when it has them, and among the values of [`Attribute::candidates_from`] when that attribute has any (ordinally).
    pub fn admits_in(
        &self,
        value: &str,
        values: &BTreeMap<String, String>,
        sets: &BTreeMap<String, Vec<String>>,
    ) -> bool {
        if !self.admits(value) {
            return false;
        }

        let Some(from) = &self.candidates_from else {
            return true;
        };
        if let Some(chosen) = sets.get(from).filter(|c| !c.is_empty()) {
            return chosen.iter().any(|c| c == value);
        }

        values
            .get(from)
            .is_none_or(|given| given.is_empty() || given == value)
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, deny_unknown_fields))]
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

/// Experimental. A decided attribute's value-layer thresholds for the first record of a batch — the same three an
/// attribute holds for every other record.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, deny_unknown_fields))]
pub struct FirstOfBatch {
    /// The score the first keyed value must reach to be trusted; `None` trusts none.
    pub key_threshold: Option<f64>,
    /// The vote's score at or above which the similar record layer is trusted; `None` trusts none.
    pub memory_threshold: Option<f64>,
    /// The similarity the nearest record voting for the value must also reach when more than one voted.
    pub memory_floor: Option<f64>,
}

/// A schema: its name scopes memory, its attributes are in order. A schema is checked when it is made
/// ([`Schema::new`], or deserializing one), so every schema in hand keeps the rules.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Schema {
    name: String,
    attributes: Vec<Attribute>,
}

/// A schema as it reads, before it is checked.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaParts {
    name: String,
    attributes: Vec<Attribute>,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Schema {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let parts = SchemaParts::deserialize(deserializer)?;
        Schema::new(parts.name, parts.attributes).map_err(serde::de::Error::custom)
    }
}

impl Schema {
    /// A schema named `name` with `attributes` in order, checked when it is made: a name, unique attribute names, at least
    /// one decided attribute, and settings that fit their attribute —
    /// several values only for a decided attribute and then no memory threshold or typed key thresholds, at least one
    /// similar-record vote, a coarse level (with a prefix of at least one, or a table of at least one value and no blank
    /// class) and values to catch only on a decided single-valued attribute, dependencies and key attributes that name
    /// other attributes that are evidence (key attributes among the dependencies when those are given), key
    /// concentrations for known attributes that are finite and above 0, and candidates taken from another attribute. The
    /// error names the first rule broken.
    pub fn new(name: impl Into<String>, attributes: Vec<Attribute>) -> Result<Self, String> {
        let schema = Self {
            name: name.into(),
            attributes,
        };
        schema.check()?;
        Ok(schema)
    }

    /// The schema's name, which scopes memory.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The schema's attributes, in order.
    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }

    /// The schema with what a layers replay chose ([`crate::replay::select_layers`]) put on the decided attribute
    /// `attribute`: the key layer's threshold as its key threshold, the similar record layer's as its memory threshold
    /// with the similarity floor as its memory floor. A layer the replay chose no threshold for is left unset, so it does
    /// not answer the attribute. The error names an attribute the schema does not have or that is not decided, or the
    /// first rule the result breaks ([`Schema::new`]).
    pub fn with_layers(&self, attribute: &str, layers: &LayerThresholds) -> Result<Self, String> {
        let (key, memory, floor) = chosen(layers);
        self.with_decided(attribute, |a| {
            a.key_threshold = key;
            a.memory_threshold = memory;
            a.memory_floor = floor;
            Ok(())
        })
    }

    /// As [`Schema::with_layers`], for what a coarse replay chose ([`crate::replay::select_coarse`]): the thresholds go
    /// on the attribute's coarse level. The error also names an attribute with no coarse level.
    pub fn with_coarse(&self, attribute: &str, layers: &LayerThresholds) -> Result<Self, String> {
        let (key, memory, floor) = chosen(layers);
        self.with_decided(attribute, |a| {
            let level = a
                .coarse
                .as_mut()
                .ok_or_else(|| format!("attribute '{}' has no coarse level", a.name))?;
            level.key_threshold = key;
            level.memory_threshold = memory;
            level.memory_floor = floor;
            Ok(())
        })
    }

    /// Experimental. As [`Schema::with_layers`], for what a replay of the first records of batches chose
    /// ([`crate::keeper::Keep::first_of_batch`]): the thresholds go on the attribute's [`Attribute::first_of_batch`],
    /// which they make. The error is as [`Schema::with_layers`]'s.
    pub fn with_first_of_batch(
        &self,
        attribute: &str,
        layers: &LayerThresholds,
    ) -> Result<Self, String> {
        let (key_threshold, memory_threshold, memory_floor) = chosen(layers);
        self.with_decided(attribute, |a| {
            a.first_of_batch = Some(FirstOfBatch {
                key_threshold,
                memory_threshold,
                memory_floor,
            });
            Ok(())
        })
    }

    /// Experimental. Whether an attribute holds thresholds for the first record of a batch.
    pub fn has_first_of_batch(&self) -> bool {
        self.attributes.iter().any(|a| a.first_of_batch.is_some())
    }

    /// Experimental. The schema a first record of a batch is answered with: each attribute's [`Attribute::first_of_batch`]
    /// key and memory thresholds in place of its value layers' — its thresholds for typed text as they are — and the
    /// attributes without them as they are.
    pub fn for_first_of_batch(&self) -> Self {
        let mut schema = self.clone();
        for a in &mut schema.attributes {
            if let Some(first) = &a.first_of_batch {
                a.key_threshold = first.key_threshold;
                a.memory_threshold = first.memory_threshold;
                a.memory_floor = first.memory_floor;
            }
        }
        debug_assert_eq!(schema.check(), Ok(()), "a derived schema stays valid");
        schema
    }

    /// The schema with `change` made to the decided attribute `attribute`, checked.
    fn with_decided(
        &self,
        attribute: &str,
        change: impl FnOnce(&mut Attribute) -> Result<(), String>,
    ) -> Result<Self, String> {
        let mut attributes = self.attributes.clone();
        let a = attributes
            .iter_mut()
            .find(|a| a.name == attribute)
            .ok_or_else(|| self.no_attribute(attribute))?;
        if a.role != AttributeRole::Decided {
            return Err(format!("attribute '{attribute}' is not decided"));
        }
        change(a)?;
        Self::new(self.name.clone(), attributes)
    }

    /// The schema with `change` made to the attribute named `attribute`, unchecked — only for a change that keeps the
    /// schema valid (a debug build checks it). A change that might not goes through [`Schema::new`].
    pub(crate) fn derived(&self, attribute: &str, change: impl FnOnce(&mut Attribute)) -> Self {
        let mut schema = self.clone();
        if let Some(a) = schema.attributes.iter_mut().find(|a| a.name == attribute) {
            change(a);
        }
        debug_assert_eq!(schema.check(), Ok(()), "a derived schema stays valid");
        schema
    }

    /// The rules [`Schema::new`] checks.
    fn check(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("a schema needs a name".into());
        }

        let mut seen = std::collections::BTreeSet::new();
        for a in &self.attributes {
            if a.name.trim().is_empty() {
                return Err("an attribute needs a name".into());
            }
            if !seen.insert(a.name.as_str()) {
                return Err(format!("attribute '{}' appears more than once", a.name));
            }
        }

        if !self
            .attributes
            .iter()
            .any(|a| a.role == AttributeRole::Decided)
        {
            return Err("a schema needs at least one decided attribute".into());
        }

        let named = |name: &str| self.attributes.iter().find(|a| a.name == name);
        for a in &self.attributes {
            let n = &a.name;
            let decided_one = a.role == AttributeRole::Decided && !a.multiple;
            if a.multiple && a.role != AttributeRole::Decided {
                return Err(format!(
                    "attribute '{n}' is observed; only a decided attribute can take several values"
                ));
            }
            if a.multiple && a.memory_threshold.is_some() {
                return Err(format!(
                    "attribute '{n}' takes several values; the similar record layer does not answer it, so it takes no memory threshold"
                ));
            }
            if a.similar_votes < 1 {
                return Err(format!(
                    "attribute '{n}' gives no similar record a vote; at least one must"
                ));
            }
            if let Some(level) = &a.coarse {
                if level.prefix < 1 && level.classes.is_none() {
                    return Err(format!(
                        "attribute '{n}' gives its coarse level a prefix of 0 characters; at least one"
                    ));
                }
                if level.classes.as_ref().is_some_and(BTreeMap::is_empty) {
                    return Err(format!(
                        "attribute '{n}' gives its coarse level a table of no classes; at least one value"
                    ));
                }
                if let Some((v, _)) = level
                    .classes
                    .iter()
                    .flatten()
                    .find(|(_, class)| class.trim().is_empty())
                {
                    return Err(format!(
                        "attribute '{n}' gives value '{v}' a blank class in its coarse level"
                    ));
                }
                if !decided_one {
                    return Err(format!(
                        "attribute '{n}' takes a coarse level, which only a decided attribute with a single value has"
                    ));
                }
            }
            if a.first_of_batch.is_some() && !decided_one {
                return Err(format!(
                    "attribute '{n}' takes thresholds for the first record of a batch, which only a decided attribute with a single value has"
                ));
            }
            if a.must_catch.as_ref().is_some_and(|m| !m.is_empty()) && !decided_one {
                return Err(format!(
                    "attribute '{n}' names values it must not miss, which only a decided attribute with a single value has"
                ));
            }
            if a.multiple && a.typed_key_thresholds.is_some() {
                return Err(format!(
                    "attribute '{n}' takes several values; typed text narrows its candidates but takes no typed key thresholds"
                ));
            }
            for d in a.depends_on.iter().flatten() {
                match named(d) {
                    Some(other) if d != n && other.use_as_evidence => {}
                    Some(other) if d != n && !other.use_as_evidence => {
                        return Err(format!(
                            "attribute '{n}' depends on '{d}', which is not evidence"
                        ));
                    }
                    _ => {
                        return Err(format!(
                            "attribute '{n}' depends on '{d}', which is not another attribute of the schema"
                        ));
                    }
                }
            }
            for k in a.key_fields.iter().flatten() {
                let rests = named(k).is_some_and(|other| {
                    k != n
                        && other.use_as_evidence
                        && a.depends_on.as_ref().is_none_or(|d| d.contains(k))
                });
                if !rests {
                    return Err(format!(
                        "attribute '{n}' is keyed by '{k}', which is not an attribute it rests on"
                    ));
                }
            }
            for (k, c) in &a.key_concentrations {
                if named(k).is_none() || !c.is_finite() || *c <= 0.0 {
                    return Err(format!(
                        "attribute '{n}' has a key concentration for '{k}' that is not an attribute of the schema with a finite concentration above 0"
                    ));
                }
            }
            if let Some(from) = &a.candidates_from
                && (from == n || named(from).is_none())
            {
                return Err(format!(
                    "attribute '{n}' takes its candidates from '{from}', which is not another attribute of the schema"
                ));
            }
        }

        Ok(())
    }

    /// Checks that `values` and `sets` keep each attribute where its kind belongs: no single value for an attribute that
    /// takes several, no set for one that takes one. Values of attributes the schema does not have are not checked. The
    /// error names the first attribute out of place.
    pub fn check_placement(
        &self,
        values: &BTreeMap<String, String>,
        sets: &BTreeMap<String, Vec<String>>,
    ) -> Result<(), String> {
        for a in &self.attributes {
            if a.multiple && values.contains_key(&a.name) {
                return Err(format!(
                    "'{}' takes several values; settle them in sets, not values",
                    a.name
                ));
            }
            if !a.multiple && sets.contains_key(&a.name) {
                return Err(format!(
                    "'{}' takes one value; settle it in values, not sets",
                    a.name
                ));
            }
        }
        Ok(())
    }

    /// The attribute named `name`.
    ///
    /// # Panics
    /// When the schema has no such attribute.
    pub fn attribute(&self, name: &str) -> &Attribute {
        self.find(name)
            .unwrap_or_else(|| panic!("{}", self.no_attribute(name)))
    }

    /// The attribute named `name`, if the schema has one.
    pub fn find(&self, name: &str) -> Option<&Attribute> {
        self.attributes.iter().find(|f| f.name == name)
    }

    /// What [`Schema::attribute`] says of a name the schema does not have.
    pub(crate) fn no_attribute(&self, name: &str) -> String {
        format!("schema '{}' has no attribute '{name}'", self.name)
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
        self.evidence_values(attribute, values, sets)
            .iter()
            .map(|(name, value)| format!("{name}: {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The (attribute, value) pairs [`Schema::evidence`] writes as lines, in the same order.
    pub fn evidence_values(
        &self,
        attribute: &str,
        values: &BTreeMap<String, String>,
        sets: &BTreeMap<String, Vec<String>>,
    ) -> Vec<(String, String)> {
        let target = self.attribute(attribute);
        let mut pairs: Vec<(String, String)> = Vec::new();
        for a in self.attributes.iter().filter(|a| {
            self.supports(&a.name, attribute) || (a.name == attribute && target.multiple)
        }) {
            if a.multiple {
                let mut elements: Vec<&String> = sets.get(&a.name).into_iter().flatten().collect();
                elements.sort();
                elements.dedup();
                pairs.extend(elements.into_iter().map(|e| (a.name.clone(), e.clone())));
            } else if let Some(value) = values.get(&a.name) {
                pairs.push((a.name.clone(), value.clone()));
            }
        }

        pairs
    }
}

/// What happened to a decided attribute when it was settled — the names every binding uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// A person kept a value suggested by something other than this decider — the host's own model, a rule, a default —
    /// shown in place of an answer. Memory learns it and choosing thresholds counts it as it counts the others; it says
    /// nothing about the answers, none having been in view. A value the host's model gave the decider as an external
    /// judgement ([`crate::decider::Decider::offer_external`]) comes back in the answer: taking that answer is an
    /// [`SettlementKind::Accept`].
    AcceptOther,
    /// A person settled another value than one suggested by something other than this decider, shown in place of an
    /// answer — as [`SettlementKind::AcceptOther`], with the suggestion changed.
    CorrectOther,
}

impl SettlementKind {
    /// The kind named `name`, as every binding spells it; `None` for any other name.
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
            "AcceptOther" => Self::AcceptOther,
            "CorrectOther" => Self::CorrectOther,
            _ => return None,
        })
    }
}

/// A settlement of one decided attribute. Accepting is not the same as being right: settling the attribute again later
/// corrects it. An attribute that takes several values is settled with [`Settlement::set`], restored with
/// [`Settlement::restore_set`], and rejected or reverted as any other.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Settlement {
    pub kind: SettlementKind,
    /// The settled value of a single-valued attribute; `None` for a reject, a revert and settlements of a set.
    pub value: Option<String>,
    /// The settled values of an attribute that takes several, once each and in scalar order; `None` otherwise.
    pub values: Option<Vec<String>>,
    /// For a check, the probability the decision was drawn to be checked with, fixed before the draw; 1 otherwise.
    pub probability: f64,
    /// What was in view when the attribute was settled, which the record keeps; `None` when the host does not say.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub in_view: Option<InView>,
}

/// What was in view when a decided attribute was settled: the value suggested, the layer of this decider's answer it
/// was (`None` for a suggestion that was not this decider's — the host's own, a default) and the precision promised for
/// it when it was shown. An open record keeps them on the attribute as [`Record::suggested`], [`Record::sources`] and
/// [`Record::promises`], which the usage counts read.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct InView {
    pub value: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub source: Option<crate::decider::Source>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub promise: Option<f64>,
}

impl Settlement {
    /// The settlement with `answer` in view when it answered — its first candidate, the layer it came from and
    /// `promise`, the precision promised for that layer ([`crate::tuning::Tuning::promise_of`]); unchanged when it
    /// abstained.
    pub fn with_answer(mut self, answer: &crate::decider::Answer, promise: Option<f64>) -> Self {
        if answer.answered() {
            self.in_view = Some(InView {
                value: answer.candidates[0].value.clone(),
                source: Some(answer.source),
                promise,
            });
        }
        self
    }

    /// The settlement with a suggestion in view that was not this decider's — what [`SettlementKind::AcceptOther`] and
    /// [`SettlementKind::CorrectOther`] settle over.
    pub fn with_suggestion(mut self, value: &str) -> Self {
        self.in_view = Some(InView {
            value: value.to_owned(),
            source: None,
            promise: None,
        });
        self
    }

    fn one(kind: SettlementKind, value: &str) -> Self {
        Self {
            kind,
            value: Some(value.to_owned()),
            values: None,
            probability: 1.0,
            in_view: None,
        }
    }

    fn many<'a>(kind: SettlementKind, values: impl IntoIterator<Item = &'a str>) -> Self {
        let mut values: Vec<String> = values.into_iter().map(str::to_owned).collect();
        values.sort();
        values.dedup();
        Self {
            kind,
            value: None,
            values: Some(values),
            probability: 1.0,
            in_view: None,
        }
    }

    fn none(kind: SettlementKind) -> Self {
        Self {
            kind,
            value: None,
            values: None,
            probability: 1.0,
            in_view: None,
        }
    }

    /// The answered value, taken as it was.
    pub fn accept(value: &str) -> Self {
        Self::one(SettlementKind::Accept, value)
    }

    /// A value other than the answered one.
    pub fn correct(value: &str) -> Self {
        Self::one(SettlementKind::Correct, value)
    }

    /// A value suggested by something other than this decider, shown in place of an answer and kept by a person.
    pub fn accept_other(value: &str) -> Self {
        Self::one(SettlementKind::AcceptOther, value)
    }

    /// A value a person settled in place of one suggested by something other than this decider, shown in place of an
    /// answer.
    pub fn correct_other(value: &str) -> Self {
        Self::one(SettlementKind::CorrectOther, value)
    }

    /// A value a person settled with no answer in view.
    pub fn enter(value: &str) -> Self {
        Self::one(SettlementKind::Enter, value)
    }

    /// The answered value, settled by the host with no person confirming it: memory does not learn it.
    pub fn fill(value: &str) -> Self {
        Self::one(SettlementKind::Fill, value)
    }

    /// A saved value put back when a record is reopened; it keeps the record's time.
    pub fn restore(value: &str) -> Self {
        Self::one(SettlementKind::Restore, value)
    }

    /// A value settled again by someone who did not see the answer, the decision having been drawn to be checked with
    /// `probability` — fixed before the draw. A decision picked because it looked doubtful is not a check.
    ///
    /// # Panics
    /// When `probability` is not above 0 and at most 1.
    pub fn check(value: &str, probability: f64) -> Self {
        assert!(
            probability > 0.0 && probability <= 1.0,
            "a check is drawn with a probability above 0 and at most 1"
        );
        Self {
            probability,
            ..Self::one(SettlementKind::Check, value)
        }
    }

    /// Settles an attribute that takes several values with these — none at all clears it. Order and repeats carry no
    /// meaning.
    pub fn set<'a>(values: impl IntoIterator<Item = &'a str>) -> Self {
        Self::many(SettlementKind::Set, values)
    }

    /// Puts back the saved values of an attribute that takes several, as [`Settlement::restore`] does for one.
    pub fn restore_set<'a>(values: impl IntoIterator<Item = &'a str>) -> Self {
        Self::many(SettlementKind::Restore, values)
    }

    /// The answer turned down, no value settled: the attribute is not answered again until it is reverted or settled.
    pub fn reject() -> Self {
        Self::none(SettlementKind::Reject)
    }

    /// The attribute's earlier settlement undone: it is open again.
    pub fn revert() -> Self {
        Self::none(SettlementKind::Revert)
    }

    /// A settlement of a kind known only at run time — a binding's, say — from the parts it needs: a value for the
    /// kinds that settle one, values for [`SettlementKind::Set`] (and for [`SettlementKind::Restore`] of an attribute
    /// that takes several, which is chosen by giving values), and for [`SettlementKind::Check`] the probability it was
    /// drawn with. Parts a kind does not need are ignored; a part it needs and lacks, or a probability out of (0, 1], is
    /// refused with a message.
    pub fn new(
        kind: SettlementKind,
        value: Option<&str>,
        values: Option<&[&str]>,
        probability: Option<f64>,
    ) -> Result<Self, String> {
        let one = || value.ok_or_else(|| format!("a {kind:?} settlement needs a value"));
        let many = || values.ok_or_else(|| format!("a {kind:?} settlement needs values"));
        Ok(match kind {
            SettlementKind::Accept => Self::accept(one()?),
            SettlementKind::Correct => Self::correct(one()?),
            SettlementKind::Enter => Self::enter(one()?),
            SettlementKind::Fill => Self::fill(one()?),
            SettlementKind::AcceptOther => Self::accept_other(one()?),
            SettlementKind::CorrectOther => Self::correct_other(one()?),
            SettlementKind::Restore if values.is_some() => {
                Self::restore_set(many()?.iter().copied())
            }
            SettlementKind::Restore => Self::restore(one()?),
            SettlementKind::Set => Self::set(many()?.iter().copied()),
            SettlementKind::Check => {
                let p = probability
                    .ok_or("a Check settlement needs the probability it was drawn with")?;
                if !(p > 0.0 && p <= 1.0) {
                    return Err(
                        "a check is drawn with a probability above 0 and at most 1".to_owned()
                    );
                }
                Self::check(one()?, p)
            }
            SettlementKind::Reject => Self::reject(),
            SettlementKind::Revert => Self::revert(),
        })
    }

    /// Whether this settles an attribute that takes several values.
    pub fn is_set(&self) -> bool {
        self.values.is_some()
    }
}

/// A record's values as last settled. `settled_at` is in 100-nanosecond ticks since the Unix epoch, UTC.
///
/// With the `serde` feature it is written with `settled_at` in ticks and reads `settled_at` as ticks or as text in the
/// form `yyyy-MM-ddTHH:mm:ss.fffffff±HH:MM` (.NET's round-trip format); an entry given as null reads as left out.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Record {
    pub id: String,
    #[cfg_attr(feature = "serde", serde(default, deserialize_with = "or_empty"))]
    pub values: BTreeMap<String, String>,
    #[cfg_attr(feature = "serde", serde(default, deserialize_with = "or_empty"))]
    pub sets: BTreeMap<String, Vec<String>>,
    #[cfg_attr(feature = "serde", serde(deserialize_with = "settled_at"))]
    pub settled_at: i64,
    /// How each decided attribute was settled, where known.
    #[cfg_attr(feature = "serde", serde(default, deserialize_with = "or_empty"))]
    pub settlements: BTreeMap<String, SettlementKind>,
    /// For an attribute settled as a check, the probability it was drawn with.
    #[cfg_attr(feature = "serde", serde(default, deserialize_with = "or_empty"))]
    pub probabilities: BTreeMap<String, f64>,
    /// The attributes with a value, in the order their current values arrived, oldest first; an attribute the list leaves out
    /// counts as there from the start. `None` when the order is unknown: the observed values are then taken as there from
    /// the start and the decided ones as settled in the schema's order.
    #[cfg_attr(feature = "serde", serde(default))]
    pub arrival: Option<Vec<String>>,
    /// The batch the record was settled in — records settled together, several at once from one source — when known.
    /// A layers replay counts what the batches do to its promise (`BatchCheck`); records of one batch name the same.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub batch: Option<String>,
    /// For each decided attribute settled with a suggestion in view, the value suggested — this decider's answer, or what
    /// the host showed in its place (the kind says which). For a coarse level, the class shown.
    #[cfg_attr(
        feature = "serde",
        serde(
            default,
            deserialize_with = "or_empty",
            skip_serializing_if = "BTreeMap::is_empty"
        )
    )]
    pub suggested: BTreeMap<String, String>,
    /// For each decided attribute settled with this decider's answer in view, the layer that gave it.
    #[cfg_attr(
        feature = "serde",
        serde(
            default,
            deserialize_with = "sources",
            skip_serializing_if = "BTreeMap::is_empty"
        )
    )]
    pub sources: BTreeMap<String, crate::decider::Source>,
    /// For each decided attribute settled with this decider's answer in view, the precision promised for it when it was
    /// shown — what [`crate::tuning::Tuning::promise_of`] gave for its layer. Each is in [0, 1].
    #[cfg_attr(
        feature = "serde",
        serde(
            default,
            deserialize_with = "promises",
            skip_serializing_if = "BTreeMap::is_empty"
        )
    )]
    pub promises: BTreeMap<String, f64>,
    /// For each decided attribute, the milliseconds from when the person reached it — its suggestion came into view, or
    /// it was reached with nothing in view — to when it was settled.
    #[cfg_attr(
        feature = "serde",
        serde(
            default,
            deserialize_with = "or_empty",
            skip_serializing_if = "BTreeMap::is_empty"
        )
    )]
    pub settle_ms: BTreeMap<String, u64>,
    /// The milliseconds the record was open, from opening it to this settlement.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub open_ms: Option<u64>,
    /// For each decided attribute, who settled it, as an opaque id the host gives — one person, one id; not a name. One
    /// record may be settled by several people, an attribute each.
    #[cfg_attr(
        feature = "serde",
        serde(
            default,
            deserialize_with = "or_empty",
            skip_serializing_if = "BTreeMap::is_empty"
        )
    )]
    pub settled_by: BTreeMap<String, String>,
    /// The version of the form the record was settled on, as the host identifies it — the edition of the code lists
    /// its decided attributes are settled from, which changes when a value comes to mean something else (a code
    /// renumbered, merged or split), not when only a label or the schema's layout changes.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub form_version: Option<String>,
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

/// An entry given as null reads as left out.
#[cfg(feature = "serde")]
fn or_empty<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    use serde::Deserialize;
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// A record's `sources`: each a layer that answers — `None` is no answer, so no source.
#[cfg(feature = "serde")]
fn sources<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, crate::decider::Source>, D::Error> {
    let sources: BTreeMap<String, crate::decider::Source> = or_empty(deserializer)?;
    match sources
        .iter()
        .find(|(_, s)| **s == crate::decider::Source::None)
    {
        Some((attribute, _)) => Err(serde::de::Error::custom(format!(
            "the source for '{attribute}' is None; a source is the layer that answered"
        ))),
        None => Ok(sources),
    }
}

/// A record's `promises`: each in [0, 1].
#[cfg(feature = "serde")]
fn promises<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, f64>, D::Error> {
    let promises: BTreeMap<String, f64> = or_empty(deserializer)?;
    match promises.iter().find(|(_, p)| !(0.0..=1.0).contains(*p)) {
        Some((attribute, p)) => Err(serde::de::Error::custom(format!(
            "the promise for '{attribute}' is {p}, not in [0, 1]"
        ))),
        None => Ok(promises),
    }
}

/// A record's `settled_at`: ticks, or a time as text.
#[cfg(feature = "serde")]
fn settled_at<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Time {
        Ticks(i64),
        Text(String),
    }

    use serde::Deserialize;
    match Time::deserialize(deserializer)? {
        Time::Ticks(ticks) => Ok(ticks),
        Time::Text(text) => ticks(&text).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "settled_at '{text}' is neither ticks nor a time yyyy-MM-ddTHH:mm:ss.fffffff±HH:MM"
            ))
        }),
    }
}

/// `yyyy-MM-ddTHH:mm:ss.fffffff±HH:MM` → 100-nanosecond ticks since the Unix epoch, UTC; `None` for any other text or a
/// date, time or offset out of range.
#[cfg(feature = "serde")]
fn ticks(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() != 33
        || [
            (4, b'-'),
            (7, b'-'),
            (10, b'T'),
            (13, b':'),
            (16, b':'),
            (19, b'.'),
            (30, b':'),
        ]
        .iter()
        .any(|&(at, separator)| bytes[at] != separator)
    {
        return None;
    }
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        let digits = &bytes[range];
        digits
            .iter()
            .all(u8::is_ascii_digit)
            .then(|| digits.iter().fold(0, |n, d| n * 10 + i64::from(d - b'0')))
    };
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second, fraction) = (
        number(11..13)?,
        number(14..16)?,
        number(17..19)?,
        number(20..27)?,
    );
    let sign = match bytes[27] {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let (offset_hours, offset_minutes) = (number(28..30)?, number(31..33)?);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&month)
        || !(1..=month_days[(month - 1) as usize]).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
        || offset_hours > 14
        || offset_minutes > 59
    {
        return None;
    }
    let offset = sign * (offset_hours * 60 + offset_minutes);
    // Days from civil (Howard Hinnant).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + hour * 3_600 + (minute - offset) * 60 + second;
    Some(seconds * 10_000_000 + fraction)
}

/// The key threshold, memory threshold and similarity floor a layers replay chose; `None` for a layer it chose none for.
fn chosen(layers: &LayerThresholds) -> (Option<f64>, Option<f64>, Option<f64>) {
    let memory = layers.memory.chosen.as_ref();
    (
        layers.key.chosen.as_ref().map(|c| c.threshold),
        memory.map(|c| c.threshold),
        memory.and_then(|c| c.similarity_floor),
    )
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

    #[test]
    fn a_typed_threshold_follows_the_key_threshold_by_characters_typed() {
        let mut attribute = Attribute::new("team", AttributeRole::Decided);
        attribute.key_threshold = Some(0.9);
        attribute.typed_key_thresholds = Some(vec![Some(0.3), None]);

        assert_eq!(attribute.key_threshold_for(0), Some(0.9));
        assert_eq!(attribute.key_threshold_for(1), Some(0.3));
        assert_eq!(attribute.key_threshold_for(2), None);
        assert_eq!(attribute.key_threshold_for(3), None);
        attribute.typed_key_thresholds = None;
        assert_eq!(attribute.key_threshold_for(1), None);
    }

    #[test]
    fn a_character_beyond_the_basic_plane_typed_is_one_character_typed() {
        let mut attribute = Attribute::new("f", AttributeRole::Decided);
        attribute.key_threshold = Some(9.0);
        attribute.typed_key_thresholds = Some(vec![Some(0.1), Some(0.2)]);

        assert_eq!(crate::text::scalars("😀"), 1);
        assert_eq!(
            attribute.key_threshold_for(crate::text::scalars("😀")),
            Some(0.1)
        );
    }

    #[test]
    fn an_attribute_is_supported_by_other_evidence_limited_to_its_declared_dependencies() {
        let mut reporter = Attribute::new("reporter", AttributeRole::Observed);
        reporter.use_as_evidence = false;
        let mut severity = Attribute::new("severity", AttributeRole::Decided);
        severity.depends_on = Some(vec!["component".to_owned()]);
        let schema = Schema::new(
            "ticket",
            vec![
                reporter,
                Attribute::new("component", AttributeRole::Observed),
                Attribute::new("summary", AttributeRole::Observed),
                Attribute::new("team", AttributeRole::Decided),
                severity,
            ],
        )
        .expect("a valid schema");

        assert!(schema.supports("component", "team"));
        assert!(schema.supports("severity", "team")); // a settled decided attribute is evidence too
        assert!(!schema.supports("reporter", "team"));
        assert!(!schema.supports("team", "team"));
        assert!(schema.supports("component", "severity"));
        assert!(!schema.supports("summary", "severity"));
        assert!(std::panic::catch_unwind(|| schema.supports("owner", "team")).is_err());
    }
}
