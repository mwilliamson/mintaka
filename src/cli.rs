use std::path::PathBuf;

use clap::Parser;

use crate::config::MintakaConfig;

#[derive(Parser)]
struct CliArgs {
    #[arg(long, short)]
    config: PathBuf,
}

pub(crate) fn load_config() -> Result<MintakaConfig, crate::config::ConfigError> {
    let args = CliArgs::parse();
    super::config::load_config(&args.config)
}
