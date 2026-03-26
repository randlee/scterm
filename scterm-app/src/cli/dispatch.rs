use super::args::{default_child_command, parse_cli};
use super::session::{
    app_log_root_for, current_ancestry_chain, current_session_path, current_window_size,
    default_session_dir, domain_error, ensure_not_self_attach, require_tty, resolve_session_path,
    runtime_error, session_label, session_state, unix_error, SessionState,
};
use super::{
    Action, CliError, GlobalOptions, SessionCommand, EXIT_EXISTS, EXIT_GENERAL, EXIT_NOT_FOUND,
    EXIT_STALE, EXIT_START_TIMEOUT, EXIT_SUCCESS, START_TIMEOUT,
};
use anyhow::{Context, Result};
use nix::errno::Errno;
use nix::poll::{poll, PollFd, PollFlags};
use nix::sys::signal::{kill, Signal};
use nix::unistd::getpid;
use scterm_core::{
    session_env_var_name, AncestryChain, KillRequest, LogCap, Packet, PushData, RedrawMethod,
    RingSize, Session, SessionPath,
};
use scterm_unix::{
    PtyCommand, RawModeGuard, SignalEvent, SignalWatcher, SocketTransport, UnixSocketTransport,
};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::fs::FileTypeExt;
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::{
    atm::{drain_atm_bridge, start_atm_bridge},
    attached_state, log_path_for_session, AttachSession, MasterConfig, MasterSession,
    NoopOutputObserver, PersistentLog, SessionLauncher,
};

/// Parses and executes the `scterm` CLI, returning the process exit code.
#[must_use]
pub fn run_cli<I>(argv: I) -> i32
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let argv_vec = argv.into_iter().map(Into::into).collect::<Vec<_>>();
    let program = argv_vec
        .first()
        .cloned()
        .unwrap_or_else(|| "scterm".to_string());

    match parse_cli(&argv_vec[1..]).and_then(|action| execute(&program, action)) {
        Ok(()) => EXIT_SUCCESS,
        Err(error) => {
            if !error.message.is_empty() {
                eprintln!("{program}: {}", error.message);
                if error.show_help {
                    eprintln!("Try '{program} --help' for more information.");
                }
            }
            error.code
        }
    }
}

