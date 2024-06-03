use crate::error;
use crate::io::Response;
use crate::io::TaskRunner;
use crate::Result;
use std::ffi::OsStr;
use std::io::BufRead;
use std::io::BufReader;
use std::process;
use std::process::Command;
use std::str;
use std::thread;

pub struct BlockingCommand;

impl TaskRunner for BlockingCommand {
    type Response = Response;

    fn run<T>(&self, cmd: T) -> Result<Self::Response>
    where
        T: IntoIterator,
        T::Item: AsRef<OsStr>,
    {
        run_args(cmd)
    }
}

fn run_args<T>(args: T) -> Result<Response>
where
    T: IntoIterator,
    T::Item: AsRef<OsStr>,
{
    let args: Vec<_> = args.into_iter().collect();
    let mut process = process::Command::new(&args[0]);
    process.args(&args[1..]);
    let mut response_builder = Response::builder();
    match process.output() {
        Ok(output) => {
            let status_code = output.status.code().unwrap_or(0);
            if output.status.success() {
                let output_str = str::from_utf8(&output.stdout)?;
                if let Some(output_stripped) = output_str.strip_suffix('\n') {
                    return Ok(response_builder
                        .status(status_code)
                        .body(output_stripped.to_string())
                        .build()?);
                };
                return Ok(response_builder
                    .status(status_code)
                    .body(output_str.to_string())
                    .build()?);
            }
            let err_msg = str::from_utf8(&output.stderr)?;
            Err(error::gen(err_msg))
        }
        Err(val) => Err(error::gen(val.to_string())),
    }
}

pub struct StreamingCommand;

impl TaskRunner for StreamingCommand {
    type Response = Response;

    fn run<T>(&self, cmd: T) -> Result<Self::Response>
    where
        T: IntoIterator,
        T::Item: AsRef<std::ffi::OsStr>,
    {
        let args: Vec<_> = cmd.into_iter().collect();
        let cmd_path = &args[0];
        let args = args[1..]
            .iter()
            .map(|s| s.as_ref().to_str().unwrap())
            .collect::<Vec<&str>>()
            .join(" ");
        let mut child = Command::new(cmd_path)
            .arg(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stdout = BufReader::new(child.stdout.take().unwrap());
        let stderr = BufReader::new(child.stderr.take().unwrap());

        let stdout_handle = thread::spawn(move || {
            for line in stdout.lines() {
                println!("{}", line.unwrap());
            }
        });
        let stderr_handle = thread::spawn(move || {
            for line in stderr.lines() {
                eprintln!("{}", line.unwrap());
            }
        });
        stdout_handle.join().unwrap();
        stderr_handle.join().unwrap();
        let _ = child.wait()?;
        Ok(Response::builder().status(0).body("".to_string()).build()?)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_run() {
        let runner = StreamingCommand;
        let cmd = vec!["echo", "Hello, world!"];
        let response = runner.run(cmd).unwrap();
        assert_eq!(response.body, "");
    }

    #[test]
    #[should_panic(expected = "No such file or directory (os error 2)")]
    fn test_run_invalid_command() {
        let runner = StreamingCommand;
        let cmd = vec!["invalid_command"];
        let _ = runner.run(cmd).unwrap();
    }
}
