//! Which observed attribute to ask a person for first, so that a decided attribute gets answered. Experimental: its shape
//! may change in any release.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::decider::{Answer, Decider};
use crate::schema::{AttributePolicy, AttributeRole, Record, Schema};

/// The gain every class starts from before anything is learned under it.
pub const PRIOR_GAIN: f64 = 0.1;

/// The characters every class starts from before anything is learned under it.
pub const PRIOR_CHARACTERS: f64 = 10.0;

/// The coarse class a suggestion showed, and whether that class was trusted.
type Class = (Option<String>, bool);

/// Whether to ask a person for one attribute of an open record.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AskAdvice {
    /// The observed attribute.
    pub attribute: String,
    /// What asking it has gained per character typed, in records whose suggestion showed the same class.
    pub gain: f64,
    /// Whether it is worth asking within the budget.
    pub ask: bool,
    /// The gain per character asking must reach now; `None` before anything is learned.
    pub threshold: Option<f64>,
}

/// Which observed attribute to ask a person for first, so that a decided attribute gets answered: for each attribute
/// that could be asked, how much asking it has gained per character typed in records like this one, and whether it is
/// worth asking within a budget of the typing asking every time would take.
///
/// The planner learns from settled records, one at a time and in the order they were settled. For each attribute that
/// could be asked and that the record filled, it compares the decided attribute's answer without that attribute (the
/// moment before a person typed it) with the answer with it, and weighs each by what it would have given: 1 for a value
/// answered right, and for a trusted coarse class that holds the settled value, the share of the attribute's values it
/// rules out (`ln(V / V_c) / ln V`, V the attribute's values and V_c those in the class); nothing otherwise. The
/// difference, counted only where no layer answered the value without the attribute, is kept with the characters the
/// attribute took, under what the answer without it showed: its coarse class and whether that class was trusted.
///
/// Advice ranks the open attributes by gain per character — the gain kept under the class the record's answer shows,
/// with a prior of [`PRIOR_GAIN`] gained over [`PRIOR_CHARACTERS`] characters — and marks an attribute to ask where its
/// gain reaches the lowest gain among the records learned whose typing, counted from the highest gain down, stays within
/// the budget of all their typing, and asking so far has spent no more than that share. An attribute the answer already
/// answers is not worth asking. The budget is counted as though every piece of advice was followed.
///
/// Each attribute's gain is learned with only that attribute missing; advice for more than one open at once is an
/// approximation.
pub struct AskPlanner {
    schema: Schema,
    attribute: String,
    askable: Vec<String>,
    budget: f64,
    ledgers: HashMap<String, Ledger>,
    seen: HashSet<String>,
    class_sizes: Option<HashMap<String, usize>>,
}

impl AskPlanner {
    /// A planner for the decided single-valued attribute `attribute` of `schema` (not off), the observed attributes
    /// `askable` a person could be asked for (at least one, none twice), and the share `budget` (above 0, at most 1) of
    /// the typing asking every attribute every time would take that asking may spend. The error names the first rule
    /// broken.
    pub fn new<I, S>(
        schema: &Schema,
        attribute: &str,
        askable: I,
        budget: f64,
    ) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        if !(budget > 0.0 && budget <= 1.0) {
            return Err("the budget is a share above 0 and at most 1".into());
        }

        let definition = schema
            .attributes()
            .iter()
            .find(|a| a.name == attribute)
            .ok_or_else(|| format!("schema '{}' has no attribute '{attribute}'", schema.name()))?;
        if definition.role != AttributeRole::Decided
            || definition.multiple
            || definition.policy == AttributePolicy::Off
        {
            return Err(format!(
                "'{attribute}' is not a decided attribute that takes one value and is suggested"
            ));
        }

        let askable: Vec<String> = askable.into_iter().map(Into::into).collect();
        if askable.is_empty() {
            return Err("at least one attribute must be askable".into());
        }

        let mut ledgers = HashMap::new();
        for name in &askable {
            let observed = schema
                .attributes()
                .iter()
                .find(|a| &a.name == name)
                .is_some_and(|a| a.role == AttributeRole::Observed);
            if !observed {
                return Err(format!("'{name}' is not an observed attribute"));
            }

            if ledgers.insert(name.clone(), Ledger::default()).is_some() {
                return Err(format!("'{name}' is given twice"));
            }
        }

