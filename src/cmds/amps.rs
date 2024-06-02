use std::path::Path;

use crate::{
    dialog,
    io::{Response, TaskRunner},
    shell, Result,
};

pub fn execute(config_file: std::path::PathBuf) -> Result<()> {
    let base_path = config_file.parent().unwrap();
    let amps_scripts = base_path.join("amps");
    let runner = shell::Shell;
    let amps = list_amps(runner, amps_scripts.to_str().unwrap())?;
    let amp_script = dialog::fuzzy_select(amps)?;
    let stream_runner = shell::StreamingCommand;
    let amp_runner = Amp::new(dialog::prompt_args, &stream_runner);
    amp_runner.exec_amps(amp_script, base_path)?;
    Ok(())
}

fn list_amps(runner: impl TaskRunner<Response = Response>, amps_path: &str) -> Result<Vec<String>> {
    let cmd = vec!["ls", amps_path];
    let response = runner.run(cmd)?;
    let amps = response.body.split('\n').map(|s| s.to_string()).collect();
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
        let cmd = vec![cmd_path.to_str().unwrap(), &args];
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
}
