//! Hits, kills and enchantments (sheets: enchantments, hooks.enemy_list / enemy_hp / player_orientation).
//! A hit is an HP drop on a character whose last_hit_by is the local player.
use crate::generated::*;
use crate::state::{State, Tracked};
use crate::{game, loot};
use eldenring::cs::{ChrIns, ChrInsExt};
use glam::{Quat, Vec3};
use std::ptr::NonNull;

pub struct Kill {
    pub pos: Vec3,
    pub max_hp: i32,
}

/// ChrIns_PostPhysics.
pub fn tick(st: &mut State) {
    let Some(wcm) = game::world() else { return };
    let enemies: Vec<NonNull<ChrIns>> = wcm.chr_inses_by_distance.iter().map(|e| e.chr_ins).collect();
    let Some(player) = game::main_player(wcm) else { return };
    let me = player.chr_ins.field_ins_handle;
    let me_ptr = &player.chr_ins as *const ChrIns;
    let my_team = player.chr_ins.team_type;
    let feet = game::player_pos(player);
    let pgd = game::game_data(player);
    let weapon = game::weapon_item(pgd);
    let armor = game::armor_item(pgd);
    let now = st.now();

    face_cursor(st, player, feet);

    // Player hurt -> Thorns.
    let php = player.chr_ins.modules.data.hp;
    if let Some(prev) = st.player_hp {
        if php < prev {
            let lvl = st.save.level_of(armor, ENCHANTMENTS_THORNS.id);
            if lvl > 0 {
                let back = ((prev - php) as f32 * ENCHANTMENTS_THORNS.value_per_level * lvl as f32 / 100.0) as i32;
                let attacker = player.chr_ins.last_hit_by;
                if let Some(a) = enemies.iter().map(|p| unsafe { &mut *p.as_ptr() }).find(|c| c.field_ins_handle == attacker) {
                    damage(st, a, back);
                }
            }
        }
    }
    st.player_hp = Some(php);

    let mut kills = Vec::new();
    for ptr in &enemies {
        if ptr.as_ptr() as *const ChrIns == me_ptr {
            continue;
        }
        let chr = unsafe { &mut *ptr.as_ptr() };
        let key = game::handle_key(&chr.field_ins_handle);
        let (hp, max_hp) = (chr.modules.data.hp, chr.modules.data.max_hp);
        let prev = st.tracked.get(&key).map(|t| t.hp);
        st.tracked.insert(key, Tracked { hp, seen: now });
        let Some(prev) = prev else { continue };
        if hp >= prev || chr.last_hit_by != me {
            continue;
        }
        let dealt = prev - hp;
        if hp > 0 {
            on_hit(st, chr, dealt, weapon, &enemies, me_ptr, my_team, feet);
        }
        if chr.modules.data.hp <= 0 {
            kills.push(Kill { pos: game::chr_pos(chr), max_hp });
        }
    }
    st.tracked.retain(|_, t| now - t.seen < 10.0);

    for k in kills {
        crate::log!("kill: max_hp={} at {:?}", k.max_hp, k.pos);
        let lvl = st.save.level_of(weapon, ENCHANTMENTS_LEECHING.id);
        if lvl > 0 {
            let data = &mut player.chr_ins.modules.data;
            let heal = (data.max_hp as f32 * ENCHANTMENTS_LEECHING.value_per_level * lvl as f32 / 100.0) as i32;
            data.hp = (data.hp + heal).min(data.max_hp);
        }
        loot::on_kill(st, &k);
    }
}

/// Extra damage from the mod. The tracked HP is updated so the mod's own write is not seen as a hit.
fn damage(st: &mut State, chr: &mut ChrIns, amount: i32) {
    if amount <= 0 {
        return;
    }
    let data = &mut chr.modules.data;
    data.hp = (data.hp - amount).max(0);
    let key = game::handle_key(&chr.field_ins_handle);
    if let Some(t) = st.tracked.get_mut(&key) {
        t.hp = chr.modules.data.hp;
    }
}

#[allow(clippy::too_many_arguments)]
fn on_hit(st: &mut State, chr: &mut ChrIns, dealt: i32, weapon: Option<u32>, all: &[NonNull<ChrIns>], me_ptr: *const ChrIns, my_team: u8, feet: Vec3) {
    let pct_hp = chr.modules.data.hp as f32 / chr.modules.data.max_hp.max(1) as f32 * 100.0;
    let mut extra = 0.0f32;
    for e in ENCHANTMENTS.iter().filter(|e| e.slot == EnchantmentsSlot::Weapon && e.trigger == EnchantmentsTrigger::OnHit) {
        let lvl = st.save.level_of(weapon, e.id);
        if lvl == 0 || pct_hp >= e.target_hp_below_pct {
            continue;
        }
        if e.chance_pct_per_level < 100.0 && !st.chance(e.chance_pct_per_level * lvl as f32) {
            continue;
        }
        if e.game_effect.param == EffectsParam::SpEffectParam {
            chr.apply_speffect(e.game_effect.row_id, false);
        } else if e.value_unit == EnchantmentsValueUnit::PctHitDamage {
            let per = if e.chance_pct_per_level < 100.0 { e.value_per_level } else { e.value_per_level * lvl as f32 };
            extra += dealt as f32 * per / 100.0;
        }
    }
    damage(st, chr, extra as i32);

    // Swirling: every nth hit also hits enemies around the player.
    let sw = &ENCHANTMENTS_SWIRLING;
    let lvl = st.save.level_of(weapon, sw.id);
    st.hit_counter += 1;
    if lvl > 0 && st.hit_counter % sw.nth.max(2) as u32 == 0 {
        let amount = (dealt as f32 * sw.value_per_level * lvl as f32 / 100.0) as i32;
        let hit_ptr = chr as *const ChrIns;
        for p in all {
            let other = unsafe { &mut *p.as_ptr() };
            let op = other as *const ChrIns;
            if op == me_ptr || op == hit_ptr || other.team_type == my_team || other.modules.data.hp <= 0 {
                continue;
            }
            if game::chr_pos(other).distance(feet) <= sw.radius_m {
                damage(st, other, amount);
            }
        }
    }
}

/// Turn toward the cursor while attacking or right after an artifact (controls aim_attack / aim_guard).
fn face_cursor(st: &mut State, player: &mut eldenring::cs::PlayerIns, feet: Vec3) {
    if !st.dungeons_view || st.satchel_open {
        return;
    }
    let attacking = st.keys.held(CONTROLS_AIM_ATTACK.vk_code) || st.keys.held(CONTROLS_AIM_GUARD.vk_code);
    if attacking {
        st.aim_until = st.now() + 0.25;
    }
    if st.now() > st.aim_until {
        return;
    }
    let Some(aim) = st.aim_point.map(Vec3::from_array) else { return };
    let mut d = aim - feet;
    d.y = 0.0;
    if d.length_squared() < 0.04 {
        return;
    }
    // Character forward is -Z in its own space (fromsoftware-rs debug-line example).
    let q = Quat::from_rotation_arc(Vec3::NEG_Z, d.normalize());
    player.chr_ins.modules.physics.orientation = eldenring::rotation::Quaternion(q.x, q.y, q.z, q.w);
}
