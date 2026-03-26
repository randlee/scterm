use super::{Action, CliError, GlobalOptions, SessionCommand};
use scterm_core::LogCap;
use std::collections::VecDeque;
use std::env;

pub(super) fn parse_cli(argv: &[String]) -> Result<Action, CliError> {
    if argv.is_empty() {
        return Ok(Action::Help);
    }

    if matches!(argv[0].as_str(), "--help" | "-h" | "?") {
        return Ok(Action::Help);
    }
    if argv[0] == "--version" {
        return Ok(Action::Version);
    }

    let mut queue = argv.iter().cloned().collect::<VecDeque<_>>();
    let mut global = GlobalOptions::default();
    parse_global_options(&mut queue, &mut global, true)?;
    if queue.is_empty() {
        return Ok(Action::Help);
    }

    if let Some(mode) = legacy_mode(&queue[0]) {
        queue.pop_front();
        return parse_legacy_mode(mode, &mut queue, global);
    }

    let head = queue.front().cloned().expect("checked non-empty");
    match head.as_str() {
        "__internal-master" => parse_internal_master(queue),
        "attach" | "a" => {
            queue.pop_front();
            parse_attach_like(queue, global).map(Action::Attach)
        }
        "new" | "n" => {
            queue.pop_front();
            parse_attach_like(queue, global).map(Action::New)
        }
        "start" | "s" => {
            queue.pop_front();
            parse_attach_like(queue, global).map(Action::Start)
        }
        "run" => {
            queue.pop_front();
            parse_attach_like(queue, global).map(Action::Run)
        }
        "push" | "p" => {
            queue.pop_front();
            let session = queue
                .pop_front()
                .ok_or_else(|| CliError::usage("No session was specified."))?;
            if !queue.is_empty() {
                return Err(CliError::usage("Invalid number of arguments."));
            }
            Ok(Action::Push { session })
        }
        "kill" | "k" => {
            queue.pop_front();
            parse_kill(queue).map(|(session, force)| Action::Kill { session, force })
        }
        "clear" => {
            queue.pop_front();
            parse_clear(queue, global.quiet)
        }
        "list" | "l" | "ls" => Ok(Action::List { options: global }),
        "current" => Ok(Action::Current),
        _ => parse_open(queue, global).map(Action::Open),
    }
}

fn parse_attach_like(
    mut args: VecDeque<String>,
    mut options: GlobalOptions,
) -> Result<SessionCommand, CliError> {
    parse_global_options(&mut args, &mut options, false)?;
    let session = args
        .pop_front()
        .ok_or_else(|| CliError::usage("No session was specified."))?;
    parse_global_options(&mut args, &mut options, false)?;
    let child_command = child_command_from_args(args);
    Ok(SessionCommand {
        options,
        session,
        child_command,
    })
}

fn parse_open(
    mut args: VecDeque<String>,
    mut options: GlobalOptions,
) -> Result<SessionCommand, CliError> {
    let session = args
        .pop_front()
        .ok_or_else(|| CliError::usage("No session was specified."))?;
    parse_global_options(&mut args, &mut options, false)?;
    let child_command = child_command_from_args(args);
    Ok(SessionCommand {
        options,
        session,
        child_command,
    })
}

fn parse_kill(mut args: VecDeque<String>) -> Result<(String, bool), CliError> {
    let mut force = false;
    while args
        .front()
        .is_some_and(|arg| arg == "-f" || arg == "--force")
    {
        force = true;
        args.pop_front();
    }
    let session = args
        .pop_front()
        .ok_or_else(|| CliError::usage("No session was specified."))?;
    while args
        .front()
        .is_some_and(|arg| arg == "-f" || arg == "--force")
    {
        force = true;
        args.pop_front();
    }
    if !args.is_empty() {
        return Err(CliError::usage("Invalid number of arguments."));
    }
    Ok((session, force))
}

fn parse_clear(mut args: VecDeque<String>, quiet: bool) -> Result<Action, CliError> {
    let session = args.pop_front();
    if !args.is_empty() {
        return Err(CliError::usage("Invalid number of arguments."));
    }
    Ok(Action::Clear { session, quiet })
}

fn parse_internal_master(mut args: VecDeque<String>) -> Result<Action, CliError> {
    args.pop_front();
    let session_path = args
        .pop_front()
        .ok_or_else(|| CliError::usage("Missing internal session path."))?;
    let log_cap_bytes = args
        .pop_front()
        .ok_or_else(|| CliError::usage("Missing internal log cap."))?
        .parse::<u64>()
        .map_err(|_| CliError::usage("Internal log cap is invalid."))?;
    if args.front().is_some_and(|arg| arg == "--") {
        args.pop_front();
    }
    let child_command = child_command_from_args(args);
    Ok(Action::InternalMaster {
        session_path,
        log_cap_bytes,
        child_command,
    })
}

