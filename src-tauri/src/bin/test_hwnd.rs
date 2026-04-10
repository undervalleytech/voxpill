use tauri::Manager;
fn main() {
    let app: tauri::AppHandle = unimplemented!();
    let w = app.get_webview_window("rec-hud").unwrap();
    let hwnd = w.hwnd().unwrap();
    let _h: *mut std::ffi::c_void = hwnd.0 as _;
}
