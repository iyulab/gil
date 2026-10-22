//! Times the promise-keeping loop on the first n of a file of settled-record lines: records arrive in steps, and each
//! time the choice is stale (a tenth more records) the thresholds are chosen again — once by replaying every record
//! from the start, as a host does by hand with `select_layers`, once with a `Keeper`, which carries its replay on.
//! Prints each rechoice's time for both and whether they chose the same.
//!
//! ```sh
//! cargo run --release -p iyulab-gil --features serde --example keeper_loop -- \
//!     <schema.json> <records.jsonl> <attribute> <n> <step>
//! ```

use std::time::Instant;

use gil::decider::Decider;
use gil::keeper::{Keep, Keeper};
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{Choice, LayerOptions, select_layers};
use gil::schema::{Record, Schema};
use gil::tuning::Tuning;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [schema, records, attribute, n, step] = args.as_slice() else {
        panic!("usage: keeper_loop <schema.json> <records.jsonl> <attribute> <n> <step>");
    };
    let schema: Schema =
        serde_json::from_str(&std::fs::read_to_string(schema).expect("the schema file"))
            .expect("a schema");
    let n: usize = n.parse().expect("a number of records");
    let step: usize = step.parse().expect("a step");
    let records: Vec<Record> = std::fs::read_to_string(records)
        .expect("the records file")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(n)
        .map(|line| serde_json::from_str(line).expect("a record line"))
        .collect();

    let keep = Keep::new(0.8);
    let mut keeper = Keeper::new(schema.clone(), keep.clone());
    let mut kept = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    let mut by_hand = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    let mut tuning = Tuning::capture(&schema, 0, keep.target_precision);
    let (mut start_total, mut keeper_total, mut settle_total) = (0.0, 0.0, 0.0);

    println!("records\tfrom_start_s\tkeeper_s\tsame");
    let mut done = 0;
    while done < records.len() {
        let next = (done + step).min(records.len());
        let part = &records[done..next];
        let t = Instant::now();
        by_hand.rebuild(&schema, part);
        settle_total += t.elapsed().as_secs_f64();
        keeper.settle(&mut kept, part);
        done = next;

        if tuning.stale(&schema, done).is_none() {
            continue;
        }
        let t = Instant::now();
        let alone = select_layers(
            &mut KeyMemory::default(),
            &mut LexicalMemory::default(),
            &schema,
            attribute,
            &records[..done],
            Choice {
                target_precision: keep.target_precision,
                minimum_answered: keep.minimum_answered,
                counting: None,
            },
            LayerOptions::default(),
        );
        let from_start = t.elapsed().as_secs_f64();
        tuning = Tuning::capture(&schema, done, keep.target_precision);

        let t = Instant::now();
        keeper.rechoose();
        let carried = t.elapsed().as_secs_f64();
        let same = format!("{alone:?}") == format!("{:?}", keeper.chosen(attribute).unwrap());
        start_total += from_start;
        keeper_total += carried;
        println!("{done}\t{from_start:.2}\t{carried:.2}\t{same}");
    }
    println!(
        "total\t{start_total:.2}\t{keeper_total:.2}\t(settling into the decider: {settle_total:.2} s)"
    );
}
