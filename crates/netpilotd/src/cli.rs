/// Represents a parsed CLI command.
#[derive(Clone, Debug, PartialEq)]
pub enum CliCommand {
    // Status
    ShowStatus,
    ShowProtocols { all: bool, name: Option<String> },
    ShowInterfaces { summary: bool },
    ShowRoute { prefix: Option<String>, table: Option<String>, filter: Option<String>, filtered: bool, count: bool },
    ShowSymbols { kind: Option<String> },
    ShowBfdSessions,
    ShowRpkI,
    ShowMemory,
    ShowMplsLabels,
    ShowSrPrefixSids,
    ShowSrv6Sids,
    ShowIsisTopology,
    ShowIsisAdjacencies,
    ShowIsisDatabase,
    ShowEigrpNeighbors,
    ShowEigrpTopology,
    ShowEigrpRoutes,
    ShowBgpLs,
    ShowBgpFlowspec,

    // Config
    Configure { file: Option<String>, soft: bool, timeout: Option<u32> },
    ConfigureCheck { file: Option<String> },
    ConfigureConfirm,
    ConfigureUndo,
    ConfigureCommit { author: String, note: String },
    ConfigureRollback { revision_id: u64 },

    // Protocol control
    Enable { name: String },
    Disable { name: String },
    Restart { name: String },
    Reload { name: String, direction: Option<String> },

    // Debug & diagnostics
    Eval { expr: String },
    Dump { kind: String, file: String },
    Debug { target: String, flags: String },
    Echo { classes: String, buffer_size: Option<usize> },
    TimeFormat { format: String, limit: Option<String> },

    // System
    Down,
    GracefulRestart,
    Help,

    // Unknown
    Unknown(String),
}

/// Parse a raw CLI input string into a CliCommand.
pub fn parse_command(input: &str) -> CliCommand {
    let input = input.trim();
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.is_empty() {
        return CliCommand::Unknown(String::new());
    }

    match parts[0] {
        "show" => parse_show(&parts[1..]),
        "configure" => parse_configure(&parts[1..]),
        "enable" => CliCommand::Enable { name: parts.get(1).map(|s| s.to_string()).unwrap_or_default() },
        "disable" => CliCommand::Disable { name: parts.get(1).map(|s| s.to_string()).unwrap_or_default() },
        "restart" => CliCommand::Restart { name: parts.get(1).map(|s| s.to_string()).unwrap_or_default() },
        "reload" => {
            let name = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
            let direction = parts.get(2).map(|s| s.to_string());
            CliCommand::Reload { name, direction }
        }
        "eval" => CliCommand::Eval { expr: parts[1..].join(" ") },
        "dump" => CliCommand::Dump {
            kind: parts.get(1).map(|s| s.to_string()).unwrap_or_default(),
            file: parts.get(2).map(|s| s.to_string()).unwrap_or_default(),
        },
        "debug" => CliCommand::Debug {
            target: parts.get(1).map(|s| s.to_string()).unwrap_or_default(),
            flags: parts[2..].join(" "),
        },
        "echo" => CliCommand::Echo {
            classes: parts[1..].join(" "),
            buffer_size: None,
        },
        "timeformat" => CliCommand::TimeFormat {
            format: parts.get(1).map(|s| s.to_string()).unwrap_or_default(),
            limit: parts.get(2).map(|s| s.to_string()),
        },
        "down" => CliCommand::Down,
        "graceful" if parts.get(1) == Some(&"restart") => CliCommand::GracefulRestart,
        "help" | "?" => CliCommand::Help,
        _ => CliCommand::Unknown(input.to_string()),
    }
}

fn parse_show(parts: &[&str]) -> CliCommand {
    match parts.first().copied() {
        Some("status") => CliCommand::ShowStatus,
        Some("protocols") => CliCommand::ShowProtocols {
            all: parts.contains(&"all"),
            name: parts.iter().find(|p| !matches!(**p, "protocols" | "all")).map(|s| s.to_string()),
        },
        Some("interfaces") => CliCommand::ShowInterfaces { summary: parts.contains(&"summary") },
        Some("route") => {
            let filtered = parts.contains(&"filtered");
            let count = parts.contains(&"count");
            CliCommand::ShowRoute {
                prefix: parts.iter().find(|p| p.contains('/')).map(|s| s.to_string()),
                table: parts.windows(2).find(|w| w[0] == "table").map(|w| w[1].to_string()),
                filter: parts.windows(2).find(|w| w[0] == "filter" || w[0] == "where").map(|w| w[1..].join(" ")),
                filtered,
                count,
            }
        }
        Some("symbols") => CliCommand::ShowSymbols { kind: parts.get(1).map(|s| s.to_string()) },
        Some("bfd") => CliCommand::ShowBfdSessions,
        Some("rpki") => CliCommand::ShowRpkI,
        Some("memory") => CliCommand::ShowMemory,
        Some("mpls") if parts.get(1) == Some(&"labels") => CliCommand::ShowMplsLabels,
        Some("sr") if parts.get(1) == Some(&"prefix-sids") => CliCommand::ShowSrPrefixSids,
        Some("srv6") if parts.get(1) == Some(&"sids") => CliCommand::ShowSrv6Sids,
        Some("isis") if parts.get(1) == Some(&"topology") => CliCommand::ShowIsisTopology,
        Some("isis") if parts.get(1) == Some(&"adjacencies") => CliCommand::ShowIsisAdjacencies,
        Some("isis") if parts.get(1) == Some(&"database") => CliCommand::ShowIsisDatabase,
        Some("eigrp") if parts.get(1) == Some(&"neighbors") => CliCommand::ShowEigrpNeighbors,
        Some("eigrp") if parts.get(1) == Some(&"topology") => CliCommand::ShowEigrpTopology,
        Some("eigrp") if parts.get(1) == Some(&"routes") => CliCommand::ShowEigrpRoutes,
        Some("bgp") if parts.get(1) == Some(&"link-state") => CliCommand::ShowBgpLs,
        Some("bgp") if parts.get(1) == Some(&"flowspec") => CliCommand::ShowBgpFlowspec,
        _ => CliCommand::Unknown(format!("show {}", parts.join(" "))),
    }
}

