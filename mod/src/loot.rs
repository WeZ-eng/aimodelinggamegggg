//! Dungeons loot: beams at kills, walk in to pick up (sheets: loot, rarity, artifacts, enchantments).
use crate::combat::Kill;
use crate::generated::*;
use crate::state::{self, Drop, Reward, State};
use crate::game;
use eldenring::cs::RendMan;
use fromsoftware_shared::{F32Vector4, FromStatic};
use glam::Vec3;

fn source_for(st: &State, max_hp: i32) -> Option<&'static LootRow> {
    if !st.save.first_kill_done {
        return LOOT.iter().copied().find(|l| l.first_kill_only);
    }
    LOOT.iter().copied().find(|l| !l.first_kill_only && max_hp >= l.min_enemy_max_hp && max_hp <= l.max_enemy_max_hp)
}

pub fn on_kill(st: &mut State, k: &Kill) {
    if st.character.is_none() {
        return;
    }
    let Some(src) = source_for(st, k.max_hp) else { return };
    if src.first_kill_only {
        st.save.first_kill_done = true;
        st.save_dirty = true;
    }
    if !st.chance(src.chance_pct) {
        return;
    }
    let tiers: Vec<(&'static RarityRow, i32)> =
        RARITY.iter().copied().filter(|r| r.tier >= src.min_rarity.tier).map(|r| (r, r.roll_weight)).collect();
    let Some(rarity) = st.pick_weighted(&tiers) else { return };
    let rewards = [(Reward::Artifact, src.w_artifact), (Reward::WeaponEnchant, src.w_weapon_enchant), (Reward::ArmorEnchant, src.w_armor_enchant)];
    let Some(reward) = st.pick_weighted(&rewards) else { return };
    let expires = st.now() + src.beam_ttl_s as f64;
    crate::log!("drop: {} {:?} from {}", rarity.id, reward, src.id);
    st.drops.push(Drop { pos: k.pos.to_array(), rarity, reward, pickup_radius: src.pickup_radius_m, expires });
}

/// ChrIns_PostPhysics: draw beams, pick up, expire.
pub fn tick(st: &mut State) {
    let now = st.now();
    st.drops.retain(|d| d.expires > now);
    if st.drops.is_empty() {
        return;
    }
    let Some(wcm) = game::world() else { return };
    let Some(player) = game::main_player(wcm) else { return };
    let feet = game::player_pos(player);
    let pgd = game::game_data(player);
    let (weapon, armor) = (game::weapon_item(pgd), game::armor_item(pgd));

    if let Some(draw) = unsafe { RendMan::instance_mut() }.ok().map(|r| r.debug_ez_draw.as_mut()) {
        for d in &st.drops {
            let base = Vec3::from_array(d.pos);
            let c = F32Vector4(d.rarity.color_r, d.rarity.color_g, d.rarity.color_b, 0.85);
            draw.set_color(&c);
            draw.draw_capsule(&game::hp(base + Vec3::Y * d.rarity.beam_height_m), &game::hp(base), d.rarity.beam_radius_m);
        }
    }

    let mut picked = Vec::new();
    for (i, d) in st.drops.iter().enumerate() {
        let mut flat = Vec3::from_array(d.pos) - feet;
        let dy = flat.y.abs();
        flat.y = 0.0;
        if flat.length() <= d.pickup_radius && dy < 3.0 {
            picked.push(i);
        }
    }
    for i in picked.into_iter().rev() {
        let d = st.drops.remove(i);
        grant(st, d.rarity, d.reward, weapon, armor);
    }
}

fn grant(st: &mut State, rarity: &'static RarityRow, reward: Reward, weapon: Option<u32>, armor: Option<u32>) {
    match reward {
        Reward::Artifact => {
            let pool: Vec<(&'static ArtifactsRow, i32)> = ARTIFACTS
                .iter()
                .copied()
                .filter(|a| !st.save.owned_artifacts.iter().any(|o| o == a.id))
                .map(|a| (a, a.drop_weight))
                .collect();
            let Some(a) = st.pick_weighted(&pool) else {
                return grant(st, rarity, Reward::WeaponEnchant, weapon, armor);
            };
            st.save.owned_artifacts.push(a.id.to_string());
            if let Some(slot) = st.save.hotbar.iter().position(|s| s.is_none()) {
                st.save.hotbar[slot] = Some(a.id.to_string());
                st.toast(format!("{}  {}  ({})  slotted on {}", rarity.label, a.dungeons_name, item_name(a.er_item), slot + 1), rarity);
            } else {
                st.toast(format!("{}  {}  ({})  in your satchel", rarity.label, a.dungeons_name, item_name(a.er_item)), rarity);
            }
        }
        Reward::WeaponEnchant | Reward::ArmorEnchant => {
            let (slot, item, what) = if reward == Reward::WeaponEnchant {
                (EnchantmentsSlot::Weapon, weapon, "your weapon")
            } else {
                (EnchantmentsSlot::Armor, armor, "your armour")
            };
            let Some(item) = item else {
                st.toast(format!("{}  enchantment fizzles: nothing equipped", rarity.label), rarity);
                return;
            };
            let pool: Vec<(&'static EnchantmentsRow, i32)> =
                ENCHANTMENTS.iter().copied().filter(|e| e.slot == slot).map(|e| (e, e.drop_weight)).collect();
            let Some(e) = st.pick_weighted(&pool) else { return };
            let lvl = rarity.enchant_level as u8;
            let changed = st.save.add_enchant(item, e.id, lvl);
            let now_lvl = st.save.level_of(Some(item), e.id);
            st.toast(
                format!("{}  {} {}  on {}{}", rarity.label, e.dungeons_name, state::roman(now_lvl), what, if changed { "" } else { " (already had it)" }),
                rarity,
            );
        }
    }
    st.save_dirty = true;
}

/// The Elden Ring item an artifact is made from (sheet hooks.item_names: Paramdex name until a msg lookup is found).
pub fn item_name(row: &EffectsRow) -> &'static str {
    let n = row.paramdex_name;
    match n.find("] ") {
        Some(i) if n.starts_with('[') => &n[i + 2..],
        _ => n,
    }
}
