use clap::Parser;

#[derive(Parser)]
pub struct CacheCommand {
    #[clap(subcommand)]
    subcommand: CacheSubcommand,
}

#[derive(Parser)]
enum CacheSubcommand {
    #[clap(name = "info", about = "Get local cache size and location")]
    Info,
}

pub enum CacheOptions {
    Info,
}

impl From<CacheCommand> for CacheOptions {
    fn from(options: CacheCommand) -> Self {
        match options.subcommand {
            CacheSubcommand::Info => CacheOptions::Info,
        }
    }
}
