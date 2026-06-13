use netpilotd::cli::{CliCommand, parse_command};

#[test]
fn parse_show_status() {
    assert!(matches!(
        parse_command("show status"),
        CliCommand::ShowStatus
    ));
}

#[test]
fn parse_eval() {
    match parse_command("eval 1 + 1") {
        CliCommand::Eval { expr } => assert_eq!(expr, "1 + 1"),
        _ => panic!("expected Eval"),
    }
}

#[test]
fn parse_down() {
    assert!(matches!(parse_command("down"), CliCommand::Down));
}

#[test]
fn parse_help() {
    assert!(matches!(parse_command("help"), CliCommand::Help));
}

#[test]
fn parse_unknown() {
    assert!(matches!(
        parse_command("garbage cmd"),
        CliCommand::Unknown(_)
    ));
}

#[test]
fn parse_echo() {
    match parse_command("echo all") {
        CliCommand::Echo { classes, .. } => assert_eq!(classes, "all"),
        _ => panic!("expected Echo"),
    }
}

#[test]
fn parse_configure_soft() {
    assert!(matches!(
        parse_command("configure soft"),
        CliCommand::Configure { soft: true, .. }
    ));
}

#[test]
fn parse_show_route_filtered() {
    assert!(matches!(
        parse_command("show route filtered"),
        CliCommand::ShowRoute { filtered: true, .. }
    ));
}

#[test]
fn execute_eval_returns_placeholder() {
    let cmd = parse_command("eval bgp_path.first");
    let output = netpilotd::cli::execute_command(&cmd);
    assert!(output.contains("not yet implemented"));
}

#[test]
fn execute_help_returns_commands() {
    let cmd = CliCommand::Help;
    let output = netpilotd::cli::execute_command(&cmd);
    assert!(output.contains("show status"));
    assert!(output.contains("eval"));
    assert!(output.contains("down"));
}
