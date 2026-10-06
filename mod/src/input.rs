//! Keyboard and mouse, read with Win32 only while the game window has focus.
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, GetCursorPos, GetForegroundWindow, GetWindowThreadProcessId};

pub struct Keys {
    down: [bool; 256],
    prev: [bool; 256],
}

impl Keys {
    pub const fn new() -> Self {
        Self { down: [false; 256], prev: [false; 256] }
    }

    /// Sample every key once per frame. Nothing reads as pressed while another window has focus.
    pub fn poll(&mut self, focused: bool) {
        self.prev = self.down;
        for vk in 1..256 {
            self.down[vk] = focused && unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000 != 0;
        }
    }

    pub fn held(&self, vk: i32) -> bool {
        self.down[(vk & 0xff) as usize]
    }

    pub fn pressed(&self, vk: i32) -> bool {
        let i = (vk & 0xff) as usize;
        self.down[i] && !self.prev[i]
    }
}

/// The foreground window, if it belongs to this process (the game).
pub fn game_window() -> Option<HWND> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        (pid == std::process::id()).then_some(hwnd)
    }
}

/// Cursor in client pixels plus the client size.
pub fn cursor_client(hwnd: HWND) -> Option<((f32, f32), (f32, f32))> {
    unsafe {
        let mut p = POINT::default();
        GetCursorPos(&mut p).ok()?;
        if !ScreenToClient(hwnd, &mut p).as_bool() {
            return None;
        }
        let mut r = RECT::default();
        GetClientRect(hwnd, &mut r).ok()?;
        let (w, h) = ((r.right - r.left) as f32, (r.bottom - r.top) as f32);
        (w > 0.0 && h > 0.0).then_some(((p.x as f32, p.y as f32), (w, h)))
    }
}

/// Aim cursor. Elden Ring may hide and re-centre the OS cursor every frame (sheet hooks.mouse_cursor);
/// after 30 frames pinned to the centre we switch to accumulating offsets into a virtual cursor.
pub struct Cursor {
    pub pos: (f32, f32),
    pub size: (f32, f32),
    pinned_frames: u32,
    pub recentred: bool,
}

impl Cursor {
    pub const fn new() -> Self {
        Self { pos: (0.0, 0.0), size: (1.0, 1.0), pinned_frames: 0, recentred: false }
    }

    pub fn update(&mut self, hwnd: HWND) {
        let Some(((x, y), (w, h))) = cursor_client(hwnd) else { return };
        if self.size != (w, h) {
            self.size = (w, h);
            self.pos = (w * 0.5, h * 0.5 - h * 0.15);
        }
        let (cx, cy) = ((w * 0.5).floor(), (h * 0.5).floor());
        let near_centre = (x - cx).abs() <= 2.0 && (y - cy).abs() <= 2.0;
        if !self.recentred {
            self.pinned_frames = if near_centre { self.pinned_frames + 1 } else { 0 };
            if self.pinned_frames >= 30 {
                self.recentred = true;
                crate::log!("cursor: game re-centres the cursor; using a virtual cursor");
            }
            self.pos = (x, y);
        } else {
            self.pos.0 = (self.pos.0 + (x - cx)).clamp(0.0, w);
            self.pos.1 = (self.pos.1 + (y - cy)).clamp(0.0, h);
        }
    }

    /// Normalised device coordinates, +y up.
    pub fn ndc(&self) -> (f32, f32) {
        (self.pos.0 / self.size.0 * 2.0 - 1.0, 1.0 - self.pos.1 / self.size.1 * 2.0)
    }
}

/// True once any visible top-level window belongs to this process (boot gate).
pub fn process_has_visible_window() -> bool {
    use windows::Win32::Foundation::{LPARAM, TRUE};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, IsWindowVisible};
    use windows::core::BOOL;
    unsafe extern "system" fn cb(hwnd: HWND, lp: LPARAM) -> BOOL {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == std::process::id() && unsafe { IsWindowVisible(hwnd) }.as_bool() {
            unsafe { *(lp.0 as *mut bool) = true };
            return BOOL(0);
        }
        TRUE
    }
    let mut found = false;
    let _ = unsafe { EnumWindows(Some(cb), LPARAM(&mut found as *mut bool as isize)) };
    found
}
