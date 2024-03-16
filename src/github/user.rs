use super::Github;
use crate::api_traits::UserInfo;
use crate::cmds::my::User;
use crate::io::{HttpRunner, Response};
use crate::Result;

impl<R: HttpRunner<Response = Response>> UserInfo for Github<R> {
    fn get(&self) -> Result<User> {
        todo!()
    }
}
