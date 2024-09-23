use crate::remote::GetRemoteCliArgs;

#[derive(Builder)]
pub struct UserCliArgs {
    pub username: String,
    pub get_args: GetRemoteCliArgs,
}

impl UserCliArgs {
    pub fn builder() -> UserCliArgsBuilder {
        UserCliArgsBuilder::default()
    }
}
