//! CLI and PTY integration tests derived from the `atch` compatibility suite.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use nix::poll::{poll, PollFd, PollFlags};
use nix::pty::{openpty, Winsize};
use nix::sys::signal::{kill, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use tempfile::TempDir;

const DETACH_CHAR: u8 = 0x1c;
const SUSPEND_CHAR: u8 = 0x1a;
const LINE_ECHO_SCRIPT: &str = "while IFS= read -r line; do printf '%s\\n' \"$line\"; done";

struct TestEnv {
    tempdir: TempDir,
    home: PathBuf,
    workdir: PathBuf,
}

impl TestEnv {
    fn new() -> Result<Self> {
        let tempdir = TempDir::new()?;
        let home = tempdir.path().join("home");
        let workdir = tempdir.path().join("work");
        fs::create_dir_all(&home)?;
        fs::create_dir_all(&workdir)?;
        Ok(Self {
            tempdir,
            home,
            workdir,
        })
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_scterm"));
        command
            .current_dir(&self.workdir)
            .env("HOME", &self.home)
            .env("SHELL", "/bin/sh")
            .env("TERM", "xterm-256color")
            .env_remove("SCTERM_SESSION");
        command
    }

    fn run(&self, args: &[&str]) -> Result<Output> {
        self.command()
            .args(args)
            .output()
            .with_context(|| format!("run scterm {args:?}"))
    }

    fn run_with_input(&self, args: &[&str], input: &[u8]) -> Result<Output> {
        let mut child = self
            .command()
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("spawn scterm {args:?}"))?;
        child
            .stdin
            .as_mut()
            .context("piped stdin")?
            .write_all(input)?;
        child.wait_with_output().context("wait for scterm output")
    }

    fn spawn_background(&self, args: &[&str]) -> Result<Child> {
        self.command()
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("spawn background scterm {args:?}"))
    }

    fn spawn_pty(&self, args: &[&str]) -> Result<PtyChild> {
        self.spawn_pty_with_size(args, 24, 80)
    }

    fn spawn_pty_with_size(&self, args: &[&str], rows: u16, cols: u16) -> Result<PtyChild> {
        let pty = openpty(
            Some(&Winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            }),
            None,
        )
        .context("open PTY")?;
        let master = File::from(pty.master);
        let slave = File::from(pty.slave);

        let mut command = self.command();
        command.args(args);
        command
            .stdin(Stdio::from(slave.try_clone()?))
            .stdout(Stdio::from(slave.try_clone()?))
            .stderr(Stdio::from(slave));

        let child = command
            .spawn()
            .with_context(|| format!("spawn PTY scterm {args:?}"))?;
        Ok(PtyChild {
            child,
            master,
            captured: Vec::new(),
        })
    }

    fn session_dir(&self) -> PathBuf {
        self.home.join(".cache").join("scterm")
    }

    fn session_socket(&self, name: &str) -> PathBuf {
        self.session_dir().join(name)
    }

    fn session_log(&self, name: &str) -> PathBuf {
        self.session_dir().join(format!("{name}.log"))
    }

    fn temp_path(&self, name: &str) -> PathBuf {
        self.tempdir.path().join(name)
    }

    fn wait_for_socket(&self, name: &str) -> Result<()> {
        wait_for(
            || self.session_socket(name).exists(),
            Duration::from_secs(3),
            &format!("socket for {name}"),
        )
    }

    fn wait_for_socket_removed(&self, name: &str) -> Result<()> {
        wait_for(
            || !self.session_socket(name).exists(),
            Duration::from_secs(8),
            &format!("socket removal for {name}"),
        )
    }

    fn wait_for_file_contains(path: &Path, needle: &str) -> Result<String> {
        let mut last = String::new();
        wait_for(
            || {
                last = fs::read_to_string(path).unwrap_or_default();
                last.contains(needle)
            },
            Duration::from_secs(3),
            &format!("file {} to contain {needle}", path.display()),
        )?;
        Ok(last)
    }

    fn cleanup_session(&self, name: &str) {
        let _ = self.run(&["kill", "-f", name]);
        let _ = self.wait_for_socket_removed(name);
    }
}

struct PtyChild {
    child: Child,
    master: File,
    captured: Vec<u8>,
}

