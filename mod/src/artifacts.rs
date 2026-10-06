//! Artifact hotbar (sheet: artifacts, controls artifact_1..3 / artifact_menu / satchel_next).
use crate::generated::*;
use crate::state::{self, State};
use crate::{bullet, game};
use eldenring::cs::ChrInsExt;
use glam::Vec3;

const SLOT_KEYS: [&ControlsRow; 3] = [&CONTROLS_ARTIFACT_1, &CONTROLS_ARTIFACT_2, &CONTROLS_ARTIFACT_3];

fn cooldown_scale(st: &State, armor: Option<u32>) -> f32 {
    let lvl = st.save.level_of(armor, ENCHANTMENTS_COOL_DOWN.id) as f32;
    (1.0 - ENCHANTMENTS_COOL_DOWN.value_per_level * lvl / 100.0).max(0.3)
}

/// Seconds left on an artifact's cooldown (for the overlay).
pub fn remaining(st: &State, id: &str) -> f32 {
    st.cooldown_until.get(id).map_or(0.0, |&u| (u - st.now()).max(0.0) as f32)
}

/// FrameBegin.
pub fn tick(st: &mut State, dt: f32) {
    if st.keys.pressed(CONTROLS_ARTIFACT_MENU.vk_code) {
        st.satchel_open = !st.satchel_open;
    }
    if st.satchel_open {
        satchel(st);
        return;
    }
    if !st.dungeons_view {
        return;
    }
    let Some(wcm) = game::world() else { return };
    let Some(player) = game::main_player(wcm) else { return };
    let pgd = game::game_data(player);
    let armor = game::armor_item(pgd);
    let feet = game::player_pos(player);
    let owner = player.chr_ins.field_ins_handle;
    let now = st.now();

    // Channelled artifact in progress.
    if let Some((slot, next)) = st.channel {
        let id = st.save.hotbar[slot].clone();
        let a = id.as_deref().and_then(state::artifact);
        let held = st.keys.held(SLOT_KEYS[slot].vk_code);
        match a {
            Some(a) if held && pgd.current_fp > 0 => {
                st.fp_debt += a.fp_per_s as f32 * dt;
                let whole = st.fp_debt.floor() as u32;
                if whole > 0 {
                    pgd.current_fp = pgd.current_fp.saturating_sub(whole);
                    st.fp_debt -= whole as f32;
                }
                if now >= next {
                    fire(st, a, owner, feet);
                    st.channel = Some((slot, now + a.channel_interval_s as f64));
                }
            }
            Some(a) => {
                st.channel = None;
                let until = now + (a.cooldown_s * cooldown_scale(st, armor)) as f64;
                st.cooldown_until.insert(a.id, until);
            }
            None => st.channel = None,
        }
        return;
    }

    for slot in 0..3 {
        if !st.keys.pressed(SLOT_KEYS[slot].vk_code) {
            continue;
        }
        let Some(a) = st.save.hotbar[slot].as_deref().and_then(state::artifact) else {
            st.status = format!("Slot {} is empty: press {} to open the satchel", slot + 1, CONTROLS_ARTIFACT_MENU.key_name);
            continue;
        };
        if remaining(st, a.id) > 0.0 {
            continue;
        }
        if pgd.current_fp < a.fp_cost as u32 {
            st.status = format!("Not enough FP for {}", a.dungeons_name);
            continue;
        }
        pgd.current_fp -= a.fp_cost as u32;
        crate::log!("artifact: {} (slot {})", a.id, slot + 1);
        if a.kind == ArtifactsKind::ChannelTowardCursor {
            st.channel = Some((slot, now));
            st.fp_debt = 0.0;
        } else {
            match a.kind {
                ArtifactsKind::SpeffectSelf => player.apply_speffect(a.effect.row_id, false),
                _ => fire(st, a, owner, feet),
            }
            let until = now + (a.cooldown_s * cooldown_scale(st, armor)) as f64;
            st.cooldown_until.insert(a.id, until);
        }
        // Health Synergy (armour enchantment).
        let hs = st.save.level_of(armor, ENCHANTMENTS_HEALTH_SYNERGY.id) as f32;
        if hs > 0.0 {
            let data = &mut player.chr_ins.modules.data;
            let heal = (data.max_hp as f32 * ENCHANTMENTS_HEALTH_SYNERGY.value_per_level * hs / 100.0) as i32;
            data.hp = (data.hp + heal).min(data.max_hp);
        }
        break;
    }
}

fn fire(st: &mut State, a: &ArtifactsRow, owner: eldenring::cs::FieldInsHandle, feet: Vec3) {
    let chest = feet + Vec3::Y * 1.2;
    let aim = st.aim_point.map(Vec3::from_array);
    let toward = aim.map(|p| (p + Vec3::Y * 1.0 - chest).normalize_or(Vec3::Z)).unwrap_or(Vec3::Z);
    let (pos, dir) = match a.kind {
        ArtifactsKind::BulletTowardCursor | ArtifactsKind::ChannelTowardCursor => (chest + toward * 0.8, toward),
        ArtifactsKind::BulletAtCursor => (aim.unwrap_or(feet) + Vec3::Y * 0.2, Vec3::NEG_Y),
        _ => (feet + Vec3::Y * 0.5, toward),
    };
    // Face the shot, as Dungeons does.
    st.aim_until = st.now() + 0.4;
    if let Err(e) = bullet::spawn(owner, a.effect.row_id, a.er_item.row_id, pos, dir) {
        crate::log!("artifact: {} bullet {} failed: error {e}", a.id, a.effect.row_id);
    }
}

fn satchel(st: &mut State) {
    let owned = st.save.owned_artifacts.len();
    if owned == 0 {
        return;
    }
    if st.keys.pressed(CONTROLS_SATCHEL_NEXT.vk_code) {
        st.satchel_sel = (st.satchel_sel + 1) % owned;
    }
    st.satchel_sel %= owned;
    for slot in 0..3 {
        if st.keys.pressed(SLOT_KEYS[slot].vk_code) {
            equip(st, st.satchel_sel, slot);
        }
    }
}

/// Put owned artifact `idx` into hotbar `slot` (also called by the overlay on click).
pub fn equip(st: &mut State, idx: usize, slot: usize) {
    let Some(id) = st.save.owned_artifacts.get(idx).cloned() else { return };
    for s in st.save.hotbar.iter_mut() {
        if s.as_deref() == Some(id.as_str()) {
            *s = None;
        }
    }
    st.save.hotbar[slot] = Some(id);
    st.save_dirty = true;
}
