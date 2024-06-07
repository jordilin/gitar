use crate::cli::common::ListArgs;

#[derive(Builder)]
pub struct GistListCliArgs {
    pub list_args: ListArgs,
}

impl GistListCliArgs {
    pub fn builder() -> GistListCliArgsBuilder {
        GistListCliArgsBuilder::default()
    }
}
pub struct GistListBodyArgs {
    pub list_args: Option<ListArgs>,
}

pub struct Gist {
    pub url: String,
    pub description: String,
    pub file: String,
}
