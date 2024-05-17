use clap::Parser;

use crate::cmds::trending::TrendingCliArgs;

use super::common::GetArgs;

#[derive(Parser)]
pub struct TrendingCommand {
    #[clap(long, value_parser(validate_trends_domain))]
    pub domain: String,
    #[clap()]
    pub language: String,
    #[clap(flatten)]
    get_args: GetArgs,
}

pub enum TrendingOptions {
    Get(TrendingCliArgs),
}

impl From<TrendingCommand> for TrendingOptions {
    fn from(options: TrendingCommand) -> Self {
        TrendingOptions::Get(TrendingCliArgs {
            domain: options.domain,
            language: options.language,
            get_args: options.get_args.into(),
            flush: false,
        })
    }
}

impl From<TrendingOptions> for TrendingCliArgs {
    fn from(options: TrendingOptions) -> Self {
        match options {
            TrendingOptions::Get(args) => args,
        }
    }
}

fn validate_trends_domain(domain: &str) -> Result<String, String> {
    if domain == "github.com" {
        Ok(domain.to_string())
    } else {
        Err("Trending projects implemented for github.com only".to_string())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_validate_trends_domain() {
        assert_eq!("github.com", validate_trends_domain("github.com").unwrap());
        assert!(validate_trends_domain("github.foo.com").is_err());
        assert!(validate_trends_domain("gitlab.com").is_err());
    }
}
