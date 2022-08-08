use std::error::Error;
use std::path::PathBuf;

use bitcoin_statistics::{fetch_client, fetch_settings, BlockSample, BlockStatistics};
use clap::Parser;

mod cli;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = cli::Args::parse();
    // Pull in settings for connecting a bitcoind
    let settings = fetch_settings(PathBuf::from(cli.config))?;
    // Fetch the RPC client
    let rpc = fetch_client(settings)?;
    // Create a new sample based on inputs
    let sample = BlockSample::new(
        cli.z_score,
        cli.std_deviation,
        cli.margin_error,
        cli.full_population,
    );

    // Run the selected analysis on the data
    match &cli.command {
        cli::Analysis::BlockTimeDrift { drift_time, window } => {
            let data = sample.collect(rpc, Some(*window)).await?;
            data.fetch_block_time_drift(*drift_time, *window, cli.full_population);
        }
    }

    Ok(())
}
