//! Engine bullet spawning. The crate's BulletSpawnData has private fields, so this mirrors its
//! layout exactly (checked at compile time) and hands the game a reinterpreted reference.
use eldenring::cs::{BulletSpawnData, CSBulletManager, FieldInsHandle};
use fromsoftware_shared::{F32Vector4, FromStatic};
use glam::Vec3;

#[repr(C)]
struct Spawn {
    owner: FieldInsHandle,
    behavior_id: i32,
    magic_id: i32,
    unk10: u32,
    bullet_id: i32,
    goods_id: i32,
    dummy_poly_id: i32,
    target: FieldInsHandle,
    unk28: u32,
    unk2c: u32,
    unk30: F32Vector4,
    unk40: u32,
    unk44: u32,
    pad48: [u8; 8],
    acceleration_angle: F32Vector4,
    unk60: F32Vector4,
    angle: F32Vector4,
    position: F32Vector4,
    unk90: u64,
    unk98: u64,
    unka0: u64,
    pada8: [u8; 8],
    unkb0_struct: [u8; 0x50],
    unk100: u8,
    pad101: [u8; 15],
}

const _: () = assert!(size_of::<Spawn>() == size_of::<BulletSpawnData>());
const _: () = assert!(align_of::<Spawn>() == align_of::<BulletSpawnData>());

/// Spawn `bullet_id` owned by `owner` at `pos`, flying along `dir`. Returns the game's error code on failure.
pub fn spawn(owner: FieldInsHandle, bullet_id: i32, goods_id: i32, pos: Vec3, dir: Vec3) -> Result<(), i32> {
    let mgr = unsafe { CSBulletManager::instance_mut() }.map_err(|_| -100)?;
    let mut empty: FieldInsHandle = unsafe { std::mem::zeroed() };
    unsafe { std::ptr::write_bytes(&mut empty as *mut FieldInsHandle as *mut u8, 0xff, 8) };
    let d = dir.normalize_or(Vec3::NEG_Z);
    let s = Spawn {
        owner,
        behavior_id: -1,
        magic_id: -1,
        unk10: 0,
        bullet_id,
        goods_id,
        dummy_poly_id: -1,
        target: empty,
        unk28: 0,
        unk2c: 0,
        unk30: F32Vector4(0.0, 0.0, 0.0, 0.0),
        unk40: 0,
        unk44: 0,
        pad48: [0; 8],
        acceleration_angle: F32Vector4(d.x, d.y, d.z, 0.0),
        unk60: F32Vector4(0.0, 0.0, 0.0, 0.0),
        angle: F32Vector4(d.x, d.y, d.z, 0.0),
        position: F32Vector4(pos.x, pos.y, pos.z, 1.0),
        unk90: 0,
        unk98: 0,
        unka0: 0,
        pada8: [0; 8],
        unkb0_struct: [0; 0x50],
        unk100: 0,
        pad101: [0; 15],
    };
    let data = unsafe { &*(&s as *const Spawn as *const BulletSpawnData) };
    mgr.spawn_bullet(data)
}
