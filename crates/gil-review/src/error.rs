use std::fmt;

/// Why a review computation refused its input. Each names what was wrong and, where there is one, the stratum or item.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReviewError {
    /// No strata were given.
    NoStrata,
    /// A stratum has no name, or shares its name with another.
    StratumName { name: String },
    /// A record is listed in two strata; strata partition the population.
    RecordInTwoStrata { record: String },
    /// Fewer than one record per stratum was asked for.
    PerStratumTooSmall,
    /// A stratum claims a population of 0.
    EmptyPopulation { stratum: String },
    /// The budget cannot give every stratum one record.
    BudgetTooSmall { budget: u64, strata: usize },
    /// A stratum's deviation is negative or not finite.
    Deviation { stratum: usize },
    /// An item names a stratum that is not there.
    UnknownStratum { stratum: u32 },
    /// An item's score is outside [0, 1], or its weight is not above 0.
    Item { index: usize },
    /// A stratum has no scored item; filling it from the others would estimate another population.
    UnsampledStratum { stratum: String },
    /// A stratum has more scored items than records.
    MoreItemsThanRecords { stratum: String },
    /// The two item sets of a band disagree in which strata they sample.
    BoundsDiffer,
    /// Fewer than two outcome categories.
    Categories,
    /// A verdict's outcome is not below the number of categories.
    Outcome { item: String, candidate: String },
    /// One reviewer judged the same (item, candidate) twice.
    SameReviewerTwice { item: String, candidate: String },
    /// More than two verdicts on one (item, candidate).
    MoreThanTwoVerdicts { item: String, candidate: String },
    /// No (item, candidate) was judged by two reviewers.
    NothingCompared,
}

impl fmt::Display for ReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoStrata => write!(f, "at least one stratum is needed"),
            Self::StratumName { name } => write!(
                f,
                "every stratum needs a name of its own; '{name}' is empty or given twice"
            ),
            Self::RecordInTwoStrata { record } => write!(
                f,
                "record '{record}' is in more than one stratum; strata must partition the population"
            ),
            Self::PerStratumTooSmall => {
                write!(f, "at least one record per stratum must be asked for")
            }
            Self::EmptyPopulation { stratum } => write!(
                f,
                "stratum '{stratum}' claims a population of 0; a stratum holding no records has no weight"
            ),
            Self::BudgetTooSmall { budget, strata } => write!(
                f,
                "a budget of {budget} cannot give each of {strata} strata one record"
            ),
            Self::Deviation { stratum } => write!(
                f,
                "the deviation of stratum {stratum} is negative or not finite"
            ),
            Self::UnknownStratum { stratum } => {
                write!(f, "an item names stratum {stratum}, which is not given")
            }
            Self::Item { index } => write!(
                f,
                "item {index} has a score outside [0, 1] or a weight that is not above 0"
            ),
            Self::UnsampledStratum { stratum } => write!(
                f,
                "stratum '{stratum}' has no scored item; an estimate without it is of another population"
            ),
            Self::MoreItemsThanRecords { stratum } => {
                write!(f, "stratum '{stratum}' has more scored items than records")
            }
            Self::BoundsDiffer => write!(
                f,
                "the lower and upper item sets must sample the same strata with the same counts"
            ),
            Self::Categories => write!(f, "agreement needs at least two outcome categories"),
            Self::Outcome { item, candidate } => write!(
                f,
                "a verdict on ('{item}', '{candidate}') has an outcome outside the categories"
            ),
            Self::SameReviewerTwice { item, candidate } => {
                write!(f, "one reviewer judged ('{item}', '{candidate}') twice")
            }
            Self::MoreThanTwoVerdicts { item, candidate } => {
                write!(f, "('{item}', '{candidate}') has more than two verdicts")
            }
            Self::NothingCompared => write!(f, "no item was judged by two reviewers"),
        }
    }
}

impl std::error::Error for ReviewError {}
