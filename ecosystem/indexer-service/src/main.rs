// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

//! The Indexer Service offers a rest API for the default Indexer
//!
#![forbid(unsafe_code)]

use aptos_logger::info;
use clap::Parser;
use std::sync::Arc;

use aptos_indexer::database::new_db_pool;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct IndexerServiceArgs {
    /// Postgres database uri, ex: "postgresql://user:pass@localhost/postgres"
    #[clap(long)]
    pg_uri: String,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    aptos_logger::Logger::new().init();

    let args: IndexerServiceArgs = IndexerServiceArgs::parse();

    info!("Starting indexer Service...");

    let conn_pool = new_db_pool(&args.pg_uri).unwrap();
    info!("Created the connection pool... ");

    warp::serve(aptos_indexer_service).bind("0.0.0.0").await;

    Ok(())
}
