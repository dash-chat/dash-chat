use clap::Parser;
use mailbox_server::{dump::dump_db, init_db};

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
    // let watermarks_table = read_txn.open_table(WATERMARKS_TABLE).unwrap();
    // let blobs_table = read_txn.open_table(BLOBS_TABLE).unwrap();

    let keys = dump_db(&db).unwrap();

    println!("{:#?}", keys);

    // for entry in dollops_table.iter().unwrap() {
    //     let (key, _) = entry.unwrap();
    //     let key: DollopsKey = key.value();
    //     println!(
    //         "* {}\t{}\t{}",
    //         key.topic_id, key.author, key.sequence_number
    //     );
    // }
}
