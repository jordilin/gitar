use std::{io::Write, sync::Arc};

use crate::{
    api_traits::UserInfo,
    cli::user::UserOptions,
    config::ConfigProperties,
    display,
    remote::{self, CacheType, GetRemoteCliArgs},
    Result,
};

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

pub fn execute(
    options: UserOptions,
    config: Arc<dyn ConfigProperties>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        UserOptions::Get(args) => {
            let remote = remote::get_user(
                domain,
                path,
                config,
                Some(&args.get_args.cache_args),
                CacheType::File,
            )?;
            get_merge_request_details(remote, &args, std::io::stdout())
        }
    }
}

pub fn get_merge_request_details<W: Write>(
    remote: Arc<dyn UserInfo>,
    args: &UserCliArgs,
    mut writer: W,
) -> Result<()> {
    let response = remote.get(args)?;
    display::print(&mut writer, vec![response], args.get_args.clone())?;
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::cmds::project::Member;

    struct MockUserInfo;

    impl MockUserInfo {
        fn new() -> Self {
            MockUserInfo {}
        }
    }

    impl UserInfo for MockUserInfo {
        fn get_auth_user(&self) -> Result<Member> {
            Ok(Member::builder().build().unwrap())
        }

        fn get(&self, _args: &UserCliArgs) -> Result<Member> {
            Ok(Member::builder()
                .username("tomsawyer".to_string())
                .id(1)
                .build()
                .unwrap())
        }
    }

    #[test]
    fn test_get_user_details() {
        let remote = MockUserInfo::new();
        let args = UserCliArgs::builder()
            .username("test".to_string())
            .get_args(GetRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        let mut writer = Vec::new();
        get_merge_request_details(Arc::new(remote), &args, &mut writer).unwrap();
        assert_eq!(
            "ID|Name|Username\n1||tomsawyer\n",
            String::from_utf8(writer).unwrap()
        );
    }
}
