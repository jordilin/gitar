use gr::io::Runner;

#[test]
fn test_execute_shell_cmd_ok() {
    let runner = gr::shell::Shell;
    let cmd = ["echo", "hello"];
    let response = runner.run(cmd).unwrap();
    assert_eq!(response.status, 0);
    assert_eq!(response.body, "hello");
}

#[test]
fn test_execute_shell_cmd_with_error_cmd_does_not_exist() {
    let runner = gr::shell::Shell;
    let cmd = ["/bin/doesnotexist", "test"];
    let err = runner.run(cmd).unwrap_err();
    assert_eq!(err.to_string(), "No such file or directory (os error 2)");
}

#[test]
fn test_execute_shell_cmd_with_error_cmd_fails_to_stderr() {
    let runner = gr::shell::Shell;
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let err_cmd = format!("{}/tests/fail_msg_to_stderr.sh", manifest_dir);
    let cmd = ["sh", "-c", &err_cmd];
    // let cmd = ["sh", "-c", "./tests/fail_msg_to_stderr.sh"];
    let err = runner.run(cmd).unwrap_err();
    assert_eq!(err.to_string(), "This is a failure message");
}
