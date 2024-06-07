use std::{io::Write, sync::Arc};

use crate::{
    api_traits::CodeGist,
    display::{Column, DisplayBody},
    remote::{ListBodyArgs, ListRemoteCliArgs},
    Result,
};

use super::common;

#[derive(Builder)]
pub struct GistListCliArgs {
    pub list_args: ListRemoteCliArgs,
}

impl GistListCliArgs {
    pub fn builder() -> GistListCliArgsBuilder {
        GistListCliArgsBuilder::default()
    }
}

#[derive(Builder)]
pub struct GistListBodyArgs {
    pub body_args: Option<ListBodyArgs>,
}

impl GistListBodyArgs {
    pub fn builder() -> GistListBodyArgsBuilder {
        GistListBodyArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct Gist {
    pub url: String,
    pub description: String,
    pub file: String,
}

impl Gist {
    pub fn builder() -> GistBuilder {
        GistBuilder::default()
    }
}

impl From<Gist> for DisplayBody {
    fn from(gist: Gist) -> Self {
        DisplayBody {
            columns: vec![
                Column::new("File", gist.file),
                Column::new("URL", gist.url),
                Column::new("Description", gist.description),
            ],
        }
    }
}

pub fn list_user_gists<W: Write>(
    remote: Arc<dyn CodeGist>,
    body_args: GistListBodyArgs,
    cli_args: GistListCliArgs,
    writer: W,
) -> Result<()> {
    common::list_user_gists(remote, body_args, cli_args, writer)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct GistMock;

    impl CodeGist for GistMock {
        fn list(&self, _args: GistListBodyArgs) -> Result<Vec<Gist>> {
            let gist = Gist::builder()
                .url("https://gist.github.com/aa5a315d61ae9438b18d".to_string())
                .description("A gist".to_string())
                .file("main.rs".to_string())
                .build()
                .unwrap();
            Ok(vec![gist])
        }
    }

    #[test]
    fn test_list_user_gists() {
        let body_args = GistListBodyArgs::builder().body_args(None).build().unwrap();
        let cli_args = GistListCliArgs::builder()
            .list_args(ListRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        let mut buff = Vec::new();
        let remote = Arc::new(GistMock);
        assert!(list_user_gists(remote, body_args, cli_args, &mut buff).is_ok());
        assert_eq!(
            "File|URL|Description\nmain.rs|https://gist.github.com/aa5a315d61ae9438b18d|A gist\n",
            String::from_utf8(buff).unwrap()
        );
    }
}
