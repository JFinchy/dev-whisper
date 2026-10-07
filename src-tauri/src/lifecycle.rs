use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use std::time::Duration;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// Heartbeats and signal capture exist so an unexplained process death
/// leaves evidence in `app.log`: a heartbeat gap with no shutdown line
/// means the process was killed without warning (or the machine slept),
/// and a logged signal names exactly what asked it to stop.
pub fn spawn_watchers() {
    std::thread::spawn(|| loop {
        std::thread::sleep(HEARTBEAT_INTERVAL);
        crate::applog!("lifecycle: heartbeat");
    });

    match Signals::new([SIGTERM, SIGINT, SIGHUP]) {
        Ok(mut signals) => {
            std::thread::spawn(move || {
                if let Some(sig) = signals.forever().next() {
                    crate::applog!("lifecycle: received signal {sig}, exiting");
                    std::process::exit(128 + sig);
                }
            });
        }
        Err(err) => crate::applog!("lifecycle: could not install signal handlers: {err}"),
    }
}

pub fn log_run_event(event: &tauri::RunEvent) {
    match event {
        tauri::RunEvent::ExitRequested { code, .. } => {
            crate::applog!("lifecycle: exit requested, code={code:?}");
        }
        tauri::RunEvent::Exit => crate::applog!("lifecycle: exiting"),
        _ => {}
    }
}