fn execute(program: &str, action: Action) -> Result<(), CliError> {
    match action {
        Action::Help => {
            print_help(program);
            Ok(())
        }
        Action::Version => {
            println!("scterm - version {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Action::List { options } => list_sessions(program, &options),
        Action::Current => current_session(program),
        Action::Clear { session, quiet } => clear_session(program, session, quiet),
        Action::Push { session } => push_to_session(program, &session),
        Action::Kill { session, force } => kill_session(program, &session, force),
        Action::Start(spec) => start_session(program, &spec, false),
        Action::Run(spec) => start_session(program, &spec, true),
        Action::Attach(spec) => attach_to_session(program, &spec.session, spec.options.detach_char),
        Action::New(spec) => {
            require_tty(program)?;
            let path = resolve_session_path(program, &spec.session)?;
            ensure_not_self_attach(program, &path)?;
            match session_state(&path)? {
                SessionState::Live => {
                    return Err(CliError::new(
                        EXIT_EXISTS,
                        format!("session '{}' is already running", session_label(&path)),
                    ));
                }
                SessionState::Stale => {
                    let _ = fs::remove_file(path.as_path());
                }
                SessionState::Absent => {}
            }
            start_session(program, &spec, false)?;
            if !spec.options.quiet {
                println!("{program}: session '{}' created", session_label(&path));
            }
            attach_to_session(program, &spec.session, spec.options.detach_char)
        }
        Action::Open(spec) => {
            require_tty(program)?;
            let path = resolve_session_path(program, &spec.session)?;
            ensure_not_self_attach(program, &path)?;
            match session_state(&path)? {
                SessionState::Live => {
                    attach_to_session(program, &spec.session, spec.options.detach_char)
                }
                SessionState::Stale | SessionState::Absent => {
                    if matches!(session_state(&path)?, SessionState::Stale) {
                        let _ = fs::remove_file(path.as_path());
                    }
                    start_session(program, &spec, false)?;
                    if !spec.options.quiet {
                        println!("{program}: session '{}' created", session_label(&path));
                    }
                    attach_to_session(program, &spec.session, spec.options.detach_char)
                }
            }
        }
        Action::InternalMaster {
            session_path,
            log_cap_bytes,
            atm_enabled,
            child_command,
        } => internal_master_main(
            program,
            &session_path,
            log_cap_bytes,
            atm_enabled,
            &child_command,
        )
        .map_err(runtime_error),
    }
}

fn print_help(program: &str) {
    println!(
        "Usage:\n  {program} [<session> [command...]]\n  {program} <command> [options] ...\n\nOptions:\n  --atm                Enable ATM inbound message injection for new sessions\n\nCommands:\n  attach <session>\n  new <session> [command...]\n  start <session> [command...]\n  run <session> [command...]\n  push <session>\n  kill [-f|--force] <session>\n  clear [<session>]\n  list\n  current"
    );
}

fn current_session(program: &str) -> Result<(), CliError> {
    let env_name = session_env_var_name(program);
    let Some(value) = env::var_os(&env_name) else {
        return Err(CliError::new(EXIT_GENERAL, "not inside a session"));
    };
    let value = value
        .into_string()
        .map_err(|_| CliError::new(EXIT_GENERAL, "session ancestry is not valid UTF-8"))?;
    let chain = AncestryChain::parse(&value).map_err(domain_error)?;
    if chain.is_empty() {
        return Err(CliError::new(EXIT_GENERAL, "not inside a session"));
    }
    println!("{}", chain.render_human());
    Ok(())
}

fn list_sessions(program: &str, options: &GlobalOptions) -> Result<(), CliError> {
    let dir = default_session_dir(program);
    let mut count = 0_usize;
    let transport = UnixSocketTransport;

    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if !options.quiet {
                println!("(no sessions)");
            }
            return Ok(());
        }
        Err(error) => {
            return Err(CliError::new(
                EXIT_GENERAL,
                format!("{}: {}", dir.display(), error),
            ));
        }
    };

    for entry in entries {
        let entry = entry
            .map_err(|error| CliError::new(EXIT_GENERAL, format!("{}: {error}", dir.display())))?;
        let path = entry.path();
        let metadata = match fs::metadata(&path) {
            Ok(metadata) if metadata.file_type().is_socket() => metadata,
            _ => continue,
        };
        let session_path = SessionPath::new(path.clone()).map_err(domain_error)?;
        let label = session_label(&session_path);

        match transport.connect(&session_path) {
            Ok(_) => {
                let marker = if attached_state(session_path.as_path()).unwrap_or(false) {
                    " [attached]"
                } else {
                    ""
                };
                println!("{label} {session_path}{marker}");
                count += 1;
            }
            Err(error) if error.is_stale_socket() => {
                println!("{label} {session_path} [stale]");
                count += 1;
            }
            Err(_) => {}
        }

        let _ = metadata;
    }

    if count == 0 && !options.quiet {
        println!("(no sessions)");
    }
    Ok(())
}

fn clear_session(program: &str, session: Option<String>, quiet: bool) -> Result<(), CliError> {
    let path = match session {
        Some(session) => resolve_session_path(program, &session)?,
        None => current_session_path(program)?,
    };
    let log_path = log_path_for_session(&path);
    let cleared = if matches!(session_state(&path)?, SessionState::Live) {
        let transport = UnixSocketTransport;
        let mut stream = transport.connect(&path).map_err(unix_error)?;
        stream
            .write_all(&Packet::Clear.encode())
            .and_then(|()| stream.flush())
            .map_err(unix_error)?;
        true
    } else {
        match fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&log_path)
        {
            Ok(_) => true,
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => {
                return Err(CliError::new(
                    EXIT_GENERAL,
                    format!("{}: {error}", log_path.display()),
                ));
            }
        }
    };

    if cleared && !quiet {
        println!("{program}: session '{}' log cleared", session_label(&path));
    }
    Ok(())
}