impl PtyChild {
    fn pid(&self) -> Pid {
        Pid::from_raw(i32::try_from(self.child.id()).expect("pid fits in i32"))
    }

    fn send(&mut self, bytes: &[u8]) -> Result<()> {
        self.master.write_all(bytes).context("write PTY input")
    }

    fn read_until(&mut self, needle: &str, timeout: Duration) -> Result<String> {
        let needle = needle.as_bytes();
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            self.read_once(Duration::from_millis(100))?;
            if self
                .captured
                .windows(needle.len())
                .any(|window| window == needle)
            {
                return Ok(String::from_utf8_lossy(&self.captured).into_owned());
            }
            if let Some(status) = self.child.try_wait()? {
                return Err(anyhow!(
                    "child exited before output {needle:?}: status {status}"
                ));
            }
        }

        Err(anyhow!(
            "timed out waiting for {:?}; output was {}",
            needle,
            String::from_utf8_lossy(&self.captured)
        ))
    }

    fn read_for(&mut self, timeout: Duration) -> Result<String> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            self.read_once(Duration::from_millis(50))?;
        }
        Ok(String::from_utf8_lossy(&self.captured).into_owned())
    }

    fn wait_with_output(
        &mut self,
        timeout: Duration,
    ) -> Result<(std::process::ExitStatus, String)> {
        let deadline = Instant::now() + timeout;
        loop {
            self.read_once(Duration::from_millis(100))?;
            if let Some(status) = self.child.try_wait()? {
                self.read_once(Duration::from_millis(50))?;
                return Ok((status, String::from_utf8_lossy(&self.captured).into_owned()));
            }
            if Instant::now() >= deadline {
                return Err(anyhow!(
                    "timed out waiting for child exit; output was {}",
                    String::from_utf8_lossy(&self.captured)
                ));
            }
        }
    }

    fn read_once(&mut self, timeout: Duration) -> Result<()> {
        let mut poll_fds = [PollFd::new(self.master.as_fd(), PollFlags::POLLIN)];
        let timeout_ms: u16 = timeout.as_millis().try_into().unwrap_or(u16::MAX);
        let ready = poll(&mut poll_fds, timeout_ms).context("poll PTY master")?;
        if ready == 0 {
            return Ok(());
        }

        let mut buffer = [0_u8; 4096];
        let read = match self.master.read(&mut buffer) {
            Ok(read) => read,
            Err(error) if error.raw_os_error() == Some(5) => 0,
            Err(error) => return Err(error).context("read PTY master"),
        };
        if read > 0 {
            self.captured.extend_from_slice(&buffer[..read]);
        }
        Ok(())
    }
}

fn probe_pty() -> Result<()> {
    let _pty = openpty(
        Some(&Winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }),
        None,
    )
    .context("probe PTY availability")?;
    Ok(())
}

fn skip_if_pty_unavailable(test_name: &str) -> Result<bool> {
    match probe_pty() {
        Ok(()) => Ok(false),
        Err(error) => {
            let Some(io_error) = error.downcast_ref::<io::Error>() else {
                return Err(error);
            };
            eprintln!("skipping {test_name}: PTY unavailable in this environment: {io_error}");
            Ok(true)
        }
    }
}

