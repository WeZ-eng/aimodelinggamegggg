//! Lands Between Dungeons: Minecraft Dungeons inside Elden Ring.
//! Loaded by ModEngine2 (Melty starts the game offline with Easy Anti-Cheat off). Design lives in sheets/;
//! generated.rs is built from them.
pub mod generated;
#[macro_use]
pub mod log;
mod artifacts;
mod bullet;
mod camera;
mod combat;
mod config;
mod game;
mod input;
mod loot;
mod overlay;
pub mod save;
mod state;
mod version;

use eldenring::cs::{CSTaskGroupIndex, CSTaskImp};
use eldenring::fd4::FD4TaskData;
use fromsoftware_shared::SharedTaskImpExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

static DISABLED: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
/// # Safety
/// Called by the Windows loader only.
pub unsafe extern "system" fn DllMain(hmodule: usize, reason: u32, _reserved: *mut std::ffi::c_void) -> i32 {
    if reason == 1 {
        std::thread::spawn(move || boot(hmodule));
    }
    1
}

/// Run one frame task; a panic disables the mod for the session instead of taking the game down.
fn guarded(name: &'static str, f: impl FnOnce()) {
    if DISABLED.load(Ordering::Relaxed) {
        return;
    }
    if catch_unwind(AssertUnwindSafe(f)).is_err() {
        DISABLED.store(true, Ordering::Relaxed);
        log!("{name}: panicked; the mod is now inactive for this session (Elden Ring keeps running)");
    }
}

fn boot(hmodule: usize) {
    log::init();
    std::panic::set_hook(Box::new(|info| log!("panic: {info}")));
    log!("Lands Between Dungeons {} loading", env!("CARGO_PKG_VERSION"));
    match version::check() {
        Ok(v) => log!("game: Elden Ring {v}"),
        Err(e) => {
            log!("game: unsupported build ({e}); this release needs Elden Ring 1.17.1. Mod inactive.");
            return;
        }
    }
    let cfg = config::load(hmodule);
    let seed = {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        t ^ ((std::process::id() as u64) << 32) ^ (Instant::now().elapsed().as_nanos() as u64)
    };
    state::init(seed);
    state::with(|st| st.dungeons_view = cfg.start_in_dungeons_view);

    // Boot gate (field note gotcha 1): no crate singleton before the game window exists.
    while !input::process_has_visible_window() {
        std::thread::sleep(Duration::from_millis(500));
    }
    std::thread::sleep(Duration::from_secs(2));
    let Ok(cs_task) = CSTaskImp::wait_for_instance(Duration::from_secs(120)) else {
        log!("boot: CSTask never appeared; mod inactive");
        return;
    };
    log!("boot: task runner found");

    let mut last_begin = Instant::now();
    cs_task.run_recurring(
        move |_: &FD4TaskData| {
            let dt = last_begin.elapsed().as_secs_f32().min(0.25);
            last_begin = Instant::now();
            guarded("frame_begin", || {
                state::with(|st| frame_begin(st, dt));
            });
        },
        CSTaskGroupIndex::FrameBegin,
    );
    cs_task.run_recurring(
        |_: &FD4TaskData| guarded("post_physics", || {
            state::with(|st| {
                combat::tick(st);
                loot::tick(st);
            });
        }),
        CSTaskGroupIndex::ChrIns_PostPhysics,
    );
    let mut last_draw = Instant::now();
    cs_task.run_recurring(
        move |_: &FD4TaskData| {
            let dt = last_draw.elapsed().as_secs_f32().min(0.25);
            last_draw = Instant::now();
            guarded("camera", || {
                state::with(|st| camera::render(st, dt));
            });
        },
        CSTaskGroupIndex::DrawParamUpdate,
    );
    log!("boot: tasks registered (FrameBegin, ChrIns_PostPhysics, DrawParamUpdate)");

    let hinstance = hudhook::windows::Win32::Foundation::HINSTANCE(hmodule as _);
    if let Err(e) = hudhook::Hudhook::builder()
        .with::<hudhook::hooks::dx12::ImguiDx12Hooks>(overlay::Overlay)
        .with_hmodule(hinstance)
        .build()
        .apply()
    {
        log!("overlay: hudhook failed ({e:?}); gameplay keeps working without the hotbar overlay");
    } else {
        log!("overlay: hudhook DX12 hooks applied");
    }
}

fn frame_begin(st: &mut state::State, dt: f32) {
    let hwnd = input::game_window();
    st.focused = hwnd.is_some();
    st.keys.poll(st.focused);
    if let Some(h) = hwnd {
        st.cursor.update(h);
    }
    if st.keys.pressed(generated::CONTROLS_TOGGLE_CAMERA.vk_code) {
        st.dungeons_view = !st.dungeons_view;
        log!("camera: {}", if st.dungeons_view { "Dungeons view" } else { "Elden Ring view" });
    }
    track_character(st);
    artifacts::tick(st, dt);
    camera::pin_follow_camera(st);
    let now = st.now();
    if st.save_dirty && now - st.last_save > 2.0 {
        if let Some(c) = st.character.clone() {
            save::store(&c, &st.save);
        }
        st.save_dirty = false;
        st.last_save = now;
    }
}

/// Load the mashup's save for whichever character is loaded; store the previous one first.
fn track_character(st: &mut state::State) {
    let Some(wcm) = game::world() else { return };
    let Some(player) = game::main_player(wcm) else { return };
    let name = game::character_name(game::game_data(player));
    if name.is_empty() || st.character.as_deref() == Some(name.as_str()) {
        return;
    }
    if let Some(old) = st.character.take() {
        save::store(&old, &st.save);
    }
    st.save = save::load(&name);
    log!("character: {name} ({} artifacts, {} enchanted items)", st.save.owned_artifacts.len(), st.save.enchants.len());
    st.character = Some(name);
    st.drops.clear();
    st.tracked.clear();
    st.player_hp = None;
    st.cooldown_until.clear();
}