fn push_to_session(program: &str, session: &str) -> Result<(), CliError> {
    let path = resolve_session_path(program, session)?;
    let transport = UnixSocketTransport;
    let mut stream = transport.connect(&path).map_err(unix_error)?;
    let stdin = io::stdin();
    let mut buffer = [0_u8; 8];

    loop {
        let read = stdin
            .lock()
            .read(&mut buffer)
            .map_err(|error| CliError::new(EXIT_GENERAL, error.to_string()))?;
        if read == 0 {
            return Ok(());
        }
        let packet = Packet::Push(PushData::new(&buffer[..read]).map_err(domain_error)?).encode();
        stream.write_all(&packet).map_err(unix_error)?;
    }
}

fn kill_session(program: &str, session: &str, force: bool) -> Result<(), CliError> {
    let path = resolve_session_path(program, session)?;
    let transport = UnixSocketTransport;
    let signal = if force {
        Signal::SIGKILL as i32
    } else {
        Signal::SIGTERM as i32
    };
    let packet = Packet::Kill(KillRequest::new(
        u8::try_from(signal).expect("signal number fits in u8"),
    ))
    .encode();
    let mut stream = transport.connect(&path).map_err(unix_error)?;
    stream.write_all(&packet).map_err(unix_error)?;
    stream.flush().map_err(unix_error)?;

    let deadline = Instant::now()
        + if force {
            std::time::Duration::from_secs(2)
        } else {
            std::time::Duration::from_secs(7)
        };
    while Instant::now() < deadline {
        if !path.as_path().exists() {
            if force {
                println!("{program}: session '{}' killed", session_label(&path));
            } else {
                println!("{program}: session '{}' stopped", session_label(&path));
            }
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Err(CliError::new(
        EXIT_GENERAL,
        format!("session '{}' did not stop", session_label(&path)),
    ))
}

fn start_session(program: &str, spec: &SessionCommand, foreground: bool) -> Result<(), CliError> {
    let path = resolve_session_path(program, &spec.session)?;
    if let Some(parent) = path.as_path().parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CliError::new(
                EXIT_GENERAL,
                format!("create session directory {}: {error}", parent.display()),
            )
        })?;
    }
    ensure_not_self_attach(program, &path)?;
    match session_state(&path)? {
        SessionState::Live => {
            return Err(CliError::new(
                EXIT_EXISTS,
                format!("session '{}' is already running", session_label(&path)),
            ));
        }
        SessionState::Stale => {
            let _ = fs::remove_file(path.as_path());
        }
        SessionState::Absent => {}
    }

    if foreground {
        internal_master_main(
            program,
            &path.to_string(),
            spec.options.log_cap.bytes(),
            spec.options.atm,
            &spec.child_command,
        )
        .map_err(runtime_error)
    } else {
        spawn_internal_master(&path, spec)?;
        wait_for_startup(&path)?;
        if !spec.options.quiet {
            println!("{program}: session '{}' started", session_label(&path));
        }
        Ok(())
    }
}