fn wait_for(
    mut predicate: impl FnMut() -> bool,
    timeout: Duration,
    description: &str,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if predicate() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(anyhow!("timed out waiting for {description}"))
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn output_text(output: &Output) -> String {
    format!("{}{}", stdout(output), stderr(output))
}

fn wait_for_attached(env: &TestEnv, session: &str) -> Result<()> {
    wait_for(
        || {
            env.run(&["list"])
                .map(|output| {
                    let text = output_text(&output);
                    text.contains(session) && text.contains("[attached]")
                })
                .unwrap_or(false)
        },
        Duration::from_secs(3),
        &format!("session {session} to become attached"),
    )
}

fn terminate_session(env: &TestEnv, session: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut last_output = String::new();
    while Instant::now() < deadline {
        let output = env.run(&["kill", session])?;
        last_output = output_text(&output);
        if output.status.success() {
            return Ok(());
        }
        if last_output.contains("is not running") {
            let socket = env.session_socket(session);
            let _ = fs::remove_file(&socket);
            return Ok(());
        }
        if !env.session_socket(session).exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err(anyhow!(
        "timed out terminating session {session}; last output: {last_output}"
    ))
}

#[test]
fn non_tty_attach_new_and_open_fail_clearly() -> Result<()> {
    let env = TestEnv::new()?;

    for args in [
        vec!["attach", "missing"],
        vec!["new", "missing"],
        vec!["missing-open"],
    ] {
        let output = env.run(&args)?;
        assert!(!output.status.success());
        assert_eq!(output.status.code(), Some(6));
        assert!(output_text(&output).contains("requires a terminal"));
    }

    Ok(())
}

#[test]
fn default_open_creates_then_attaches_existing_session() -> Result<()> {
    if skip_if_pty_unavailable("default_open_creates_then_attaches_existing_session")? {
        return Ok(());
    }
    let env = TestEnv::new()?;

    let mut first = env.spawn_pty(&[
        "demo",
        "/bin/sh",
        "-c",
        "printf 'open-ready\\n'; while IFS= read -r line; do printf '%s\\n' \"$line\"; done",
    ])?;
    env.wait_for_socket("demo")?;
    first.read_until("open-ready", Duration::from_secs(5))?;
    wait_for_attached(&env, "demo")?;
    first.send(&[DETACH_CHAR])?;
    assert!(first.wait_with_output(Duration::from_secs(3))?.0.success());

    let mut second = env.spawn_pty(&["demo"])?;
    second.read_until("open-ready", Duration::from_secs(5))?;
    wait_for_attached(&env, "demo")?;
    second.send(b"second-attach\n")?;
    second.read_until("second-attach", Duration::from_secs(5))?;
    second.send(&[DETACH_CHAR])?;
    assert!(second.wait_with_output(Duration::from_secs(3))?.0.success());

    terminate_session(&env, "demo")?;
    Ok(())
}

#[test]
fn strict_attach_failure_with_tty_reports_missing_session() -> Result<()> {
    if skip_if_pty_unavailable("strict_attach_failure_with_tty_reports_missing_session")? {
        return Ok(());
    }
    let env = TestEnv::new()?;

    let mut attach = env.spawn_pty(&["attach", "missing-session"])?;
    let (status, output) = attach.wait_with_output(Duration::from_secs(3))?;

    assert!(!status.success());
    assert_eq!(status.code(), Some(3));
    assert!(output.contains("does not exist"));
    Ok(())
}

#[test]
fn start_run_and_new_each_create_a_session() -> Result<()> {
    let env = TestEnv::new()?;

    let start = env.run(&["start", "s-start", "sleep", "999"])?;
    assert!(start.status.success(), "{}", output_text(&start));
    env.wait_for_socket("s-start")?;

    let mut run_child = env.spawn_background(&["run", "s-run", "sleep", "999"])?;
    env.wait_for_socket("s-run")?;

    let mut new_child =
        env.spawn_pty(&["new", "s-new", "/bin/sh", "-c", "printf 'new-ok\\n'; cat"])?;
    env.wait_for_socket("s-new")?;
    new_child.read_until("new-ok", Duration::from_secs(5))?;
    wait_for_attached(&env, "s-new")?;
    new_child.send(&[DETACH_CHAR])?;
    assert!(new_child
        .wait_with_output(Duration::from_secs(3))?
        .0
        .success());

    for session in ["s-start", "s-run", "s-new"] {
        assert!(
            env.session_socket(session).exists(),
            "missing socket for {session}"
        );
    }

    for session in ["s-start", "s-run", "s-new"] {
        let output = env.run(&["kill", session])?;
        assert!(
            output.status.success(),
            "kill {session}: {}",
            output_text(&output)
        );
    }
    let _ = run_child.wait();
    Ok(())
}

#[test]
fn push_writes_to_the_session_log() -> Result<()> {
    let env = TestEnv::new()?;

    let start = env.run(&["start", "push-log", "/bin/sh", "-c", LINE_ECHO_SCRIPT])?;
    assert!(start.status.success(), "{}", output_text(&start));
    env.wait_for_socket("push-log")?;

    let output = env.run_with_input(&["push", "push-log"], b"abcdefghijklmnopqrstuvwxyz\n")?;
    assert!(output.status.success(), "{}", output_text(&output));
    wait_for(
        || env.session_log("push-log").exists(),
        Duration::from_secs(3),
        "push-log file",
    )?;
    wait_for(
        || fs::read(env.session_log("push-log")).is_ok_and(|bytes| !bytes.is_empty()),
        Duration::from_secs(3),
        "push-log data",
    )?;

    let log_bytes = fs::read(env.session_log("push-log"))?;
    assert!(log_bytes
        .windows(b"xyz".len())
        .any(|window| window == b"xyz"));

    env.cleanup_session("push-log");
    Ok(())
}

#[test]
fn disabled_log_cap_skips_log_creation() -> Result<()> {
    let env = TestEnv::new()?;

    let start = env.run(&["start", "-C", "0", "nolog", "/bin/sh", "-c", "sleep 30"])?;
    assert!(start.status.success(), "{}", output_text(&start));
    env.wait_for_socket("nolog")?;
    thread::sleep(Duration::from_millis(200));
    assert!(!env.session_log("nolog").exists());

    env.cleanup_session("nolog");
    Ok(())
}

#[test]
fn clear_clears_live_log_and_ring_history() -> Result<()> {
    if skip_if_pty_unavailable("clear_clears_live_log_and_ring_history")? {
        return Ok(());
    }
    let env = TestEnv::new()?;

    let start = env.run(&["start", "clear-live", "/bin/sh", "-c", LINE_ECHO_SCRIPT])?;
    assert!(start.status.success(), "{}", output_text(&start));
    env.wait_for_socket("clear-live")?;

    let push = env.run_with_input(&["push", "clear-live"], b"before-clear\n")?;
    assert!(push.status.success(), "{}", output_text(&push));
    wait_for(
        || {
            fs::read_to_string(env.session_log("clear-live"))
                .is_ok_and(|text| text.contains("before-clear"))
        },
        Duration::from_secs(3),
        "clear-live log to contain marker",
    )?;

    let cleared = env.run(&["clear", "clear-live"])?;
    assert!(cleared.status.success(), "{}", output_text(&cleared));

    let mut attach = env.spawn_pty(&["attach", "clear-live"])?;
    let output = attach.read_for(Duration::from_millis(400))?;
    assert!(!output.contains("before-clear"), "{output}");
    attach.send(&[DETACH_CHAR])?;
    assert!(attach.wait_with_output(Duration::from_secs(3))?.0.success());

    env.cleanup_session("clear-live");
    Ok(())
}

#[test]
fn clear_clears_offline_log_history() -> Result<()> {
    let env = TestEnv::new()?;

    let start = env.run(&["start", "clear-offline", "/bin/sh", "-c", LINE_ECHO_SCRIPT])?;
    assert!(start.status.success(), "{}", output_text(&start));
    env.wait_for_socket("clear-offline")?;

    let push = env.run_with_input(&["push", "clear-offline"], b"before-offline-clear\n")?;
    assert!(push.status.success(), "{}", output_text(&push));
    wait_for(
        || {
            fs::read_to_string(env.session_log("clear-offline"))
                .is_ok_and(|text| text.contains("before-offline-clear"))
        },
        Duration::from_secs(3),
        "clear-offline log to contain marker",
    )?;

    let kill_output = env.run(&["kill", "clear-offline"])?;
    assert!(
        kill_output.status.success(),
        "{}",
        output_text(&kill_output)
    );
    env.wait_for_socket_removed("clear-offline")?;

    let cleared = env.run(&["clear", "clear-offline"])?;
    assert!(cleared.status.success(), "{}", output_text(&cleared));

    let log_metadata = fs::metadata(env.session_log("clear-offline"))?;
    assert_eq!(log_metadata.len(), 0);
    Ok(())
}

#[test]
fn list_marks_attached_and_unattached_sessions() -> Result<()> {
    if skip_if_pty_unavailable("list_marks_attached_and_unattached_sessions")? {
        return Ok(());
    }
    let env = TestEnv::new()?;

    let start = env.run(&["start", "ls-attach", "/bin/sh", "-c", LINE_ECHO_SCRIPT])?;
    assert!(start.status.success(), "{}", output_text(&start));
    env.wait_for_socket("ls-attach")?;

    let list = env.run(&["list"])?;
    let list_text = output_text(&list);
    assert!(list_text.contains("ls-attach"));
    assert!(!list_text.contains("[attached]"));

    let mut attach = env.spawn_pty(&["attach", "ls-attach"])?;
    wait_for_attached(&env, "ls-attach")?;

    attach.send(&[DETACH_CHAR])?;
    assert!(attach.wait_with_output(Duration::from_secs(3))?.0.success());

    let list = env.run(&["list"])?;
    let list_text = output_text(&list);
    assert!(list_text.contains("ls-attach"));
    assert!(!list_text.contains("[attached]"));

    env.cleanup_session("ls-attach");
    Ok(())
}

#[test]
fn stale_session_is_reported_and_can_be_recreated() -> Result<()> {
    if skip_if_pty_unavailable("stale_session_is_reported_and_can_be_recreated")? {
        return Ok(());
    }
    let env = TestEnv::new()?;

    let mut run_child = env.spawn_background(&["run", "stale-case", "sleep", "99999"])?;
    env.wait_for_socket("stale-case")?;
    kill(
        Pid::from_raw(i32::try_from(run_child.id()).expect("pid fits")),
        Signal::SIGKILL,
    )?;
    let _ = run_child.wait();
    thread::sleep(Duration::from_millis(150));

    let list = env.run(&["list"])?;
    assert!(
        output_text(&list).contains("[stale]"),
        "{}",
        output_text(&list)
    );

    let mut attach = env.spawn_pty(&["attach", "stale-case"])?;
    let (status, output) = attach.wait_with_output(Duration::from_secs(3))?;
    assert!(!status.success());
    assert_eq!(status.code(), Some(4));
    assert!(output.contains("not running"));

    let restart = env.run(&["start", "stale-case", "sleep", "999"])?;
    assert!(restart.status.success(), "{}", output_text(&restart));
    env.wait_for_socket("stale-case")?;
    env.cleanup_session("stale-case");
    Ok(())
}

#[test]
fn self_attach_prevention_uses_the_session_ancestry_env_var() -> Result<()> {
    let env = TestEnv::new()?;
    let session_path = env.session_socket("loop");
    fs::create_dir_all(session_path.parent().context("session parent")?)?;

    let output = env
        .command()
        .env("SCTERM_SESSION", &session_path)
        .args(["start", "loop", "sleep", "1"])
        .output()
        .context("run self-attach prevention test")?;

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(5));
    assert!(output_text(&output).contains("attach to itself"));
    Ok(())
}

#[test]
fn current_subcommand_prints_the_innermost_session_name() -> Result<()> {
    let env = TestEnv::new()?;
    let session_path = env.session_socket("current-demo");
    fs::create_dir_all(session_path.parent().context("session parent")?)?;

    let output = env
        .command()
        .env("SCTERM_SESSION", &session_path)
        .arg("current")
        .output()
        .context("run current subcommand")?;

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output).trim(), "current-demo");
    Ok(())
}

