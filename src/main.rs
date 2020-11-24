mod logger;

use async_std::fs::File as AsyncFile;
use async_std::fs::File;
use async_std::io::BufReader;
use async_std::io::BufWriter as AsyncBufWriter;
use chain::ChainStore;
use fil_types::verifier::FullVerifier;
use forest_blocks::TipsetKeys;
use forest_car::CarReader;
use genesis::{import_chain, initialize_genesis};
use state_manager::StateManager;
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
        #[structopt(help = "Import path or url for car file")]
        car: String,

        #[structopt(short, long, help = "Data directory for chain data")]
        data_dir: Option<String>,

        #[structopt(
            short,
            long,
            default_value = "0",
            help = "Height to validate the chain from"
        )]
        height: i64,

        #[structopt(short, long, help = "Skip loading full car file")]
        skip_load: bool,
    },
    #[structopt(name = "export", about = "export chain data to car file")]
    Export {
        #[structopt(help = "Import car file to use for exporting at height")]
        car: String,

        #[structopt(help = "Height to export chain from")]
        height: i64,

        #[structopt(short, long, help = "File to export to")]
        out: String,

        #[structopt(short, long, help = "Data directory for chain data")]
        data_dir: Option<String>,

        #[structopt(
            short,
            long,
            default_value = "900",
            help = "Number of state roots to include"
        )]
        recent_roots: i64,
    },
}

#[async_std::main]
async fn main() {
    logger::setup_logger();

    match Cli::from_args() {
        Cli::Import {
            car,
            data_dir,
            height,
            skip_load,
        } => {
            let db = {
                let dir = data_dir
                    .as_ref()
                    .map(|s| s.as_str())
                    // TODO switch to home dir
                    .unwrap_or("data");

                let db = db::rocks::RocksDb::open(format!("{}{}", dir, "/db")).unwrap();
                Arc::new(db)
            };

            // Initialize StateManager
            let chain_store = Arc::new(ChainStore::new(Arc::clone(&db)));
            let state_manager = Arc::new(StateManager::new(Arc::clone(&chain_store)));

            // Read default Genesis into state (needed for validation)
            initialize_genesis(None, &state_manager).await.unwrap();

            // Sync from snapshot
            if skip_load {
                let file = File::open(car)
                    .await
                    .expect("Snapshot file path not found!");
                let file_reader = BufReader::new(file);
                let cr = CarReader::new(file_reader).await.unwrap();
                let ts = chain_store
                    .tipset_from_keys(&TipsetKeys::new(cr.header.roots))
                    .await
                    .unwrap();
                state_manager
                    .validate_chain::<FullVerifier>(ts, height)
                    .await
                    .unwrap();
            } else {
                import_chain::<FullVerifier, _>(&state_manager, &car, Some(height))
                    .await
                    .unwrap();
            }
        }
        Cli::Export {
            car,
            out,
            data_dir,
            height,
            recent_roots,
        } => {
            let db = {
                let dir = data_dir
                    .as_ref()
                    .map(|s| s.as_str())
                    // TODO switch to home dir
                    .unwrap_or("data");

                let db = db::rocks::RocksDb::open(format!("{}{}", dir, "/db")).unwrap();
                Arc::new(db)
            };

            let chain_store = Arc::new(ChainStore::new(Arc::clone(&db)));

            let file = File::open(car)
                .await
                .expect("Snapshot file path not found!");
            let file_reader = BufReader::new(file);
            let cr = CarReader::new(file_reader).await.unwrap();
            let mut ts = chain_store
                .tipset_from_keys(&TipsetKeys::new(cr.header.roots))
                .await
                .unwrap();

            while ts.epoch() > height {
                ts = chain_store.tipset_from_keys(ts.parents()).await.unwrap();
            }

            let file = AsyncFile::create(out)
                .await
                .expect("Snapshot file path not found!");
            let writer = AsyncBufWriter::new(file);

            chain_store
                .export(&ts, recent_roots, false, writer)
                .await
                .unwrap();
        }
    }
}
