use std::path::Path;

use crate::{
    cli::amps::AmpsOptions::{self, Exec},
    dialog,
    error::GRError,
    io::{Response, TaskRunner},
    shell, Result,
};

pub fn execute(options: AmpsOptions, config_file: std::path::PathBuf) -> Result<()> {
    match options {
        Exec(amp_name_args) => {
            let base_path = config_file.parent().unwrap();
            let amps_scripts = base_path.join("amps");
            if amp_name_args.is_empty() {
                let runner = shell::BlockingCommand;
                let amps = list_amps(runner, amps_scripts.to_str().unwrap())?;
                let amp_script = dialog::fuzzy_select(amps)?;
                let stream_runner = shell::StreamingCommand;
                let amp_runner = Amp::new(dialog::prompt_args, &stream_runner);
                amp_runner.exec_amps(amp_script, base_path)?;
                return Ok(());
            }
            let stream_runner = shell::StreamingCommand;
            let amp_name_args: Vec<&str> = amp_name_args.split(' ').collect();
            let amp_name = amp_name_args[0];
            let amp_path = amps_scripts.join(amp_name);
            let args = amp_name_args[1..].join(" ");
            stream_runner.run(vec![&amp_path.to_str().unwrap(), &args.as_str()])?;
            Ok(())
        }
        _ => {
            let base_path = config_file.parent().unwrap();
            let amps_scripts = base_path.join("amps");
            let runner = shell::BlockingCommand;
            let amps = list_amps(runner, amps_scripts.to_str().unwrap())?;
            for amp in amps {
                println!("{}", amp);
            }
            Ok(())
        }
    }
}

fn list_amps(runner: impl TaskRunner<Response = Response>, amps_path: &str) -> Result<Vec<String>> {
    let cmd = vec!["ls", amps_path];
    let response = runner.run(cmd)?;
    if response.body.is_empty() {
        return Err(GRError::PreconditionNotMet(format!(
            "No amps are available in {}. Please check \
            https://github.com/jordilin/gitar-amps for installation instructions",
            amps_path
        ))
        .into());
    }
    let amps: Vec<String> = response.body.split('\n').map(|s| s.to_string()).collect();
    Ok(amps)
}

enum AmpPrompts {
    Args,
    Help,
    Exit,
}

impl From<&String> for AmpPrompts {
    fn from(s: &String) -> Self {
        match s.as_str() {
            "-h" | "--help" | "help" | "h" => AmpPrompts::Help,
            "exit" | "cancel" | "quit" | "q" => AmpPrompts::Exit,
            _ => AmpPrompts::Args,
        }
    }
}

struct Amp<'a, A, R> {
    args_prompter_fn: A,
    runner: &'a R,
}

impl<'a, A, R> Amp<'a, A, R> {
    fn new(args: A, runner: &'a R) -> Self {
        Self {
            args_prompter_fn: args,
            runner,
        }
    }
}

impl<'a, A: Fn() -> String, R: TaskRunner<Response = Response>> Amp<'a, A, R> {
    fn exec_amps(&self, amp: String, base_path: &Path) -> Result<()> {
        let mut args = (self.args_prompter_fn)();
        loop {
            match (&args).into() {
                AmpPrompts::Help => {
                    let cmd_path = base_path.join("amps").join(&amp);
                    let cmd = vec![cmd_path.to_str().unwrap(), "--help"];
                    self.runner.run(cmd)?;
                    args = (self.args_prompter_fn)();
                }
                AmpPrompts::Exit => {
                    return Ok(());
                }
                AmpPrompts::Args => break,
            }
        }
        let cmd_path = base_path.join("amps").join(amp);
        let mut cmd = vec![cmd_path.to_str().unwrap()];
        for arg in args.split_whitespace() {
            cmd.push(arg);
        }
        self.runner.run(cmd)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use crate::test::utils::MockRunner;

    use super::*;

    #[test]
    fn test_exec_amp_with_help_and_run() {
        let response_help = Response::builder()
            .status(0)
            .body("this is the help".to_string())
            .build()
            .unwrap();
        let response = Response::builder()
            .status(0)
            .body("response output".to_string())
            .build()
            .unwrap();
        let amp_script = "list_releases".to_string();
        let runner = MockRunner::new(vec![response, response_help]);
        let prompted_args = RefCell::new(vec![
            "github.com/jordilin/gitar".to_string(),
            "help".to_string(),
        ]);
        let args_prompter_fn = || prompted_args.borrow_mut().pop().unwrap();
        let amp_runner = Amp::new(args_prompter_fn, &runner);
        let base_path = Path::new("/tmp");
        assert!(amp_runner.exec_amps(amp_script, base_path).is_ok());
        assert_eq!(2, *runner.run_count.borrow());
    }

    #[test]
    fn test_exec_amp_with_help_and_exit() {
        let response_help = Response::builder()
            .status(0)
            .body("this is the help".to_string())
            .build()
            .unwrap();
        let amp_script = "list_releases".to_string();
        let runner = MockRunner::new(vec![response_help]);
        let prompted_args = RefCell::new(vec!["exit".to_string(), "help".to_string()]);
        let args_prompter_fn = || prompted_args.borrow_mut().pop().unwrap();
        let amp_runner = Amp::new(args_prompter_fn, &runner);
        let base_path = Path::new("/tmp");
        assert!(amp_runner.exec_amps(amp_script, base_path).is_ok());
        assert_eq!(1, *runner.run_count.borrow());
    }

    #[test]
    fn test_list_amps_error_if_none_available() {
        let response = Response::builder()
            .status(0)
            .body("".to_string())
            .build()
            .unwrap();
        let runner = MockRunner::new(vec![response]);
        let amps_path = "/tmp/amps";
        let amps = list_amps(runner, amps_path);
        match amps {
            Err(err) => match err.downcast_ref::<GRError>() {
                Some(GRError::PreconditionNotMet(msg)) => {
                    assert_eq!(
                        "No amps are available in /tmp/amps. Please check \
                        https://github.com/jordilin/gitar-amps for installation instructions",
                        msg
                    );
                }
                _ => panic!("Expected error"),
            },
            Ok(_) => panic!("Expected error"),
        }
    }
}