#[test]
fn legacy_modes_execute_the_compat_surface() -> Result<()> {
    if skip_if_pty_unavailable("legacy_modes_execute_the_compat_surface")? {
        return Ok(());
    }
    let env = TestEnv::new()?;

    let list = env.run(&["-l"])?;
    assert_eq!(list.status.code(), Some(0));

    let current_path = env.session_socket("legacy-current");
    fs::create_dir_all(current_path.parent().context("session parent")?)?;
    let current = env
        .command()
        .env("SCTERM_SESSION", &current_path)
        .arg("-i")
        .output()
        .context("run legacy current mode")?;
    assert_eq!(current.status.code(), Some(0));
    assert_eq!(stdout(&current).trim(), "legacy-current");

    let start = env.run(&["-n", "legacy-start", "sleep", "999"])?;
    assert_eq!(start.status.code(), Some(0));
    env.wait_for_socket("legacy-start")?;

    let mut attach = env.spawn_pty(&["-a", "legacy-start"])?;
    wait_for_attached(&env, "legacy-start")?;
    attach.send(&[DETACH_CHAR])?;
    assert!(attach.wait_with_output(Duration::from_secs(3))?.0.success());

    let push = env.run_with_input(&["-p", "legacy-start"], b"legacy-push\n")?;
    assert_eq!(push.status.code(), Some(0));
    wait_for(
        || {
            fs::read_to_string(env.session_log("legacy-start"))
                .is_ok_and(|text| text.contains("legacy-push"))
        },
        Duration::from_secs(3),
        "legacy push to reach the log",
    )?;

    let mut create = env.spawn_pty(&[
        "-c",
        "legacy-new",
        "/bin/sh",
        "-c",
        "printf 'legacy-new-ok\\n'; cat",
    ])?;
    env.wait_for_socket("legacy-new")?;
    create.read_until("legacy-new-ok", Duration::from_secs(5))?;
    wait_for_attached(&env, "legacy-new")?;
    create.send(&[DETACH_CHAR])?;
    assert!(create.wait_with_output(Duration::from_secs(3))?.0.success());

    let mut open = env.spawn_pty(&["-A", "legacy-new"])?;
    wait_for_attached(&env, "legacy-new")?;
    open.read_until("legacy-new-ok", Duration::from_secs(5))?;
    open.send(&[DETACH_CHAR])?;
    assert!(open.wait_with_output(Duration::from_secs(3))?.0.success());

    let mut run = env.spawn_background(&["-N", "legacy-run", "sleep", "999"])?;
    env.wait_for_socket("legacy-run")?;

    let kill_start = env.run(&["-k", "legacy-start"])?;
    assert_eq!(kill_start.status.code(), Some(0));
    env.wait_for_socket_removed("legacy-start")?;

    let kill_new = env.run(&["-k", "legacy-new"])?;
    assert_eq!(kill_new.status.code(), Some(0));
    env.wait_for_socket_removed("legacy-new")?;

    let kill_run = env.run(&["-k", "legacy-run"])?;
    assert_eq!(kill_run.status.code(), Some(0));
    env.wait_for_socket_removed("legacy-run")?;
    let _ = run.wait();

    Ok(())
}

