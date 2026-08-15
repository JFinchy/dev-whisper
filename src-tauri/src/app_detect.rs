use objc2_app_kit::NSWorkspace;

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
