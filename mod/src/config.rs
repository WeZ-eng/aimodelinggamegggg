//! lands_between_dungeons.toml next to the DLL: player preferences only. Gameplay numbers live in the sheets.
use serde::Deserialize;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

#[derive(Deserialize)]
#[serde(default)]
pub struct Config {
    pub start_in_dungeons_view: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { start_in_dungeons_view: true }
    }
}

pub fn load(hmodule: usize) -> Config {
    let mut buf = [0u16; 1024];
    let n = unsafe { GetModuleFileNameW(Some(HMODULE(hmodule as _)), &mut buf) } as usize;
    let dll = std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n]));
    let path = dll.with_file_name("lands_between_dungeons.toml");
    match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
            crate::log!("config: {} unreadable ({e}); using defaults", path.display());
            Config::default()
        }),
        Err(_) => {
            crate::log!("config: none at {}; using defaults", path.display());
            Config::default()
        }
    }
}
