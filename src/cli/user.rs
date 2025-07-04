use clap::Parser;

use crate::cmds::user::UserCliArgs;

use super::common::GetArgs;

#[derive(Parser)]
pub struct UserCommand {
    #[clap(subcommand)]
    subcommand: UserSubCommand,
}

#[derive(Parser)]
enum UserSubCommand {
    #[clap(about = "Gets user information")]
    Get(GetUser),
}

#[derive(Parser)]
struct GetUser {
    /// Retrieve user information by username
    #[clap()]
    username: String,
    #[clap(flatten)]
    get_args: GetArgs,
}

impl From<UserCommand> for UserOptions {
    fn from(cmd: UserCommand) -> Self {
        match cmd.subcommand {
            UserSubCommand::Get(options) => options.into(),
        }
    }
}

impl From<GetUser> for UserOptions {
    fn from(options: GetUser) -> Self {
        UserOptions::Get(
            UserCliArgs::builder()
                .username(options.username)
                .get_args(options.get_args.into())
                .build()
                .unwrap(),
        )
    }
}

pub enum UserOptions {
    Get(UserCliArgs),
}

#[cfg(test)]
mod tests {
    use crate::cli::{Args, Command};

    use super::*;

    #[test]
    fn test_user_command() {
        let args = Args::parse_from(["gr", "us", "get", "octocat"]);
        let user_command = match args.command {
            Command::User(cmd) => cmd,
            _ => panic!("Expected user command"),
        };
        let options: UserOptions = user_command.into();
        match options {
            UserOptions::Get(args) => {
                assert_eq!(args.username, "octocat");
            }
        }
    }
}
