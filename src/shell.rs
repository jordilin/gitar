use crate::error;
use crate::io::Response;
use crate::io::ResponseBuilder;
use crate::io::Runner;
use crate::Result;
use std::ffi::OsStr;
use std::process;
use std::str;

pub struct Shell;

impl Runner for Shell {
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
    let mut response_builder = ResponseBuilder::default();
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
