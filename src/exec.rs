use crate::Cmd;
use crate::Result;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;

/// Executes a sequence of commands in parallel
pub fn parallel_stream<T>(cmds: impl IntoIterator<Item = Cmd<T>>) -> Receiver<Result<T>>
where
    T: Send + 'static,
{
    let (sender, receiver) = channel();
    let mut cmd_handles = Vec::new();
    for cmd in cmds.into_iter() {
        let sender = sender.clone();
        let handle = std::thread::spawn(move || {
            let cmd_info = cmd();
            sender.send(cmd_info).unwrap_or_default();
        });
        cmd_handles.push(handle);
    }
    drop(sender);
    receiver
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_exec_one_single_cmd_ok() {
        let first_op_cmd = || -> Result<String> { Ok("1st op".to_string()) };
        let cmds: Vec<Cmd<String>> = vec![Box::new(first_op_cmd)];
        let repo_data_stream = parallel_stream(cmds);
        let results = repo_data_stream.iter().collect::<Vec<_>>();
        assert_eq!(1, results.len());
        assert_eq!("1st op", results[0].as_ref().unwrap());
    }

    #[test]
    fn test_exec_several_cmds_ok() {
        let first_op_cmd = || -> Result<String> { Ok("1st op".to_string()) };
        let second_op_cmd = || -> Result<String> { Ok("2nd op".to_string()) };
        let cmds: Vec<Cmd<String>> = vec![Box::new(first_op_cmd), Box::new(second_op_cmd)];
        let repo_data_stream = parallel_stream(cmds);
        let results = repo_data_stream.iter().collect::<Vec<_>>();
        assert_eq!(2, results.len());
    }
}
