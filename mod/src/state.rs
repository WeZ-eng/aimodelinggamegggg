//! Everything the game tasks and the overlay share. Plain data only: no game pointers live here.
use crate::generated::*;
use crate::input::{Cursor, Keys};
use crate::save::CharacterSave;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

pub struct Drop {
    pub pos: [f32; 3],
    pub rarity: &'static RarityRow,
    pub reward: Reward,
    pub pickup_radius: f32,
    pub expires: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reward {
    Artifact,
    WeaponEnchant,
    ArmorEnchant,
}

pub struct Toast {
    pub text: String,
    pub color: [f32; 4],
    pub until: f64,
}

pub struct Tracked {
    pub hp: i32,
    pub seen: f64,
}

/// What the game tasks learn about the camera so the overlay can project and the next frame can reuse it.
#[derive(Clone, Copy, Default)]
pub struct CamFrame {
    pub pos: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    /// From the camera toward what it looks at.
    pub look: [f32; 3],
    pub tan_half_fov_y: f32,
    pub aspect: f32,
    pub valid: bool,
}

/// Conventions read from the vanilla camera before the first override (sheet camera.dungeons._verify).
#[derive(Clone, Copy)]
pub struct CamConvention {
    /// +1 when matrix row 2 points where the camera looks, -1 when it points backwards.
    pub forward_sign: f32,
    /// +1 when right = cross(up, forward_row), -1 otherwise.
    pub handedness: f32,
    pub fov_is_radians: bool,
}

pub struct State {
    pub t0: Instant,
    pub keys: Keys,
    pub cursor: Cursor,
    pub focused: bool,

    pub dungeons_view: bool,
    pub distance: f32,
    pub smoothed_target: Option<[f32; 3]>,
    pub convention: Option<CamConvention>,
    pub cam: CamFrame,
    pub aim_point: Option<[f32; 3]>,
    pub aim_until: f64,

    pub character: Option<String>,
    pub save: CharacterSave,
    pub save_dirty: bool,
    pub last_save: f64,

    pub cooldown_until: HashMap<&'static str, f64>,
    pub channel: Option<(usize, f64)>,
    pub fp_debt: f32,
    pub satchel_open: bool,
    pub satchel_sel: usize,

    pub tracked: HashMap<u64, Tracked>,
    pub hit_counter: u32,
    pub player_hp: Option<i32>,
    pub rng: u64,

    pub drops: Vec<Drop>,
    pub toasts: Vec<Toast>,
    pub status: String,
}

impl State {
    pub fn now(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    pub fn toast(&mut self, text: String, rarity: &RarityRow) {
        let until = self.now() + 4.0;
        crate::log!("toast: {text}");
        self.toasts.push(Toast { text, color: [rarity.color_r, rarity.color_g, rarity.color_b, 1.0], until });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    /// xorshift64*, seeded from the performance counter and pid at boot (see field note gotcha 15).
    pub fn rand(&mut self) -> f64 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        (x.wrapping_mul(0x2545F4914F6CDD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn chance(&mut self, pct: f32) -> bool {
        self.rand() * 100.0 < pct as f64
    }

    pub fn pick_weighted<T: Copy>(&mut self, items: &[(T, i32)]) -> Option<T> {
        let total: i32 = items.iter().map(|(_, w)| (*w).max(0)).sum();
        if total <= 0 {
            return None;
        }
        let mut r = self.rand() * total as f64;
        for (t, w) in items {
            r -= (*w).max(0) as f64;
            if r < 0.0 {
                return Some(*t);
            }
        }
        items.last().map(|(t, _)| *t)
    }
}

pub static STATE: Mutex<Option<State>> = Mutex::new(None);

pub fn init(seed: u64) {
    *STATE.lock().unwrap() = Some(State {
        t0: Instant::now(),
        keys: Keys::new(),
        cursor: Cursor::new(),
        focused: false,
        dungeons_view: true,
        distance: CAMERA_DUNGEONS.distance_m,
        smoothed_target: None,
        convention: None,
        cam: CamFrame::default(),
        aim_point: None,
        aim_until: 0.0,
        character: None,
        save: CharacterSave::default(),
        save_dirty: false,
        last_save: 0.0,
        cooldown_until: HashMap::new(),
        channel: None,
        fp_debt: 0.0,
        satchel_open: false,
        satchel_sel: 0,
        tracked: HashMap::new(),
        hit_counter: 0,
        player_hp: None,
        rng: seed | 1,
        drops: Vec::new(),
        toasts: Vec::new(),
        status: String::from("starting"),
    });
}

/// Run `f` with the state locked. Game tasks call this once per task per frame.
pub fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    let mut g = STATE.lock().ok()?;
    g.as_mut().map(f)
}

pub fn artifact(id: &str) -> Option<&'static ArtifactsRow> {
    ARTIFACTS.iter().copied().find(|a| a.id == id)
}

pub fn enchantment(id: &str) -> Option<&'static EnchantmentsRow> {
    ENCHANTMENTS.iter().copied().find(|e| e.id == id)
}

pub fn roman(n: u8) -> &'static str {
    ["", "I", "II", "III", "IV", "V"].get(n as usize).copied().unwrap_or("?")
}
