//! Positions the two child WebViews inside the native window.
//!
//! The chrome has a fixed logical height at the top; the guest fills the rest. While the trusted
//! share-confirmation dialog is open ("overlay"), the chrome covers the whole window and the guest
//! is hidden so it cannot draw over or click-jack the prompt.

use tauri::{LogicalPosition, LogicalSize, Manager, Rect, Runtime, Window};

/// Logical (DPI-independent) height of the browser chrome WebView.
pub const CHROME_HEIGHT: f64 = 76.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    pub fn to_rect(self) -> Rect {
        Rect {
            position: LogicalPosition::new(self.x, self.y).into(),
            size: LogicalSize::new(self.width, self.height).into(),
        }
    }
}

/// Pure layout computation in logical pixels.
pub fn compute(window_width: f64, window_height: f64, overlay: bool) -> (Bounds, Bounds) {
    let width = window_width.max(0.0);
    let height = window_height.max(0.0);
    let chrome_height = if overlay { height } else { CHROME_HEIGHT.min(height) };
    let chrome = Bounds { x: 0.0, y: 0.0, width, height: chrome_height };
    let guest = Bounds { x: 0.0, y: CHROME_HEIGHT.min(height), width, height: (height - CHROME_HEIGHT).max(0.0) };
    (chrome, guest)
}

/// Current inner size of `window` in logical pixels (handles Retina / scale-factor changes).
pub fn logical_inner_size<R: Runtime>(window: &Window<R>) -> tauri::Result<(f64, f64)> {
    let scale = window.scale_factor()?;
    let size = window.inner_size()?.to_logical::<f64>(scale);
    Ok((size.width, size.height))
}

/// Applies the layout to the chrome and the current guest WebView.
pub fn apply<R: Runtime>(window: &Window<R>, guest_label: Option<&str>, overlay: bool) -> tauri::Result<()> {
    let (width, height) = logical_inner_size(window)?;
    let (chrome_bounds, guest_bounds) = compute(width, height, overlay);
    if let Some(chrome) = window.get_webview(crate::CHROME_LABEL) {
        chrome.set_bounds(chrome_bounds.to_rect())?;
    }
    if let Some(guest) = guest_label.and_then(|label| window.get_webview(label)) {
        guest.set_bounds(guest_bounds.to_rect())?;
        if overlay {
            guest.hide()?;
        } else {
            guest.show()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_is_fixed_height_and_guest_fills_the_rest() {
        let (chrome, guest) = compute(1200.0, 800.0, false);
        assert_eq!(chrome, Bounds { x: 0.0, y: 0.0, width: 1200.0, height: CHROME_HEIGHT });
        assert_eq!(guest, Bounds { x: 0.0, y: CHROME_HEIGHT, width: 1200.0, height: 800.0 - CHROME_HEIGHT });
        assert_eq!(chrome.height + guest.height, 800.0, "no gap and no overlap");

        let (chrome, guest) = compute(400.0, 300.0, false);
        assert_eq!(chrome.height, CHROME_HEIGHT);
        assert_eq!(guest.y, chrome.height);
    }

    #[test]
    fn overlay_gives_the_chrome_the_whole_window() {
        let (chrome, _) = compute(1000.0, 700.0, true);
        assert_eq!(chrome.height, 700.0);
    }

    #[test]
    fn tiny_windows_never_produce_negative_sizes() {
        let (chrome, guest) = compute(10.0, 20.0, false);
        assert_eq!(chrome.height, 20.0);
        assert_eq!(guest.height, 0.0);
        assert!(guest.y <= 20.0);
    }
}
