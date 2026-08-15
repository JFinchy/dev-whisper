use objc2_app_kit::{NSApplicationActivationPolicy, NSWorkspace};

#[derive(Clone)]
pub struct AppInfo {
    pub bundle_id: String,
    pub name: String,
}

/// Reads the frontmost application via NSWorkspace. No special permission
/// needed (unlike Accessibility), but AppKit requires this run on the main
/// thread — callers should dispatch through `AppHandle::run_on_main_thread`.
pub fn frontmost_app_info() -> Option<AppInfo> {
    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication()?;
    let bundle_id = app.bundleIdentifier()?.to_string();
    let name = app
        .localizedName()
        .map(|n| n.to_string())
        .unwrap_or_else(|| bundle_id.clone());
    Some(AppInfo { bundle_id, name })
}

/// Lists currently-running regular (foreground, Dock-visible) apps via
/// NSWorkspace, for the Settings "assign a mode" picker. Excludes
/// background/agent processes (menu-bar-only helpers etc.) and Dev
/// Whisper itself. Same main-thread requirement as `frontmost_app_info`.
pub fn list_running_apps(own_bundle_id: &str) -> Vec<AppInfo> {
    let workspace = NSWorkspace::sharedWorkspace();
    let running = workspace.runningApplications();

    running
        .iter()
        .filter(|app| app.activationPolicy() == NSApplicationActivationPolicy::Regular)
        .filter_map(|app| {
            let bundle_id = app.bundleIdentifier()?.to_string();
            if bundle_id == own_bundle_id {
                return None;
            }
            let name = app
                .localizedName()
                .map(|n| n.to_string())
                .unwrap_or_else(|| bundle_id.clone());
            Some(AppInfo { bundle_id, name })
        })
        .collect()
}
