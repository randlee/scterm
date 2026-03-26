//! Optional ATM bridge integration for the app layer.

use crate::{AppLogger, MasterSession};
use anyhow::Result;
use scterm_core::{SessionName, SessionPath};
use std::path::Path;

#[cfg(feature = "atm")]
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[cfg(feature = "atm")]
use scterm_atm::{AtmConfig, AtmError, AtmEvent, AtmWatcher};

/// One running ATM bridge instance.
#[cfg(feature = "atm")]
pub struct AtmBridge {
    receiver: Receiver<AtmEvent>,
}

#[cfg(feature = "atm")]
pub fn start_atm_bridge(
    path: &SessionPath,
    app_log_root: &Path,
    enabled: bool,
) -> Result<Option<AtmBridge>> {
    if !enabled {
        return Ok(None);
    }

    let mailbox = SessionName::new(mailbox_name(path))?;
    let mut config = AtmConfig::new(
        mailbox.clone(),
        app_log_root.join(format!("{}.atm-dedup", mailbox.as_str())),
    )
    .with_self_identity(mailbox);
    if let Some(username) = nix::unistd::User::from_uid(nix::unistd::geteuid())
        .ok()
        .flatten()
        .map(|user| user.name)
    {
        config = config.with_username(username);
    }

    let (sender, receiver) = mpsc::channel();
    let log_root = app_log_root.to_path_buf();
    std::thread::spawn(move || {
        let logger = AppLogger::new(log_root).ok();
        let mut watcher = match AtmWatcher::new(config) {
            Ok(watcher) => watcher,
            Err(error) => {
                log_bridge_error(logger.as_ref(), &error);
                return;
            }
        };

        loop {
            match watcher.poll_once() {
                Ok(events) => {
                    for event in events {
                        if sender.send(event).is_err() {
                            return;
                        }
                    }
                }
                Err(AtmError::ParseFailure(_)) => {
                    log_bridge_error(
                        logger.as_ref(),
                        &AtmError::ParseFailure("dropping malformed ATM payload".to_string()),
                    );
                }
                Err(error) => {
                    log_bridge_error(logger.as_ref(), &error);
                    return;
                }
            }
        }
    });

    Ok(Some(AtmBridge { receiver }))
}

#[cfg(feature = "atm")]
pub fn drain_atm_bridge<O>(master: &mut MasterSession<O>, bridge: &AtmBridge)
where
    O: crate::OutputObserver,
{
    loop {
        match bridge.receiver.try_recv() {
            Ok(event) => {
                let sender = event.sender().to_string();
                let message_id = event.message_id().to_string();
                master.enqueue_inbound_message(event.injection_bytes());
                let _ = master.log_event(
                    "atm",
                    "inject",
                    &format!("injected ATM message {message_id} from {sender}"),
                );
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return,
        }
    }
}

#[cfg(feature = "atm")]
fn log_bridge_error(logger: Option<&AppLogger>, error: &AtmError) {
    if let Some(logger) = logger {
        let _ = logger.emit("atm", "watch", &error.to_string());
    }
}

#[cfg(feature = "atm")]
fn mailbox_name(path: &SessionPath) -> String {
    path.file_name()
        .map_or_else(|| "scterm".to_string(), ToString::to_string)
}

#[cfg(not(feature = "atm"))]
pub struct AtmBridge;

#[cfg(not(feature = "atm"))]
pub fn start_atm_bridge(
    _path: &SessionPath,
    _app_log_root: &Path,
    _enabled: bool,
) -> Result<Option<AtmBridge>> {
    Ok(None)
}

#[cfg(not(feature = "atm"))]
pub fn drain_atm_bridge<O>(_master: &mut MasterSession<O>, _bridge: &AtmBridge)
where
    O: crate::OutputObserver,
{
}
