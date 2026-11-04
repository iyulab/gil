//! History loaded in bulk from another system: its records carry no settlement order worth asking about, so a replay can
//! take them into memory without counting them. Naming their batch in `uncounted_batches` leaves them out of exactly what a
//! kind of settlement left out of `counting` is left out of — and, unlike a kind, leaves out nothing else.

use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{Choice, LayerOptions, LayerThresholds, select_layers};
use gil::schema::{Attribute, AttributeRole, Record, Schema, SettlementKind};

const MINUTE: i64 = 60 * 10_000_000;
const TEAMS: [&str; 4] = ["network", "desk", "facilities", "billing"];
const IMPORT: &str = "import";

fn schema() -> Schema {
    Schema::new(
        "ticket",
        vec![
            Attribute::new("source", AttributeRole::Observed),
            Attribute::new("summary", AttributeRole::Observed),
            Attribute::new("team", AttributeRole::Decided),
        ],
    )
    .expect("a valid schema")
}

/// A record whose source decides its team, so a memory that has seen the source can answer.
fn record(n: usize, source: usize, kind: SettlementKind, batch: Option<&str>) -> Record {
    Record {
        id: format!("t{n}"),
        values: [
            ("source".to_owned(), format!("s{source}")),
            ("summary".to_owned(), format!("s{source} w{}", n % 7)),
            ("team".to_owned(), TEAMS[source % TEAMS.len()].to_owned()),
        ]
        .into_iter()
        .collect(),
        settled_at: n as i64 * MINUTE,
        settlements: [("team".to_owned(), kind)].into_iter().collect(),
        batch: batch.map(str::to_owned),
        ..Record::default()
    }
}

/// An import of `imported` records from sources 0..12, then `live` records from the same sources, each live record its own
/// batch. The import is settled as `import_kind`; live records as `Enter`.
fn history(imported: usize, live: usize, import_kind: SettlementKind) -> Vec<Record> {
    let mut records: Vec<Record> = (0..imported)
        .map(|n| record(n, n % 12, import_kind, Some(IMPORT)))
        .collect();
    records
        .extend((0..live).map(|n| record(imported + n, (n * 5) % 12, SettlementKind::Enter, None)));
    records
}

fn chosen(
    records: &[Record],
    counting: Option<&[SettlementKind]>,
    uncounted: &[String],
    value_tails: bool,
) -> LayerThresholds {
    select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema(),
        "team",
        records,
        Choice {
            target_precision: 0.7,
            minimum_answered: 10,
            counting,
            uncounted_batches: uncounted,
        },
        LayerOptions {
            value_tails,
            ..LayerOptions::default()
        },
    )
}

const ENTER: [SettlementKind; 1] = [SettlementKind::Enter];

/// The workaround the option replaces: the import settled as a kind `counting` leaves out. Everything must come out the
/// same — thresholds, promise, value tails, the batch check.
#[test]
fn an_uncounted_batch_is_left_out_as_a_kind_outside_counting_is() {
    let by_batch = chosen(
        &history(400, 300, SettlementKind::Enter),
        Some(&ENTER),
        &[IMPORT.to_owned()],
        true,
    );
    let by_kind = chosen(
        &history(400, 300, SettlementKind::AcceptOther),
        Some(&ENTER),
        &[],
        true,
    );
    assert_eq!(by_batch, by_kind);
    assert!(by_batch.key.chosen.is_some() || by_batch.memory.chosen.is_some());
    assert!(by_batch.key.lookups + by_batch.memory.lookups > 0);
}

/// What a borrowed kind cannot do: the live records settled as `Enter`, like the import, still count.
#[test]
fn live_records_of_the_imports_kind_still_count() {
    let records = history(400, 300, SettlementKind::Enter);
    let all = chosen(&records, Some(&ENTER), &[], false);
    let live_only = chosen(&records, Some(&ENTER), &[IMPORT.to_owned()], false);
    assert!(
        live_only.key.lookups < all.key.lookups,
        "the import's lookups are not counted: {} against {}",
        live_only.key.lookups,
        all.key.lookups
    );
    assert!(live_only.key.lookups > 0, "the live records are counted");
    // Borrowing the kind instead drops the live records too.
    let borrowed = chosen(
        &history(400, 300, SettlementKind::Enter),
        Some(&[SettlementKind::Check]),
        &[],
        false,
    );
    assert_eq!(borrowed.key.lookups, 0);
}

/// The import is remembered all the same: a live record is answered from a source only the import taught.
#[test]
fn the_memory_still_learns_from_an_uncounted_batch() {
    // The live records come from sources the import shows; with the import left out of memory they would find nothing.
    let records = history(400, 120, SettlementKind::Enter);
    let mut answered = 0;
    let mut sink = |d: &gil::decision_record::DecisionRecord| {
        if d.weight > 0.0 && d.key.is_some() {
            answered += 1;
        }
    };
    let result = select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema(),
        "team",
        &records,
        Choice {
            target_precision: 0.7,
            minimum_answered: 10,
            counting: Some(&ENTER),
            uncounted_batches: &[IMPORT.to_owned()],
        },
        LayerOptions {
            sink: Some(&mut sink),
            ..LayerOptions::default()
        },
    );
    assert_eq!(result.key.lookups, 120);
    assert!(answered > 0, "the key memory answers from the import");
}

/// Records that name no batch, or other batches, are never left out by the list.
#[test]
fn a_record_naming_no_other_batch_is_not_left_out() {
    let records = history(300, 200, SettlementKind::Enter);
    let other = chosen(&records, Some(&ENTER), &["elsewhere".to_owned()], false);
    let none = chosen(&records, Some(&ENTER), &[], false);
    assert_eq!(other, none);
}
