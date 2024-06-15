use clap::Parser;

#[derive(Parser)]
pub struct AmpsCommand {
    #[clap(subcommand)]
    subcommand: Option<AmpsSubcommand>,
}

#[derive(Parser)]
enum AmpsSubcommand {
    #[clap(about = "List available amps")]
    List,
    #[clap(
        name = "exec",
        about = "Execute an amp, either by name or through prompt",
        alias = "ex"
    )]
    Exec(ExecCommand),
}

#[derive(Parser)]
struct ExecCommand {
    /// The name of the amp to execute
    #[clap()]
    pub name: Option<String>,
}

pub enum AmpsOptions {
    List,
    Exec(String),
}

impl From<AmpsCommand> for AmpsOptions {
    fn from(options: AmpsCommand) -> Self {
        match options.subcommand {
            Some(AmpsSubcommand::List) => AmpsOptions::List,
            Some(AmpsSubcommand::Exec(options)) => options.into(),
            // defaults to list available amps
            None => AmpsOptions::List,
        }
    }
}

impl From<ExecCommand> for AmpsOptions {
    fn from(options: ExecCommand) -> Self {
        match options.name {
            Some(name) => AmpsOptions::Exec(name),
            // defaults to execute amp through prompt
            None => AmpsOptions::Exec(String::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::{Args, Command};

    use super::*;

    #[test]
    fn test_amps_list_command() {
        let args = Args::parse_from(vec!["gr", "amps", "list"]);
        match args.command {
            Command::Amps(AmpsCommand {
                subcommand: Some(AmpsSubcommand::List),
            }) => {}
            _ => panic!("Expected Amp ListCommand"),
        }
    }

    #[test]
    fn test_amps_exec_command() {
        let args = Args::parse_from(vec!["gr", "amps", "exec", "amp-name"]);
        match args.command {
            Command::Amps(AmpsCommand {
                subcommand: Some(AmpsSubcommand::Exec(ExecCommand { name })),
            }) => {
                assert_eq!(name, Some("amp-name".to_string()));
            }
            _ => panic!("Expected Amp ExecCommand"),
        }
    }
}
