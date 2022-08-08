use bitcoincore_rpc::{bitcoin::BlockHeader, Auth, Client, RpcApi};
use config::Config;
use indicatif::ProgressBar;
use rand::{distributions::Uniform, Rng};
use serde::Deserialize;
use std::{collections::BinaryHeap, path::PathBuf, sync::Arc};
use tabled::{Footer, Header, Table, Tabled};
use thiserror::Error;
use tokio::task::JoinError;

use crate::utils::*;

pub mod utils;

pub type Result<T> = std::result::Result<T, StatisticsError>;
pub type BlockHeap = BinaryHeap<BlockTimeDriftTable>;

/// Application errors.
#[derive(Error, Debug)]
pub enum StatisticsError {
    #[error("Missing configuration for accessing bitcoind")]
    ConfigError(#[from] config::ConfigError),
    #[error("Bitcoin client connectivity error")]
    ClientError(#[from] bitcoincore_rpc::Error),
    #[error("An error occurred fetching block data")]
    RPCError(#[from] JoinError),
}

/// Configurations required for connecting to bitcoind via RPC.
#[derive(Deserialize)]
pub struct ClientConfig {
    host: String,
    username: String,
    password: String,
}

/// Configuration for sampling data from the network.
#[derive(Debug)]
pub struct BlockSample {
    z_score: f64,
    margin_error: f64,
    std_deviation: f64,
    full_population: bool,
}

/// Collected sample data ready for analysis.
#[derive(Debug)]
pub struct BlockSampleData(Vec<BlockHeader>);

/// Use a struct to store the drift and blocks for a binary heap.
/// Doubles as the sample table.
#[derive(Tabled, Eq, PartialEq, Debug)]
pub struct BlockTimeDriftTable {
    #[tabled(rename = "Mining Time", order = 2, display_with = "display_mins")]
    drift: i64,
    #[tabled(rename = "Parent Block Hash", order = 0)]
    parent_hash: String,
    #[tabled(rename = "Child Block Hash", order = 1)]
    child_hash: String,
}

/// Table for showing the Poission distribution of the sampled data.
/// In theory, due to Bitoin's target difficulty, this distribution should
/// be as such that the 95% percentile of block times should fall within
/// the 10-minute range.
#[derive(Tabled, Eq, PartialEq, Debug)]
pub struct BlockTimePoissonTable {
    #[tabled(rename = "Mining Time", order = 2)]
    drift: i64,
    #[tabled(rename = "Parent Block Hash", order = 0)]
    parent_hash: String,
    #[tabled(rename = "Child Block Hash", order = 1)]
    child_hash: String,
}

/// Possible statistical analysis that can be run on sampled data.
pub trait BlockStatistics {
    /// Run a statistical analysis of two contiguous blocks having a specified
    /// drift time between mining. The window specifies the number of
    /// contiguous blocks occur in the sample data.
    ///
    /// Note: This will not be totally accurate as blocks depends on miner
    /// provided timestamps which must be within a 2-hour window of network
    /// adjusted time and be greater than the median of the past 11 blocks.
    /// See https://github.com/bitcoin/bips/blob/master/bip-0113.mediawiki
    /// See https://arxiv.org//pdf/1803.09028.pdf
    fn fetch_block_time_drift(self, drift_time: i64, window: u64, sample: bool);
}

impl PartialOrd for BlockTimeDriftTable {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.drift.partial_cmp(&other.drift)
    }
}

impl Ord for BlockTimeDriftTable {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.drift.cmp(&other.drift)
    }
}

impl BlockTimeDriftTable {
    /// Create a new instance of a BlockSample which will specify the
    /// parameters for fetching the sample data from bitcoind.
    pub fn new(drift: i64, parent_hash: String, child_hash: String) -> Self {
        BlockTimeDriftTable {
            drift,
            parent_hash,
            child_hash,
        }
    }
}

impl BlockSample {
    /// Create a new instance of a BlockSample which will specify the
    /// parameters for fetching the sample data from bitcoind.
    pub fn new(z_score: f64, std_deviation: f64, margin_error: f64, full_population: bool) -> Self {
        BlockSample {
            z_score,
            std_deviation,
            margin_error,
            full_population,
        }
    }

    /// Collect the sample data from the blockchain. You can provide an
    /// optional window if you want to handle n contiguous blocks. This
    /// will return the random sampling / window. This is important when
    /// comparing contiguous blocks and defaults to 2 (min required).
    pub async fn collect(&self, client: Client, window: Option<u64>) -> Result<BlockSampleData> {
        let block_heights;
        let progress_bar = ProgressBar::new_spinner();
        progress_bar.println("Fetching current block height...");
        // Get the current block height
        let block_max = client.get_block_count()?;

        progress_bar.finish_with_message(format!("Success! Block height: {}", block_max));

        if self.full_population {
            // Get all the blocks for full population analysis
            block_heights = Vec::from_iter(0..block_max);

            println!("Using total population of {}", block_max);
        } else {
            // Get a sample of randomized block heights
            block_heights = self.get_random_heights(block_max, window.unwrap_or(2));

            println!("Utilizing a z-score of {}", self.z_score);
            println!("With a standard deviation of {}", self.std_deviation);
            println!("Within a {:.2}% error margin", (self.margin_error * 100.0));
            println!(
                "Sampling {} blocks from a population of {}",
                block_heights.len(),
                block_max
            );
        }

        // Get the block data from the sample indexes
        let blocks = get_blocks(block_heights, Arc::new(client));
        Ok(BlockSampleData(blocks.await?))
    }

