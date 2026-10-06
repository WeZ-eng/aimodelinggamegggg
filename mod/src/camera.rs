//! Dungeons camera (sheet: camera, hooks.render_camera / follow_camera / phys_raycast).
use crate::game;
use crate::generated::*;
use crate::state::{CamConvention, CamFrame, State};
use eldenring::cs::{CSCamera, CSHavokMan};
use fromsoftware_shared::{F32Vector4, FromStatic};
use glam::Vec3;

/// Map collision filter from the CS2-in-Elden-Ring field note (sheet hooks.phys_raycast: unverified).
const MAP_FILTER: u32 = 0x2000058;

fn row(v: &F32Vector4) -> Vec3 {
    Vec3::new(v.0, v.1, v.2)
}

fn look_dir(yaw: f32, pitch: f32) -> Vec3 {
    let h = Vec3::new(yaw.sin(), 0.0, yaw.cos());
    (h * pitch.cos() - Vec3::Y * pitch.sin()).normalize()
}

/// Camera rotation rows for a look direction, in the game's own convention.
fn basis(look: Vec3, conv: &CamConvention) -> (Vec3, Vec3, Vec3) {
    let up = (Vec3::Y - look * Vec3::Y.dot(look)).normalize_or(Vec3::Z);
    let fwd_row = look * conv.forward_sign;
    let right = (up.cross(fwd_row) * conv.handedness).normalize();
    (right, up, fwd_row)
}

/// Read the vanilla camera once to learn which way its rows point and how fov is stored.
fn learn(st: &mut State, cam_rows: [Vec3; 4], fov: f32, target: Vec3) {
    if st.convention.is_some() {
        return;
    }
    let look = (target - cam_rows[3]).normalize_or_zero();
    if look == Vec3::ZERO {
        return;
    }
    let forward_sign = if cam_rows[2].dot(look) >= 0.0 { 1.0 } else { -1.0 };
    let handedness = if cam_rows[1].cross(cam_rows[2]).dot(cam_rows[0]) >= 0.0 { 1.0 } else { -1.0 };
    let conv = CamConvention { forward_sign, handedness, fov_is_radians: fov < 3.2 };
    crate::log!(
        "camera: vanilla convention forward_sign={forward_sign} handedness={handedness} fov={fov} ({})",
        if conv.fov_is_radians { "radians" } else { "degrees" }
    );
    st.convention = Some(conv);
}

/// FrameBegin: pin the follow camera's yaw so movement input is relative to the Dungeons screen.
pub fn pin_follow_camera(st: &mut State) {
    if !st.dungeons_view {
        return;
    }
    let Some(conv) = st.convention else { return };
    let Some(wcm) = game::world() else { return };
    let Some(mut chr_cam) = wcm.chr_cam else { return };
    let cam = unsafe { chr_cam.as_mut() };
    let (r, u, f) = basis(look_dir(CAMERA_DUNGEONS.yaw_deg.to_radians(), 0.0), &conv);
    let m = &mut cam.pers_cam.matrix;
    m.0 = game::v4(r, 0.0);
    m.1 = game::v4(u, 0.0);
    m.2 = game::v4(f, 0.0);
}

