use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(name = "Bitcoin Statistics")]
#[clap(author = "Tanveer Wahid <tan@wahid.email>")]
#[clap(version = "1.0")]
#[clap(about = "Run statistical analysis on Bitcoin blocks", long_about = None)]
pub struct Args {
    /// Path to config file if applicable
    #[clap(short, long, default_value_t = String::from("src/config"))]
    pub config: String,

    /// Analysis to run
    #[clap(subcommand)]
    pub command: Analysis,

    /// Z-Score for sampling
    #[clap(short, long, default_value_t = 1.96)]
    pub z_score: f64,

    /// Margin of error for sampling
    #[clap(short, long, default_value_t = 0.05)]
    pub margin_error: f64,

    /// Standard deviation for sampling
    #[clap(short, long, default_value_t = 0.5)]
    pub std_deviation: f64,

    /// Run the analysis on the full population
    /// Small hack as clap does not handle bools properly
    #[clap(short, long, parse(try_from_str), default_value = "false")]
    pub full_population: bool,
}

#[derive(Debug, Subcommand)]
pub enum Analysis {
    /// Run the drift time analysis using a drift time as unix seconds
    BlockTimeDrift {
        /// Time between two contiguous blocks
        #[clap(short, long, default_value_t = 7200)]
        drift_time: i64,
        /// Number of contiguous blocks within the sample
        #[clap(short, long, default_value_t = 2)]
        window: u64,
    },
}