        Ok(Self {
            schema: schema.clone(),
            attribute: attribute.to_owned(),
            askable,
            budget,
            ledgers,
            seen: HashSet::new(),
            class_sizes: None,
        })
    }

    /// The schema it plans for, as it was given.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// The decided attribute it plans for.
    pub fn attribute(&self) -> &str {
        &self.attribute
    }

    /// The attributes a person could be asked for, as given.
    pub fn askable(&self) -> &[String] {
        &self.askable
    }

    /// The share of the typing asking every time would take that asking may spend.
    pub fn budget(&self) -> f64 {
        self.budget
    }

    /// Learns from a settled record of its schema, in settlement order, asking `decider` for the decided attribute's
    /// answer with and without each askable attribute the record filled. The answers never take the record as evidence
    /// for itself, so the decider's memory may already hold it — but no record settled after it. A record without a
    /// value for the decided attribute teaches nothing.
    pub fn learn(&mut self, decider: &Decider, record: &Record) {
        let Some(truth) = record
            .values
            .get(&self.attribute)
            .filter(|v| !v.trim().is_empty())
        else {
            return;
        };

        self.seen.insert(truth.clone());
        let known: BTreeMap<String, String> = record
            .values
            .iter()
            .filter(|(k, _)| **k != self.attribute)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut after: Option<Answer> = None;
        for name in self.askable.clone() {
            let Some(value) = known.get(&name).filter(|v| !v.trim().is_empty()) else {
                continue;
            };

            let mut without = known.clone();
            without.remove(&name);
            let before = self.answer(decider, record, &without);
            if before.answered() {
                continue;
            }

            let after = after.get_or_insert_with(|| self.answer(decider, record, &known));
            let c = self.weight(&before, truth);
            let a = self.weight(after, truth);
            let characters = value.chars().count();
            let budget = self.budget;
            self.ledgers
                .get_mut(&name)
                .expect("every askable attribute has a ledger")
                .learn(class_of(&before), a - c, characters, budget);
        }
    }

    /// Advice for an open record: every askable attribute it has no value for, by gain per character, highest first (in
    /// the order given where gains tie), each marked whether to ask. `answer` is the decided attribute's answer for the
    /// record as it stands, `values` its values as they stand.
    ///
    /// # Panics
    /// When `answer` is for another attribute.
    pub fn advise(&self, answer: &Answer, values: &BTreeMap<String, String>) -> Vec<AskAdvice> {
        assert!(
            answer.attribute == self.attribute,
            "the answer is for '{}', not '{}'",
            answer.attribute,
            self.attribute
        );
        let key = class_of(answer);
        let mut advice: Vec<AskAdvice> = self
            .askable
            .iter()
            .filter(|name| values.get(*name).is_none_or(|v| v.trim().is_empty()))
            .map(|name| {
                let ledger = &self.ledgers[name];
                let gain = ledger.gain(&key);
                AskAdvice {
                    attribute: name.clone(),
                    gain,
                    ask: !answer.answered() && ledger.asks(gain, self.budget),
                    threshold: ledger.threshold(self.budget),
                }
            })
            .collect();
        advice.sort_by(|x, y| y.gain.total_cmp(&x.gain));
        advice
    }

    fn answer(
        &self,
        decider: &Decider,
        record: &Record,
        values: &BTreeMap<String, String>,
    ) -> Answer {
        decider.answer_one(
            &self.schema,
            &record.id,
            &self.attribute,
            values,
            &record.sets,
            None,
        )
    }

    /// What an answer gives: 1 for a value answered right, 0 for one answered wrong, else the share a trusted class that
    /// holds the settled value rules out.
    fn weight(&mut self, answer: &Answer, truth: &str) -> f64 {
        if answer.answered() {
            return if answer.candidates[0].value == truth {
                1.0
            } else {
                0.0
            };
        }

        let Some(coarse) = answer.coarse.as_ref().filter(|c| c.trusted) else {
            return 0.0;
        };
        let Some(level) = self.schema.attribute(&self.attribute).coarse.as_ref() else {
            return 0.0;
        };

        let (values, in_class) = if let Some(table) = &level.classes {
            match table.get(truth) {
                Some(named) if *named == coarse.value => {}
                _ => return 0.0,
            }

            let sizes = self.class_sizes.get_or_insert_with(|| {
                let mut sizes = HashMap::new();
                for class in table.values() {
                    *sizes.entry(class.clone()).or_insert(0) += 1;
                }
                sizes
            });
            (table.len(), sizes[&coarse.value])
        } else {
            if level.of(truth) != coarse.value {
                return 0.0;
            }

            let in_class = self
                .seen
                .iter()
                .filter(|v| level.of(v) == coarse.value)
                .count();
            (self.seen.len(), in_class)
        };

        if values > 1 {
            (values as f64 / in_class as f64).ln() / (values as f64).ln()
        } else {
            0.0
        }
    }
}

fn class_of(answer: &Answer) -> Class {
    match &answer.coarse {
        Some(c) => (Some(c.value.clone()), c.trusted),
        None => (None, false),
    }
}

/// One askable attribute's gains by class and the budget it has spent.
#[derive(Default)]
struct Ledger {
    by_class: HashMap<Class, (f64, f64)>,
    /// Every learned decision's gain per character at the time, with its characters, ascending.
    history: Vec<(f64, usize)>,
    seen_characters: u64,
    spent_characters: u64,
}

impl Ledger {
    fn gain(&self, key: &Class) -> f64 {
        let (gain, characters) = self.by_class.get(key).copied().unwrap_or((0.0, 0.0));
        (gain + PRIOR_GAIN) / (characters + PRIOR_CHARACTERS)
    }

    fn threshold(&self, budget: f64) -> Option<f64> {
        if self.history.is_empty() {
            return None;
        }

        let need = budget * self.seen_characters as f64;
        let mut counted = 0u64;
        let mut threshold = f64::INFINITY;
        for &(gain, characters) in self.history.iter().rev() {
            counted += characters as u64;
            threshold = gain;
            if counted as f64 >= need {
                break;
            }
        }

        Some(threshold)
    }

    fn asks(&self, gain: f64, budget: f64) -> bool {
        self.threshold(budget).is_some_and(|t| gain >= t)
            && self.spent_characters as f64 <= budget * self.seen_characters as f64
    }

    /// Learns one decision and says whether the advice before it was to ask.
    fn learn(&mut self, key: Class, gained: f64, characters: usize, budget: f64) -> bool {
        let gain = self.gain(&key);
        let asked = self.asks(gain, budget);
        let entry = (gain, characters);
        let at = self.history.partition_point(|e| {
            e.0.total_cmp(&entry.0).then_with(|| e.1.cmp(&entry.1)) != std::cmp::Ordering::Greater
        });
        self.history.insert(at, entry);
        self.seen_characters += characters as u64;
        if asked {
            self.spent_characters += characters as u64;
        }
        let slot = self.by_class.entry(key).or_insert((0.0, 0.0));
        slot.0 += gained;
        slot.1 += characters as f64;
        asked
    }
}