    /// Calculate the sample size based on the known highest block height.
    /// We'll use the Cochran Formula for this as there are a lot of blocks
    /// (large population) at this point on the Bitcoin network.
    fn get_sample_size(&self, n: u64) -> u64 {
        let zpq = self.z_score.powf(2.0) * (self.std_deviation * (1.0 - self.std_deviation));
        let n0 = (zpq / self.margin_error.powf(2.0)).ceil();
        let sample = n0 / (1.0 + ((n0 - 1.0) / n as f64));

        sample.ceil() as u64
    }

    /// Get the randomized sample of block heights.
    fn get_random_heights(&self, block_max: u64, window: u64) -> Vec<u64> {
        let range = Uniform::new(0, block_max);
        let mut rng = rand::thread_rng();
        let mut result: Vec<u64> = Vec::new();

        // O(n) time windowing for allowing contigous sample blocks
        for _ in 0..(self.get_sample_size(block_max) / window) {
            let mut sample = rng.sample(&range);
            result.push(sample);

            for _ in 0..window - 1 {
                sample += 1;
                result.push(sample);
            }
        }

        result
    }
}

impl BlockStatistics for BlockSampleData {
    fn fetch_block_time_drift(self, drift_time: i64, window: u64, sample: bool) {
        let window = window as usize;
        // Store the data in a binary heap to bubble up the longest drifts
        let mut heap: BlockHeap = BinaryHeap::new();
        // Result formatting for sample data
        let mut sample_table = vec![];
        // Result formatting for probability data
        let mut block_deltas = vec![];

        // Add the blocks to the heap by their timestamp difference
        self.0.windows(window).step_by(window).for_each(|blocks| {
            let mut prev = &blocks[0];

            blocks.iter().skip(1).for_each(|block| {
                let drift = (block.time as i64).checked_sub(prev.time as i64);
                if let Some(time) = drift {
                    block_deltas.push(time as f64 / 60.0);
                    // Pushing to the heap after iterating blocks gives us
                    // O(n log n) time
                    heap.push(BlockTimeDriftTable::new(
                        time as i64 / 60,
                        prev.block_hash().to_string(),
                        block.block_hash().to_string(),
                    ));
                    prev = block;
                }
            })
        });

        // Popping from the heap will give us the highest drifts descending
        while let Some(leaf) = heap.pop() {
            if leaf.drift >= drift_time as i64 / 60 {
                sample_table.push(leaf);
            } else {
                break;
            }
        }

        let occurences = sample_table.len();
        let hours = -(drift_time as f64 / 60.0_f64.powf(2.0));
        // Get the mean block minting time
        let mean_time = get_mean(&block_deltas);
        // Get the standard deviation
        let std_deviation = get_standard_deviation(&block_deltas, sample);
        // Get the poisson probability using the sample data
        let poisson_prob = get_poisson_probability(60.0 / mean_time, hours);

        let table = Table::new(sample_table)
            .with(Header("Block Times"))
            .with(Footer(format!(
                "Occurrences: {}, Mean: {} minutes, Standard Deviation: {}, Poisson Probability: 1 / {} hours",
                occurences,
                get_rounded_by(mean_time, 2),
                std_deviation,
                get_rounded_by(poisson_prob, 2)
            )));

        // Output the table
        println!("{}", table);
    }
}

/// Fetch settings for connecting to bitcoind.
pub fn fetch_settings(config_path: PathBuf) -> Result<ClientConfig> {
    let path = config_path.to_str().expect("Cannot parse path");
    let settings = Config::builder()
        .add_source(config::File::with_name(path))
        .add_source(config::Environment::with_prefix("APP"))
        .build()?
        .try_deserialize::<ClientConfig>()?;

    Ok(settings)
}

/// Return a new bitcoin RPC client using the specified configuration.
pub fn fetch_client(config: ClientConfig) -> Result<Client> {
    println!("Connecting to: {}...", config.host);
    let client = Client::new(
        &config.host,
        Auth::UserPass(config.username, config.password),
    )?;

    println!("Connectied to: {}!", config.host);
    Ok(client)
}

/// Get the blocks using the list of block heights and the specified
/// RPC client.
///
/// TODO: bitcoincore_rpc does not yet support RPC batch calls which is quite
/// unfortunate. We will work around this using multiple async calls.
/// See https://github.com/rust-bitcoin/rust-bitcoincore-rpc/issues/24
async fn get_blocks(block_heights: Vec<u64>, client: Arc<Client>) -> Result<Vec<BlockHeader>> {
    let progress_bar = ProgressBar::new(block_heights.len() as u64);
    let mut result = Vec::new();
    let mut handles = Vec::new();

    for height in block_heights.iter() {
        handles.push(tokio::spawn(get_block(*height, Arc::clone(&client))));
    }

    for handle in handles {
        if let Ok(block) = handle.await {
            progress_bar.set_message(format!("Fetched block {}", block.block_hash()));
            result.push(block);
        } else {
            progress_bar.println("Error retrieving block");
        }

        progress_bar.inc(1);
    }

    println!("Finished fetching {} blocks.", result.len());
    Ok(result)
}

/// Get a block by block height.
async fn get_block(block_height: u64, client: Arc<Client>) -> BlockHeader {
    let hash = client.get_block_hash(block_height).unwrap();
    client.get_block_header(&hash).unwrap()
}

/// Display table column in minutes
fn display_mins(mins: &i64) -> String {
    format!("{} m", mins)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_sample_size() {
        let sample = BlockSample::new(1.96, 0.5, 0.05, false);
        assert_eq!(sample.get_sample_size(2000), 323);
    }

    #[test]
    fn test_get_random_heights() {
        let sample = BlockSample::new(1.96, 0.5, 0.05, false);
        let result = sample.get_random_heights(10 as u64, 2);
        assert_eq!(result.len(), 10);
    }
}