fn spawn_internal_master(path: &SessionPath, spec: &SessionCommand) -> Result<(), CliError> {
    let exe = env::current_exe().map_err(|error| {
        CliError::new(EXIT_GENERAL, format!("resolve current executable: {error}"))
    })?;
    let mut command = Command::new(exe);
    command
        .arg("__internal-master")
        .arg(path.to_string())
        .arg(spec.options.log_cap.bytes().to_string())
        .arg(spec.options.atm.to_string())
        .arg("--");
    for arg in &spec.child_command {
        command.arg(arg);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
        .spawn()
        .map_err(|error| CliError::new(EXIT_GENERAL, format!("spawn session master: {error}")))?;
    Ok(())
}

fn wait_for_startup(path: &SessionPath) -> Result<(), CliError> {
    let transport = UnixSocketTransport;
    let deadline = Instant::now() + START_TIMEOUT;
    while Instant::now() < deadline {
        match transport.connect(path) {
            Ok(_) => return Ok(()),
            Err(error) if error.is_absent_session() || error.is_stale_socket() => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(error) => return Err(unix_error(error)),
        }
    }

    Err(CliError::new(
        EXIT_START_TIMEOUT,
        format!(
            "session '{}' did not become ready in time",
            session_label(path)
        ),
    ))
}

fn attach_to_session(
    program: &str,
    session: &str,
    detach_char: Option<u8>,
) -> Result<(), CliError> {
    require_tty(program)?;
    let path = resolve_session_path(program, session)?;
    ensure_not_self_attach(program, &path)?;
    let state = session_state(&path)?;
    if matches!(state, SessionState::Absent) {
        return Err(CliError::new(
            EXIT_NOT_FOUND,
            format!("session '{}' does not exist", session_label(&path)),
        ));
    }
    if matches!(state, SessionState::Stale) {
        return Err(CliError::new(
            EXIT_STALE,
            format!("session '{}' is not running", session_label(&path)),
        ));
    }

    let log = PersistentLog::open(log_path_for_session(&path), LogCap::disabled())
        .map_err(runtime_error)?;
    let attach = AttachSession::new(path);
    let (log_replaying, history) = attach.replay_log(&log).map_err(runtime_error)?;
    io::stdout()
        .write_all(&history)
        .map_err(|error| CliError::new(EXIT_GENERAL, error.to_string()))?;

    let connecting = attach.finish_log_replay(log_replaying);
    let (ring_replaying, mut stream) = attach.connect(connecting, false).map_err(runtime_error)?;
    let stdin = io::stdin();
    let size = current_window_size(&stdin);
    attach
        .request_redraw(&mut stream, size)
        .map_err(runtime_error)?;
    let _live = ring_replaying.go_live();

    let _raw_mode = RawModeGuard::new(&stdin).map_err(unix_error)?;
    let mut signal_watcher = SignalWatcher::new().map_err(unix_error)?;
    let mut stdout = io::stdout().lock();
    let mut inbound = [0_u8; 4096];
    let mut outbound = [0_u8; 1024];

    loop {
        let mut poll_fds = [
            PollFd::new(stream.as_fd(), PollFlags::POLLIN),
            PollFd::new(stdin.as_fd(), PollFlags::POLLIN),
        ];
        match poll(&mut poll_fds, 250_u16) {
            Ok(_) | Err(Errno::EINTR) => {}
            Err(error) => {
                return Err(CliError::new(
                    EXIT_GENERAL,
                    format!("poll attach streams: {error}"),
                ));
            }
        }

        let socket_ready = poll_fds[0]
            .revents()
            .is_some_and(|flags| flags.contains(PollFlags::POLLIN));
        let stdin_ready = poll_fds[1]
            .revents()
            .is_some_and(|flags| flags.contains(PollFlags::POLLIN));
        let _ = poll_fds;

        for event in signal_watcher.pending() {
            if matches!(event, SignalEvent::WindowChange) {
                send_winch(&mut stream, current_window_size(&stdin))?;
            }
        }

        if socket_ready {
            let read = stream.read(&mut inbound).map_err(unix_error)?;
            if read == 0 {
                return Ok(());
            }
            stdout
                .write_all(&inbound[..read])
                .and_then(|()| stdout.flush())
                .map_err(|error| CliError::new(EXIT_GENERAL, error.to_string()))?;
        }

        if stdin_ready {
            let read = stdin
                .lock()
                .read(&mut outbound)
                .map_err(|error| CliError::new(EXIT_GENERAL, error.to_string()))?;
            if read == 0 {
                return Ok(());
            }

            let mut to_send = Vec::with_capacity(read);
            for byte in &outbound[..read] {
                if detach_char.is_some_and(|detach| *byte == detach) {
                    send_detach(&mut stream)?;
                    return Ok(());
                }
                if *byte == 0x1a {
                    send_detach(&mut stream)?;
                    kill(getpid(), Signal::SIGSTOP)
                        .map_err(|error| CliError::new(EXIT_GENERAL, error.to_string()))?;
                    return Ok(());
                }
                to_send.push(*byte);
            }
            send_push_bytes(&mut stream, &to_send)?;
        }
    }
}

fn send_detach(stream: &mut scterm_unix::UnixSocketStream) -> Result<(), CliError> {
    stream
        .write_all(&Packet::Detach.encode())
        .and_then(|()| stream.flush())
        .map_err(unix_error)
}

fn send_winch(
    stream: &mut scterm_unix::UnixSocketStream,
    size: scterm_core::WindowSize,
) -> Result<(), CliError> {
    stream
        .write_all(&Packet::Winch(size).encode())
        .and_then(|()| stream.flush())
        .map_err(unix_error)
}

fn send_push_bytes(
    stream: &mut scterm_unix::UnixSocketStream,
    bytes: &[u8],
) -> Result<(), CliError> {
    for chunk in bytes.chunks(8) {
        let packet = Packet::Push(PushData::new(chunk).map_err(domain_error)?).encode();
        stream.write_all(&packet).map_err(unix_error)?;
    }
    stream.flush().map_err(unix_error)
}

fn internal_master_main(
    program: &str,
    session_path: &str,
    log_cap_bytes: u64,
    atm_enabled: bool,
    child_command: &[String],
) -> Result<()> {
    let path = SessionPath::new(session_path).context("validate internal session path")?;
    let command = build_pty_command(program, &path, child_command)?;
    let config = MasterConfig::new(
        RingSize::new(128 * 1024).expect("static ring size is valid"),
        LogCap::from_bytes(log_cap_bytes),
        app_log_root_for(&path),
    );
    let app_log_root = config.app_log_root().to_path_buf();
    let launcher = SessionLauncher::new(config);
    let started = launcher.start(
        Session::new_resolved(path),
        &command,
        Some(current_window_size(&io::stdin())),
        NoopOutputObserver,
    )?;
    let mut master = started.into_master();
    let atm_bridge = start_atm_bridge(master.path(), &app_log_root, atm_enabled)?;
    drive_master(&mut master, atm_bridge.as_ref())
}

fn drive_master(
    master: &mut MasterSession,
    atm_bridge: Option<&crate::atm::AtmBridge>,
) -> Result<()> {
    let mut next_client_id = 1_u64;
    let mut read_clients = Vec::<ReadClient>::new();
    let mut pty_buffer = [0_u8; 4096];

    loop {
        if let Some(bridge) = atm_bridge {
            drain_atm_bridge(master, bridge);
        }

        if master.child_exited()? {
            master.handle_child_exit()?;
            return Ok(());
        }

        let mut poll_fds = Vec::with_capacity(2 + read_clients.len());
        poll_fds.push(PollFd::new(master.listener_fd(), PollFlags::POLLIN));
        poll_fds.push(PollFd::new(master.pty_fd(), PollFlags::POLLIN));
        for client in &read_clients {
            poll_fds.push(PollFd::new(
                client.stream.as_fd(),
                PollFlags::POLLIN | PollFlags::POLLHUP,
            ));
        }
        poll(&mut poll_fds, 100_u16)?;

        let listener_ready = poll_fds[0]
            .revents()
            .is_some_and(|flags| flags.contains(PollFlags::POLLIN));
        let pty_ready = poll_fds[1]
            .revents()
            .is_some_and(|flags| flags.contains(PollFlags::POLLIN));
        let client_events = read_clients
            .iter()
            .zip(poll_fds.iter().skip(2))
            .map(|(client, poll_fd)| {
                (
                    client.id,
                    poll_fd
                        .revents()
                        .is_some_and(|flags| flags.contains(PollFlags::POLLIN)),
                    poll_fd
                        .revents()
                        .is_some_and(|flags| flags.contains(PollFlags::POLLHUP)),
                )
            })
            .collect::<Vec<_>>();
        drop(poll_fds);

        if listener_ready {
            let stream = master.accept_client()?;
            read_clients.push(ReadClient {
                id: next_client_id,
                stream,
                attached: false,
            });
            next_client_id += 1;
        }

        if pty_ready {
            let read = master.read_pty(&mut pty_buffer)?;
            if read > 0 {
                let _ = master.record_pty_output(&pty_buffer[..read]);
            }
        }

        let mut removals = Vec::new();
        for (client_id, ready, hung_up) in client_events {
            if !ready && !hung_up {
                continue;
            }
            let Some(client) = read_clients
                .iter_mut()
                .find(|client| client.id == client_id)
            else {
                continue;
            };

            loop {
                let mut bytes = [0_u8; scterm_core::PACKET_SIZE];
                if client.stream.read_exact(&mut bytes).is_ok() {
                    handle_client_packet(master, client, Packet::decode(bytes)?)?;
                } else {
                    let _ = master.detach_client_by_id(client_id);
                    removals.push(client_id);
                    break;
                }

                let mut pending = [PollFd::new(client.stream.as_fd(), PollFlags::POLLIN)];
                let more_ready = poll(&mut pending, 0_u8)? > 0
                    && pending[0]
                        .revents()
                        .is_some_and(|flags| flags.contains(PollFlags::POLLIN));
                if !more_ready {
                    break;
                }
            }

            if hung_up && !removals.contains(&client_id) {
                let _ = master.detach_client_by_id(client_id);
                removals.push(client_id);
            }
        }

        if !removals.is_empty() {
            read_clients.retain(|client| !removals.contains(&client.id));
        }

        let _ = master.drain_input_queue()?;
    }
}

#[derive(Debug)]
struct ReadClient {
    id: u64,
    stream: scterm_unix::UnixSocketStream,
    attached: bool,
}

fn handle_client_packet(
    master: &mut MasterSession,
    client: &mut ReadClient,
    packet: Packet,
) -> Result<()> {
    match packet {
        Packet::Attach(request) => {
            let writer = client.stream.try_clone()?;
            let ring = master.attach_client_with_id(client.id, writer, request)?;
            if !ring.is_empty() {
                master.write_to_client(client.id, &ring)?;
            }
            client.attached = true;
        }
        Packet::Detach => {
            if client.attached {
                master.detach_client_by_id(client.id)?;
            }
        }
        Packet::Push(data) => master.enqueue_push_input(data.as_slice().to_vec()),
        Packet::Winch(size) => {
            master.resize_pty(size)?;
            master.signal_child_group(Signal::SIGWINCH)?;
        }
        Packet::Redraw(request) => match request.method() {
            RedrawMethod::None | RedrawMethod::Unspecified => {}
            RedrawMethod::CtrlL => master.enqueue_redraw_input(vec![0x0c]),
            RedrawMethod::Winch => {
                master.resize_pty(request.size())?;
                master.signal_child_group(Signal::SIGWINCH)?;
            }
        },
        Packet::Kill(request) => {
            if let Ok(signal) = Signal::try_from(i32::from(request.signal())) {
                master.signal_child_group(signal)?;
            }
        }
        Packet::Clear => master.clear_history()?,
    }
    Ok(())
}

fn build_pty_command(program: &str, path: &SessionPath, command: &[String]) -> Result<PtyCommand> {
    let command = if command.is_empty() {
        default_child_command()
    } else {
        command.to_vec()
    };
    let mut pty = PtyCommand::new(&command[0])?;
    for arg in &command[1..] {
        pty = pty.arg(arg)?;
    }

    let env_name = session_env_var_name(program);
    let mut chain = current_ancestry_chain(program).unwrap_or_default();
    chain.append(path.clone());
    pty = pty.env(&env_name, &chain.build_env_value())?;
    Ok(pty)
}
