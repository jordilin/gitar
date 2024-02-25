use crate::remote::ListRemoteCliArgs;

pub struct DockerListCliArgs {
    pub list_args: ListRemoteCliArgs,
}

impl DockerListCliArgs {
    pub fn new(list_args: ListRemoteCliArgs) -> DockerListCliArgs {
        DockerListCliArgs { list_args }
    }
}