#[test]
fn sigwinch_is_forwarded_from_the_attach_client_to_the_child_process() -> Result<()> {
    if skip_if_pty_unavailable("sigwinch_is_forwarded_from_the_attach_client_to_the_child_process")?
    {
        return Ok(());
    }
    let env = TestEnv::new()?;
    let size_path = env.temp_path("sigwinch-size.txt");
    let ready_path = env.temp_path("sigwinch-ready.txt");
    let command = format!(
        "trap 'stty size > {}' WINCH; printf ready > {}; while :; do sleep 1; done",
        size_path.display(),
        ready_path.display()
    );

    let start = env.run(&["start", "sigwinch", "/bin/sh", "-c", &command])?;
    assert!(start.status.success(), "{}", output_text(&start));
    env.wait_for_socket("sigwinch")?;
    TestEnv::wait_for_file_contains(&ready_path, "ready")?;

    let mut attach = env.spawn_pty(&["attach", "sigwinch"])?;
    wait_for_attached(&env, "sigwinch")?;
    kill(attach.pid(), Signal::SIGWINCH)?;

    wait_for(
        || fs::read_to_string(&size_path).is_ok_and(|text| text.split_whitespace().count() == 2),
        Duration::from_secs(3),
        "SIGWINCH forwarding to child process",
    )?;

    attach.send(&[DETACH_CHAR])?;
    assert!(attach.wait_with_output(Duration::from_secs(3))?.0.success());
    env.cleanup_session("sigwinch");
    Ok(())
}

