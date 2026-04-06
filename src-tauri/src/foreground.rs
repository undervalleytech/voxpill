//! Fenêtre au premier plan Windows : sauvegarde au début de la dictée, restauration avant collage.

#[cfg(windows)]
pub fn capture_foreground_window() -> Option<usize> {
    use winapi::um::winuser::GetForegroundWindow;
    unsafe {
        let h = GetForegroundWindow();
        if h.is_null() {
            None
        } else {
            Some(h as usize)
        }
    }
}

#[cfg(windows)]
pub fn try_focus_window(hwnd: usize) {
    use winapi::shared::windef::HWND;
    use winapi::um::winuser::{AllowSetForegroundWindow, SetForegroundWindow, ASFW_ANY};
    unsafe {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        SetForegroundWindow(hwnd as HWND);
    }
}

#[cfg(not(windows))]
pub fn capture_foreground_window() -> Option<usize> {
    None
}

#[cfg(not(windows))]
pub fn try_focus_window(_hwnd: usize) {}
