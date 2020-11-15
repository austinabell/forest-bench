mod logger;

use chain::ChainStore;
use fil_types::verifier::FullVerifier;
use genesis::{import_chain, initialize_genesis};
use state_manager::StateManager;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "forest_bench",
    version = "0.0.1",
    about = "Forest benchmarking utils",
    author = "ChainSafe Systems <info@chainsafe.io>",
    setting = structopt::clap::AppSettings::VersionlessSubcommands
)]
pub enum Cli {
    #[structopt(name = "import", about = "import chain from snapshot and validate.")]
    Import {
        #[structopt(help = "The genesis CAR file")]
        car: String,

        #[structopt(short, long, help = "The genesis CAR file")]
        data_dir: Option<String>,

        #[structopt(
            short,
            long,
            default_value = "0",
            help = "Height to validate the chain from"
        )]
        height: i64,
    },
}

#[async_std::main]
async fn main() {
    logger::setup_logger();

    match Cli::from_args() {
        #[allow(unused_variables)]
        Cli::Import {
            car,
            data_dir,
            height,
        } => {
            #[cfg(feature = "rocksdb")]
            let db = {
                let dir = data_dir
                    .as_ref()
                    .map(|s| s.as_str())
                    // TODO switch to home dir
                    .unwrap_or("data");

                let mut db = db::RocksDb::new(format!("{}{}", dir, "/db"));
                db.open().unwrap();
                Arc::new(db)
            };

            #[cfg(not(feature = "rocksdb"))]
            let db = Arc::new(db::MemoryDB::default());

            // Initialize StateManager
            let chain_store = Arc::new(ChainStore::new(Arc::clone(&db)));
            let state_manager = Arc::new(StateManager::new(Arc::clone(&chain_store)));

            // Read default Genesis into state (needed for validation)
            initialize_genesis(None, &state_manager).unwrap();

            // Sync from snapshot
            let file = File::open(car).expect("Snapshot file path not found!");
            let reader = BufReader::new(file);
            import_chain::<FullVerifier, _, _>(&state_manager, reader, Some(height))
                .await
                .unwrap();
        }
    }
}