fn parse_configure(parts: &[&str]) -> CliCommand {
    match parts.first().copied() {
        Some("check") => CliCommand::ConfigureCheck { file: parts.get(1).map(|s| s.to_string()) },
        Some("confirm") => CliCommand::ConfigureConfirm,
        Some("undo") => CliCommand::ConfigureUndo,
        Some("soft") => CliCommand::Configure { file: parts.get(1).map(|s| s.to_string()), soft: true, timeout: None },
        _ => {
            let soft = parts.contains(&"soft");
            let timeout = parts.windows(2).find(|w| w[0] == "timeout").and_then(|w| w[1].parse::<u32>().ok());
            CliCommand::Configure { file: parts.first().map(|s| s.to_string()), soft, timeout }
        }
    }
}

/// Execute a CLI command against the application state.
/// Returns the output string to send back to the client.
pub fn execute_command(cmd: &CliCommand) -> String {
    match cmd {
        CliCommand::ShowStatus => "NetPilot daemon running\n".to_string(),

        CliCommand::ShowProtocols { all, name: _ } => {
            if *all { "All protocols:\n".to_string() }
            else { "Protocols:\n".to_string() }
        }

        CliCommand::Eval { expr } => {
            format!("eval: filter VM not yet implemented — cannot evaluate: {expr}\n")
        }

        CliCommand::Dump { kind, file } => {
            format!("dump: state export not yet implemented — {kind} -> {file}\n")
        }

        CliCommand::Debug { target, flags } => {
            format!("debug {target} {flags}\n")
        }

        CliCommand::Echo { classes, .. } => {
            format!("echo: {classes}\n")
        }

        CliCommand::TimeFormat { format, limit } => {
            let limit_str = limit.as_deref().unwrap_or("none");
            format!("timeformat: {format} (limit: {limit_str})\n")
        }

        CliCommand::Down => {
            "shutting down...\n".to_string()
        }

        CliCommand::GracefulRestart => {
            "graceful restart: not yet implemented (requires protocol state)\n".to_string()
        }

        CliCommand::ShowRoute { filtered: true, .. } => {
            "show route filtered: not yet implemented (requires RIB with import keep filtered)\n".to_string()
        }

        CliCommand::Help => {
            let mut help = String::from("Available commands:\n");
            help.push_str("  show status | protocols | interfaces | route | symbols | bfd | rpki | memory\n");
            help.push_str("  configure [soft] [check] [confirm | undo] [timeout <n>]\n");
            help.push_str("  enable | disable | restart | reload <name>\n");
            help.push_str("  eval <expr>\n");
            help.push_str("  dump <kind> <file>\n");
            help.push_str("  debug <target> <flags>\n");
            help.push_str("  echo <classes>\n");
            help.push_str("  timeformat <format> [limit <format>]\n");
            help.push_str("  down | graceful restart\n");
            help
        }

        CliCommand::ShowBgpLs => {
            "show bgp link-state: BGP-LS not configured\n".to_string()
        }
        CliCommand::ShowBgpFlowspec => {
            "show bgp flowspec: flowspec not configured\n".to_string()
        }

        CliCommand::ShowMplsLabels => {
            "show mpls labels: no MPLS table routes loaded yet\n".to_string()
        }

        CliCommand::ShowSrPrefixSids => {
            "show sr prefix-sids: no IGP topology loaded yet\n".to_string()
        }
        CliCommand::ShowSrv6Sids => {
            "show srv6 sids: no SRv6 dataplane configured yet\n".to_string()
        }
        CliCommand::ShowIsisTopology => {
            "show isis topology: IS-IS protocol not started\n".to_string()
        }
        CliCommand::ShowIsisAdjacencies => {
            "show isis adjacencies: IS-IS protocol not started\n".to_string()
        }
        CliCommand::ShowIsisDatabase => {
            "show isis database: IS-IS protocol not started\n".to_string()
        }
        CliCommand::ShowEigrpNeighbors => {
            "show eigrp neighbors: EIGRP protocol not started\n".to_string()
        }
        CliCommand::ShowEigrpTopology => {
            "show eigrp topology: EIGRP protocol not started\n".to_string()
        }
        CliCommand::ShowEigrpRoutes => {
            "show eigrp routes: EIGRP protocol not started\n".to_string()
        }

        CliCommand::Unknown(input) => {
            format!("Unknown command: {input}. Type 'help' for available commands.\n")
        }

        // Config commands that need state — handled by API layer
        _ => "This command must be executed via the API\n".to_string(),
    }
}
