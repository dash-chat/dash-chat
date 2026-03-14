use std::collections::{BTreeMap, BTreeSet};

use clap::Parser;
use mailbox_server::{init_db, DollopsKey, BLOBS_TABLE, DOLLOPS_TABLE, WATERMARKS_TABLE};
use redb::{ReadableDatabase, ReadableTable};

#[derive(Parser, Debug)]
#[command(name = "dump-db")]
#[command(about = "Dumps the contents of a redb database", long_about = None)]
struct Args {
    /// Path to the redb database file
    #[arg(short = 'p', long)]
    db_path: String,
}

fn main() {
    let args = Args::parse();
    let db = init_db(args.db_path.into()).unwrap();
    let read_txn = db.begin_read().unwrap();
    let dollops_table = read_txn.open_table(DOLLOPS_TABLE).unwrap();
    // let watermarks_table = read_txn.open_table(WATERMARKS_TABLE).unwrap();
    // let blobs_table = read_txn.open_table(BLOBS_TABLE).unwrap();

    let mut keys = BTreeMap::new();

    for entry in dollops_table.iter().unwrap() {
        let (key, _) = entry.unwrap();
        let key: DollopsKey = key.value();
        keys.entry(key.topic_id)
            .and_modify(|v: &mut BTreeMap<String, BTreeSet<u64>>| {
                v.entry(key.author)
                    .and_modify(|v: &mut BTreeSet<u64>| {
                        v.insert(key.sequence_number);
                    })
                    .or_insert(BTreeSet::new());
            })
            .or_insert(BTreeMap::new());
    }

    println!("{:#?}", keys);

    // for entry in dollops_table.iter().unwrap() {
    //     let (key, _) = entry.unwrap();
    //     let key: DollopsKey = key.value();
    //     println!(
    //         "* {}\t{}\t{}",
    //         key.topic_id, key.author, key.sequence_number
    //     );
    // }
    // println!("Let's buuild a toy");
}
