use clap::{Parser, Subcommand};

mod db_insert;
mod db_query;
mod init;
mod run;
mod trace_range;
mod utils;

pub use utils::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(name = "brontes", author = "Sorella Labs", version = "0.1.0")]
#[command(propagate_version = true)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Runs brontes
    #[command(name = "run")]
    Run(run::RunArgs),
    #[command(name = "init")]
    Init(init::Init),
    #[command(name = "db")]
    QueryDb(db_query::DatabaseQuery),
    #[command(name = "db_add")]
    AddToDb(db_insert::AddToDb),
    #[command(name = "trace-range")]
    TraceRange(trace_range::TraceArgs),
}
