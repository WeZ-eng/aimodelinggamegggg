//! Our own version check, so an unsupported build leaves the mod inactive instead of panicking
//! inside the crate's RVA lookup (sheet hooks.game_version).
use fromsoftware_shared::game_version::{GameVersion, LANG_ID_EN, LANG_ID_JP};
use pelite::pe64::PeView;
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::core::PCSTR;

pub struct Supported(pub &'static str);

impl GameVersion for Supported {
    const NAME: &'static str = "elden ring";

    fn from_lang_version(lang_id: u16, version: &str) -> Option<Self> {
        match (lang_id, version) {
            (LANG_ID_EN, "2.7.1.0") => Some(Self("1.17.1 (2.7.1.0)")),
            (LANG_ID_JP, "2.7.1.1") => Some(Self("1.17.1 JP (2.7.1.1)")),
            _ => None,
        }
    }
}

pub fn check() -> Result<&'static str, String> {
    let module = unsafe {
        let h = GetModuleHandleA(PCSTR(std::ptr::null())).map_err(|e| e.to_string())?;
        PeView::module(h.0 as *const u8)
    };
    std::panic::catch_unwind(|| Supported::detect(&module))
        .map_err(|_| "could not read the game's version resource".to_string())?
        .map(|s| s.0)
        .map_err(|e| e.to_string())
}
