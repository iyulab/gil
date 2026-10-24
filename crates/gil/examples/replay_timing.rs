//! Times a layers replay on the first n of a file of settled-record lines, for each n given — how the cost of choosing
//! thresholds grows with the records settled.
//!
//! ```sh
//! cargo run --release -p iyulab-gil --features serde --example replay_timing -- \
//!     <schema.json> <records.jsonl> <attribute> 5000 10000 20000
//! ```

use std::time::Instant;

use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{Choice, LayerOptions, select_layers};
use gil::schema::{Record, Schema};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [schema, records, attribute, sizes @ ..] = args.as_slice() else {
        panic!("usage: replay_timing <schema.json> <records.jsonl> <attribute> <n>...");
    };
    let schema: Schema =
        serde_json::from_str(&std::fs::read_to_string(schema).expect("the schema file"))
            .expect("a schema");
    let records: Vec<Record> = std::fs::read_to_string(records)
        .expect("the records file")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("a record line"))
        .collect();

    println!("records\tseconds\tkey_lookups\tmemory_chosen");
    for size in sizes {
        let n: usize = size.parse().expect("a number of records");
        let start = Instant::now();
        let layers = select_layers(
            &mut KeyMemory::default(),
            &mut LexicalMemory::default(),
            &schema,
            attribute,
            &records[..n.min(records.len())],
            Choice {
                target_precision: 0.8,
                minimum_answered: 10,
                counting: None,
            },
            LayerOptions::default(),
        );
        println!(
            "{}\t{:.2}\t{}\t{:?}",
            n.min(records.len()),
            start.elapsed().as_secs_f64(),
            layers.key.lookups,
            layers.memory.chosen.as_ref().map(|c| c.threshold),
        );
    }
}