/// DrawParamUpdate (right after CameraStep): replace the rendered camera.
pub fn render(st: &mut State, dt: f32) {
    let Some(wcm) = game::world() else { return };
    let Some(player) = game::main_player(wcm) else { return };
    let feet = game::player_pos(player);
    let Ok(camera) = (unsafe { CSCamera::instance_mut() }) else { return };
    let pc = &mut camera.pers_cam_1;
    let rows = [row(&pc.matrix.0), row(&pc.matrix.1), row(&pc.matrix.2), row(&pc.matrix.3)];
    let c = &CAMERA_DUNGEONS;
    learn(st, rows, pc.fov, feet + Vec3::Y * c.target_height_m);

    if !st.dungeons_view || !c.overrides_render {
        st.cam.valid = false;
        st.smoothed_target = None;
        return;
    }
    let Some(conv) = st.convention else { return };

    // Zoom (controls zoom_in / zoom_out are held keys; step is per second).
    if st.keys.held(CONTROLS_ZOOM_IN.vk_code) {
        st.distance -= c.zoom_step_m * 6.0 * dt;
    }
    if st.keys.held(CONTROLS_ZOOM_OUT.vk_code) {
        st.distance += c.zoom_step_m * 6.0 * dt;
    }
    st.distance = st.distance.clamp(c.distance_min_m, c.distance_max_m);

    let target_now = feet + Vec3::Y * c.target_height_m;
    let target = match st.smoothed_target.map(Vec3::from_array) {
        Some(prev) if prev.distance(target_now) < 20.0 => prev.lerp(target_now, 1.0 - c.follow_smoothing),
        _ => target_now,
    };
    st.smoothed_target = Some(target.to_array());

    let look = look_dir(c.yaw_deg.to_radians(), c.pitch_deg.to_radians());
    let mut dist = st.distance;
    if c.ceiling_mode == CameraCeilingMode::Shorten {
        if let Ok(havok) = unsafe { CSHavokMan::instance() } {
            if let Some(hit) = havok.phys_world.cast_ray(MAP_FILTER, &game::hp(target), eldenring::position::PositionDelta(-look.x * dist, -look.y * dist, -look.z * dist), &*player) {
                let d = game::v3(&hit).distance(target) - c.ceiling_margin_m;
                dist = d.clamp(2.0, dist);
            }
        }
    }
    let pos = target - look * dist;
    let (r, u, f) = basis(look, &conv);
    pc.matrix.0 = game::v4(r, 0.0);
    pc.matrix.1 = game::v4(u, 0.0);
    pc.matrix.2 = game::v4(f, 0.0);
    pc.matrix.3 = game::v4(pos, 1.0);
    let fov_rad = c.fov_deg.to_radians();
    pc.fov = if conv.fov_is_radians { fov_rad } else { c.fov_deg };

    st.cam = CamFrame {
        pos: pos.to_array(),
        right: (r).to_array(),
        up: u.to_array(),
        look: look.to_array(),
        tan_half_fov_y: (fov_rad * 0.5).tan(),
        aspect: if pc.aspect_ratio > 0.1 { pc.aspect_ratio } else { 16.0 / 9.0 },
        valid: true,
    };

    // Aim point: the cursor's ray meets the ground plane at the player's feet.
    let (nx, ny) = st.cursor.ndc();
    // Row 0 is taken to be screen-right in the game's own convention (verify: aim must not be mirrored).
    let right_screen = Vec3::from_array(st.cam.right);
    let dir = (look + right_screen * (nx * st.cam.tan_half_fov_y * st.cam.aspect) + u * (ny * st.cam.tan_half_fov_y)).normalize();
    st.aim_point = (dir.y < -0.01).then(|| (pos + dir * ((feet.y - pos.y) / dir.y)).to_array());
}

/// Screen position of a world point, for the overlay.
pub fn project(cam: &CamFrame, conv: Option<CamConvention>, p: Vec3, screen: (f32, f32)) -> Option<(f32, f32)> {
    if !cam.valid {
        return None;
    }
    let conv = conv?;
    let rel = p - Vec3::from_array(cam.pos);
    let z = rel.dot(Vec3::from_array(cam.look));
    if z < 0.1 {
        return None;
    }
    let _ = conv;
    let right_screen = Vec3::from_array(cam.right);
    let x = rel.dot(right_screen) / (z * cam.tan_half_fov_y * cam.aspect);
    let y = rel.dot(Vec3::from_array(cam.up)) / (z * cam.tan_half_fov_y);
    Some(((x + 1.0) * 0.5 * screen.0, (1.0 - y) * 0.5 * screen.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(fs: f32, hd: f32) -> CamConvention {
        CamConvention { forward_sign: fs, handedness: hd, fov_is_radians: true }
    }

    #[test]
    fn basis_is_orthonormal_for_every_convention() {
        for (fs, hd) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            let c = conv(fs, hd);
            let (r, u, f) = basis(look_dir(0.8, 0.96), &c);
            assert!((r.length() - 1.0).abs() < 1e-5 && (u.length() - 1.0).abs() < 1e-5);
            assert!(r.dot(u).abs() < 1e-5 && r.dot(f).abs() < 1e-5 && u.dot(f).abs() < 1e-5);
            assert!(u.y > 0.0, "camera up points skyward");
            // learn() must recover the convention from a matrix built with it
            let recovered = if u.cross(f).dot(r) >= 0.0 { 1.0 } else { -1.0 };
            assert_eq!(recovered, hd);
        }
    }

    #[test]
    fn steep_look_points_down() {
        let l = look_dir(45f32.to_radians(), 55f32.to_radians());
        assert!(l.y < -0.8 && l.y > -0.83);
    }

    #[test]
    fn centre_of_screen_projects_to_centre() {
        let c = conv(1.0, 1.0);
        let look = look_dir(0.3, 0.9);
        let (r, u, _) = basis(look, &c);
        let cam = CamFrame { pos: [0.0, 10.0, 0.0], right: r.to_array(), up: u.to_array(), look: look.to_array(), tan_half_fov_y: 0.36, aspect: 16.0 / 9.0, valid: true };
        let p = Vec3::new(0.0, 10.0, 0.0) + look * 12.0;
        let (x, y) = project(&cam, Some(c), p, (1920.0, 1080.0)).unwrap();
        assert!((x - 960.0).abs() < 0.5 && (y - 540.0).abs() < 0.5);
    }
}
