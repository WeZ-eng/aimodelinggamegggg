//! Small helpers over the eldenring crate: positions, handles, the main player.
use eldenring::cs::{ChrIns, FieldInsHandle, GaitemHandle, PlayerGameData, PlayerIns, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::{F32Vector4, FromStatic};
use glam::Vec3;

const _: () = assert!(size_of::<FieldInsHandle>() == 8);
const _: () = assert!(size_of::<GaitemHandle>() == 4);

pub fn handle_key(h: &FieldInsHandle) -> u64 {
    unsafe { std::mem::transmute_copy::<FieldInsHandle, u64>(h) }
}

pub fn gaitem_key(h: &GaitemHandle) -> u32 {
    unsafe { std::mem::transmute_copy::<GaitemHandle, u32>(h) }
}

pub fn v3(p: &HavokPosition) -> Vec3 {
    Vec3::new(p.0, p.1, p.2)
}

pub fn hp(v: Vec3) -> HavokPosition {
    HavokPosition(v.x, v.y, v.z, 0.0)
}

pub fn v4(v: Vec3, w: f32) -> F32Vector4 {
    F32Vector4(v.x, v.y, v.z, w)
}

pub fn world() -> Option<&'static mut WorldChrMan> {
    unsafe { WorldChrMan::instance_mut() }.ok()
}

pub fn main_player(wcm: &mut WorldChrMan) -> Option<&mut PlayerIns> {
    wcm.main_player.as_deref_mut()
}

pub fn game_data(player: &mut PlayerIns) -> &'static mut PlayerGameData {
    unsafe { player.player_game_data.as_mut() }
}

pub fn player_pos(player: &PlayerIns) -> Vec3 {
    v3(&player.chr_ins.modules.physics.position)
}

pub fn chr_pos(chr: &ChrIns) -> Vec3 {
    v3(&chr.modules.physics.position)
}

pub fn character_name(pgd: &PlayerGameData) -> String {
    let n = pgd.character_name.iter().position(|&c| c == 0).unwrap_or(pgd.character_name.len());
    String::from_utf16_lossy(&pgd.character_name[..n])
}

/// Inventory handles of the item each enchantment slot type applies to.
pub fn weapon_item(pgd: &PlayerGameData) -> Option<u32> {
    let asm = &pgd.equipment.chr_asm;
    let h = gaitem_key(&asm.gaitem_handles[asm.equipment.active_right_weapon_slot()]);
    (h != 0 && h != u32::MAX).then_some(h)
}

pub fn armor_item(pgd: &PlayerGameData) -> Option<u32> {
    let asm = &pgd.equipment.chr_asm;
    let h = gaitem_key(&asm.gaitem_handles[eldenring::cs::ChrAsmSlot::ProtectorChest]);
    (h != 0 && h != u32::MAX).then_some(h)
}
