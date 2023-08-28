use gr::io::Runner;
use std::fs::File;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

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
    let failure_sh = r#"
    #!/usr/bin/env bash

    # echo msg to stderr and exit with failure
    fail_msg_to_stderr() {
        echo -n "$1" 1>&2
        exit 2
    }

    fail_msg_to_stderr "This is a failure message"
    "#;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("fail_msg_to_stderr.sh");
    let mut file = File::create(&file_path).unwrap();
    let mut perms = std::fs::metadata(&file_path).unwrap().permissions();

    file.write_all(failure_sh.as_bytes()).unwrap();
    perms.set_mode(0o755);
    file.set_permissions(perms).unwrap();
    // flush and close file
    drop(file);
    let cmd = ["sh", "-c", &file_path.to_string_lossy()];
    let err = runner.run(cmd).unwrap_err();
    assert_eq!(err.to_string(), "This is a failure message");
}
