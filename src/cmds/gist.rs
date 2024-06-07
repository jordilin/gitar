use std::{io::Write, sync::Arc};

use crate::{
    api_traits::{CodeGist, Timestamp},
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
    pub files: String,
    pub created_at: String,
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
                Column::new("Files", gist.files),
                Column::new("URL", gist.url),
                Column::new("Description", gist.description),
                Column::new("Created at", gist.created_at),
            ],
        }
    }
}

impl Timestamp for Gist {
    fn created_at(&self) -> String {
        self.created_at.clone()
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
                .files("main.rs,hello_rust.rs".to_string())
                .created_at("2021-08-01T00:00:00Z".to_string())
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
            "Files|URL|Description|Created at\nmain.rs,hello_rust.rs|https://gist.github.com/aa5a315d61ae9438b18d|A gist|2021-08-01T00:00:00Z\n",
            String::from_utf8(buff).unwrap()
        );
    }
}
