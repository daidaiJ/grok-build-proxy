use super::*;

const ROW: RowSize = RowSize {
    cols: 80,
    lines: 24,
};

fn ctx() -> StatusLineContext {
    crate::app::status_line::test_context(r"C:\Windows")
}

#[test]
fn drive_colon_and_non_pe_files_retry_under_the_shell() {
    assert!(should_retry_under_shell(&std::io::Error::from_raw_os_error(
        123
    )));
    assert!(should_retry_under_shell(&std::io::Error::from_raw_os_error(
        193
    )));
    assert!(should_retry_under_shell(&std::io::Error::from_raw_os_error(
        2
    )));
    assert!(!should_retry_under_shell(
        &std::io::Error::from_raw_os_error(5)
    ));
}

#[tokio::test]
async fn python_command_line_with_a_drive_path_runs() {
    let script = std::env::temp_dir().join("grok_status_line_probe.py");
    std::fs::write(&script, "print('ok')\n").expect("write probe script");
    let command = format!("python {}", script.display());
    assert!(
        command.contains(':'),
        "probe must include a drive colon so CreateProcess returns 123: {command}"
    );

    let outcome = run_status_command(&command, &ctx(), ROW, COMMAND_TIMEOUT).await;
    let text = match outcome {
        RunOutcome::Output(line) => line,
        RunOutcome::Failed { text, error } => panic!("{text}: {error}"),
    };
    assert_eq!(text.trim(), "ok", "got {text:?}");
}