#[test]
fn kill_session_grace_reports_stopped() -> Result<()> {
    let env = TestEnv::new()?;

    let start = env.run(&["start", "kill-grace", "sleep", "999"])?;
    assert!(start.status.success(), "{}", output_text(&start));
    env.wait_for_socket("kill-grace")?;

    let output = env.run(&["kill", "kill-grace"])?;
    assert_eq!(output.status.code(), Some(0));
    assert!(output_text(&output).contains("stopped"));
    env.wait_for_socket_removed("kill-grace")?;
    Ok(())
}

#[test]
fn kill_session_force_reports_killed() -> Result<()> {
    let env = TestEnv::new()?;

    let start = env.run(&["start", "kill-force", "sleep", "999"])?;
    assert!(start.status.success(), "{}", output_text(&start));
    env.wait_for_socket("kill-force")?;

    let output = env.run(&["kill", "-f", "kill-force"])?;
    assert_eq!(output.status.code(), Some(0));
    assert!(output_text(&output).contains("killed"));
    env.wait_for_socket_removed("kill-force")?;
    Ok(())
}

#[test]
fn child_process_receives_scterm_session_env_var() -> Result<()> {
    let env = TestEnv::new()?;
    let output_path = env.temp_path("session-env.txt");
    let command = format!(
        "printf '%s' \"$SCTERM_SESSION\" > {}; sleep 30",
        output_path.display()
    );

    let start = env.run(&["start", "env-test", "/bin/sh", "-c", &command])?;
    assert!(start.status.success(), "{}", output_text(&start));
    env.wait_for_socket("env-test")?;

    let contents = TestEnv::wait_for_file_contains(&output_path, "env-test")?;
    assert!(contents.contains("env-test"));

    env.cleanup_session("env-test");
    Ok(())
}

