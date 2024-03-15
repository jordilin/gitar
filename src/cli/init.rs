use clap::Parser;

#[derive(Parser)]
pub struct InitCommand {
    #[clap(long)]
    pub domain: String,
}

pub struct InitCommandOptions {
    pub domain: String,
}

impl From<InitCommand> for InitCommandOptions {
    fn from(options: InitCommand) -> Self {
        InitCommandOptions {
            domain: options.domain,
        }
    }
}