fn parse_legacy_mode(
    mode: char,
    args: &mut VecDeque<String>,
    options: GlobalOptions,
) -> Result<Action, CliError> {
    match mode {
        'l' => Ok(Action::List { options }),
        'i' => Ok(Action::Current),
        'a' => parse_attach_like(std::mem::take(args), options).map(Action::Attach),
        'A' => parse_open(std::mem::take(args), options).map(Action::Open),
        'c' => parse_attach_like(std::mem::take(args), options).map(Action::New),
        'n' => parse_attach_like(std::mem::take(args), options).map(Action::Start),
        'N' => parse_attach_like(std::mem::take(args), options).map(Action::Run),
        'p' => {
            let session = args
                .pop_front()
                .ok_or_else(|| CliError::usage("No session was specified."))?;
            if !args.is_empty() {
                return Err(CliError::usage("Invalid number of arguments."));
            }
            Ok(Action::Push { session })
        }
        'k' => {
            parse_kill(std::mem::take(args)).map(|(session, force)| Action::Kill { session, force })
        }
        _ => Err(CliError::usage(format!("Invalid mode '-{mode}'"))),
    }
}

fn parse_global_options(
    args: &mut VecDeque<String>,
    options: &mut GlobalOptions,
    allow_unknown_break: bool,
) -> Result<(), CliError> {
    loop {
        let Some(arg) = args.front().cloned() else {
            return Ok(());
        };

        if arg == "--" {
            args.pop_front();
            return Ok(());
        }
        if !arg.starts_with('-') || arg == "-" || arg.starts_with("--") {
            return Ok(());
        }

        let token = arg.clone();
        let mut chars = token[1..].chars();
        let mut consumed_token = true;
        let mut consumed_value_arg = false;

        while let Some(ch) = chars.next() {
            match ch {
                'q' => options.quiet = true,
                'E' => options.detach_char = None,
                'z' | 't' => {}
                'e' => {
                    let (value, consumed_next) =
                        consume_option_value(chars.collect::<String>(), args)?;
                    consumed_value_arg = consumed_next;
                    options.detach_char = Some(parse_detach_char(&value)?);
                    break;
                }
                'C' => {
                    let (value, consumed_next) =
                        consume_option_value(chars.collect::<String>(), args)?;
                    consumed_value_arg = consumed_next;
                    options.log_cap = LogCap::parse(&value)
                        .map_err(|_| CliError::usage("Invalid log cap value."))?;
                    break;
                }
                'r' | 'R' => {
                    let (_, consumed_next) = consume_option_value(chars.collect::<String>(), args)?;
                    consumed_value_arg = consumed_next;
                    break;
                }
                _ if allow_unknown_break => {
                    consumed_token = false;
                    break;
                }
                _ => return Err(CliError::usage(format!("Invalid option '-{ch}'"))),
            }
        }

        if consumed_token {
            args.pop_front();
            if consumed_value_arg {
                args.pop_front();
            }
        } else {
            return Ok(());
        }
    }
}

fn consume_option_value(
    remainder: String,
    args: &VecDeque<String>,
) -> Result<(String, bool), CliError> {
    if !remainder.is_empty() {
        return Ok((remainder, false));
    }
    args.get(1)
        .cloned()
        .map(|value| (value, true))
        .ok_or_else(|| CliError::usage("Missing option value."))
}

fn parse_detach_char(value: &str) -> Result<u8, CliError> {
    if value.len() == 1 {
        return Ok(value.as_bytes()[0]);
    }
    if let Some(rest) = value.strip_prefix('^') {
        if rest.len() == 1 {
            let byte = rest.as_bytes()[0].to_ascii_uppercase();
            return Ok(byte & 0x1f);
        }
    }
    Err(CliError::usage(
        "Detach character must be a single byte or ^X form.",
    ))
}

fn legacy_mode(token: &str) -> Option<char> {
    if token.starts_with('-') && !token.starts_with("--") && token.len() == 2 {
        token.chars().nth(1)
    } else {
        None
    }
}

pub(super) fn default_child_command() -> Vec<String> {
    vec![
        env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()),
        "-i".to_string(),
    ]
}

fn child_command_from_args(args: VecDeque<String>) -> Vec<String> {
    if args.is_empty() {
        return default_child_command();
    }
    args.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::parse_cli;
    use crate::cli::Action;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn parses_start_alias_and_quiet_option_before_session() {
        let action = parse_cli(&argv(&["s", "-q", "demo", "sleep", "1"])).expect("parse cli");
        let Action::Start(spec) = action else {
            panic!("expected start action");
        };
        assert!(spec.options.quiet);
        assert_eq!(spec.session, "demo");
        assert_eq!(spec.child_command, ["sleep", "1"]);
    }

    #[test]
    fn parses_legacy_attach_or_create_mode() {
        let action = parse_cli(&argv(&["-A", "demo", "sleep", "1"])).expect("parse cli");
        let Action::Open(spec) = action else {
            panic!("expected open action");
        };
        assert_eq!(spec.session, "demo");
        assert_eq!(spec.child_command, ["sleep", "1"]);
    }

    #[test]
    fn parses_kill_force_after_session_name() {
        let action = parse_cli(&argv(&["kill", "demo", "-f"])).expect("parse cli");
        let Action::Kill { session, force } = action else {
            panic!("expected kill action");
        };
        assert_eq!(session, "demo");
        assert!(force);
    }

    #[test]
    fn parses_default_open_mode() {
        let action = parse_cli(&argv(&["demo", "sleep", "1"])).expect("parse cli");
        let Action::Open(spec) = action else {
            panic!("expected open action");
        };
        assert_eq!(spec.session, "demo");
        assert_eq!(spec.child_command, ["sleep", "1"]);
    }
}