#[test]
fn detach_and_suspend_leave_the_session_running() -> Result<()> {
    if skip_if_pty_unavailable("detach_and_suspend_leave_the_session_running")? {
        return Ok(());
    }
    let env = TestEnv::new()?;

    let start = env.run(&["start", "detachable", "/bin/sh", "-c", LINE_ECHO_SCRIPT])?;
    assert!(start.status.success(), "{}", output_text(&start));
    env.wait_for_socket("detachable")?;

    let mut detach_client = env.spawn_pty(&["attach", "detachable"])?;
    wait_for_attached(&env, "detachable")?;
    detach_client.send(&[DETACH_CHAR])?;
    assert!(detach_client
        .wait_with_output(Duration::from_secs(3))?
        .0
        .success());
    assert!(env.run(&["list"])?.status.success());
    assert!(output_text(&env.run(&["list"])?).contains("detachable"));

    let mut suspend_client = env.spawn_pty(&["attach", "detachable"])?;
    wait_for_attached(&env, "detachable")?;
    suspend_client.send(&[SUSPEND_CHAR])?;
    wait_for(
        || {
            matches!(
                waitpid(
                    suspend_client.pid(),
                    Some(WaitPidFlag::WUNTRACED | WaitPidFlag::WNOHANG)
                ),
                Ok(WaitStatus::Stopped(_, Signal::SIGSTOP))
            )
        },
        Duration::from_secs(3),
        "client suspend stop",
    )?;
    kill(suspend_client.pid(), Signal::SIGCONT)?;
    assert!(suspend_client
        .wait_with_output(Duration::from_secs(3))?
        .0
        .success());

    let list = env.run(&["list"])?;
    assert!(output_text(&list).contains("detachable"));
    env.cleanup_session("detachable");
    Ok(())
}

#[test]
fn multi_client_attach_receives_the_same_output() -> Result<()> {
    if skip_if_pty_unavailable("multi_client_attach_receives_the_same_output")? {
        return Ok(());
    }
    let env = TestEnv::new()?;

    let start = env.run(&["start", "multi", "/bin/sh", "-c", LINE_ECHO_SCRIPT])?;
    assert!(start.status.success(), "{}", output_text(&start));
    env.wait_for_socket("multi")?;

    let mut first = env.spawn_pty(&["attach", "multi"])?;
    let mut second = env.spawn_pty(&["attach", "multi"])?;
    wait_for_attached(&env, "multi")?;

    let pushed = env.run_with_input(&["push", "multi"], b"multi-line\n")?;
    assert!(pushed.status.success(), "{}", output_text(&pushed));

    first.read_until("multi-line", Duration::from_secs(5))?;
    second.read_until("multi-line", Duration::from_secs(5))?;

    first.send(&[DETACH_CHAR])?;
    second.send(&[DETACH_CHAR])?;
    assert!(first.wait_with_output(Duration::from_secs(3))?.0.success());
    assert!(second.wait_with_output(Duration::from_secs(3))?.0.success());

    env.cleanup_session("multi");
    Ok(())
}

#[test]
fn bad_exec_path_fails_startup_readiness() -> Result<()> {
    let env = TestEnv::new()?;
    let start = env.run(&["start", "badexec", "__scterm_no_such_command__"])?;
    assert!(!start.status.success());
    assert!(!env.session_socket("badexec").exists());
    Ok(())
}
