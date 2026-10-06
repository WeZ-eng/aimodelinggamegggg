//! ImGui overlay drawn by hudhook over the D3D12 swap chain (sheet: ui, hooks.overlay).
use crate::generated::*;
use crate::{artifacts, camera, loot, state};
use glam::Vec3;
use hudhook::imgui::{Condition, Context, Ui, WindowFlags};
use hudhook::{ImguiRenderLoop, RenderContext};

pub struct Overlay;

fn rgba(r: &RarityRow, a: f32) -> [f32; 4] {
    [r.color_r, r.color_g, r.color_b, a]
}

impl ImguiRenderLoop for Overlay {
    fn before_render<'a>(&'a mut self, ctx: &mut Context, _rc: &'a mut dyn RenderContext) {
        let open = state::with(|st| st.satchel_open).unwrap_or(false);
        ctx.io_mut().mouse_draw_cursor = open;
    }

    fn render(&mut self, ui: &mut Ui) {
        let _ = state::with(|st| draw(ui, st));
    }
}

const FLAGS: WindowFlags = WindowFlags::NO_DECORATION
    .union(WindowFlags::ALWAYS_AUTO_RESIZE)
    .union(WindowFlags::NO_SAVED_SETTINGS)
    .union(WindowFlags::NO_FOCUS_ON_APPEARING)
    .union(WindowFlags::NO_NAV);

fn anchor(row: &UiRow, display: [f32; 2]) -> ([f32; 2], [f32; 2]) {
    let (x, y, pivot) = match row.anchor {
        UiAnchor::BottomCenter => (display[0] * 0.5, display[1], [0.5, 1.0]),
        UiAnchor::TopCenter => (display[0] * 0.5, 0.0, [0.5, 0.0]),
        UiAnchor::RightCenter => (display[0], display[1] * 0.5, [1.0, 0.5]),
        _ => (display[0] * 0.5, display[1] * 0.5, [0.5, 0.5]),
    };
    ([x + row.offset_x_px as f32, y + row.offset_y_px as f32], pivot)
}

fn draw(ui: &Ui, st: &mut state::State) {
    let display = ui.io().display_size;
    let now = st.now();
    st.toasts.retain(|t| t.until > now);

    if st.dungeons_view {
        // Reticle (ui.reticle). Overlay pixels may differ from client pixels; scale the cursor.
        let sx = display[0] / st.cursor.size.0.max(1.0);
        let sy = display[1] / st.cursor.size.1.max(1.0);
        let c = [st.cursor.pos.0 * sx, st.cursor.pos.1 * sy];
        let dl = ui.get_background_draw_list();
        dl.add_circle(c, 11.0, [1.0, 1.0, 1.0, 0.85]).thickness(2.0).build();
        dl.add_circle(c, 2.5, [1.0, 1.0, 1.0, 0.9]).filled(true).build();

        // Off-screen-safe labels above loot beams.
        for d in &st.drops {
            let top = Vec3::from_array(d.pos) + Vec3::Y * (d.rarity.beam_height_m * 0.5);
            if let Some((x, y)) = camera::project(&st.cam, st.convention, top, (display[0], display[1])) {
                dl.add_text([x - 20.0, y], rgba(d.rarity, 1.0), d.rarity.label);
            }
        }

        // Hotbar (ui.hotbar).
        let (pos, pivot) = anchor(&UI_HOTBAR, display);
        ui.window("##ldb_hotbar").position(pos, Condition::Always).position_pivot(pivot).flags(FLAGS).bg_alpha(0.55).build(|| {
            for slot in 0..3 {
                if slot > 0 {
                    ui.same_line();
                }
                let id = st.save.hotbar[slot].clone();
                let a = id.as_deref().and_then(state::artifact);
                ui.group(|| {
                    ui.text_disabled(format!("[{}]", slot + 1));
                    match a {
                        Some(a) => {
                            let left = artifacts::remaining(st, a.id);
                            if left > 0.0 {
                                ui.text_disabled(a.dungeons_name);
                                ui.text_disabled(format!("{left:.1}s"));
                            } else {
                                ui.text(a.dungeons_name);
                                let cost = if a.fp_per_s > 0 { format!("{} FP/s", a.fp_per_s) } else if a.fp_cost > 0 { format!("{} FP", a.fp_cost) } else { "ready".into() };
                                ui.text_disabled(cost);
                            }
                            ui.text_colored([0.75, 0.68, 0.5, 1.0], loot::item_name(a.er_item));
                        }
                        None => {
                            ui.text_disabled("empty");
                            ui.text_disabled(" ");
                            ui.text_disabled(" ");
                        }
                    }
                });
                if slot < 2 {
                    ui.same_line();
                    ui.text_disabled(" | ");
                }
            }
        });
    }

    // Toasts (ui.toast).
    if !st.toasts.is_empty() {
        let (pos, pivot) = anchor(&UI_TOAST, display);
        ui.window("##ldb_toast").position(pos, Condition::Always).position_pivot(pivot).flags(FLAGS).bg_alpha(0.6).build(|| {
            for t in &st.toasts {
                ui.text_colored(t.color, &t.text);
            }
        });
    }

    // Satchel (ui.satchel).
    if st.satchel_open {
        let (pos, pivot) = anchor(&UI_SATCHEL, display);
        let mut equip: Option<(usize, usize)> = None;
        ui.window("Artifact satchel").position(pos, Condition::Always).position_pivot(pivot)
            .flags(WindowFlags::ALWAYS_AUTO_RESIZE | WindowFlags::NO_SAVED_SETTINGS | WindowFlags::NO_COLLAPSE)
            .build(|| {
                if st.save.owned_artifacts.is_empty() {
                    ui.text("No artifacts yet. Defeat enemies and walk into their loot beams.");
                }
                for (i, id) in st.save.owned_artifacts.iter().enumerate() {
                    let Some(a) = state::artifact(id) else { continue };
                    let mark = if i == st.satchel_sel { ">" } else { " " };
                    if ui.selectable(format!("{mark} {}  ({})##{i}", a.dungeons_name, loot::item_name(a.er_item))) {
                        st.satchel_sel = i;
                    }
                    if ui.is_item_hovered() {
                        ui.tooltip_text(a.description);
                    }
                }
                ui.separator();
                ui.text_disabled(format!("{}: next   1 / 2 / 3: put the selected artifact in that slot", CONTROLS_SATCHEL_NEXT.key_name));
                for slot in 0..3 {
                    if slot > 0 {
                        ui.same_line();
                    }
                    if ui.button(format!("Slot {}", slot + 1)) {
                        equip = Some((st.satchel_sel, slot));
                    }
                }
                ui.separator();
                gear(ui, st);
            });
        if let Some((i, s)) = equip {
            artifacts::equip(st, i, s);
        }
    }

    if st.satchel_open && !st.status.is_empty() {
        ui.window("##ldb_status").position([display[0] * 0.5, display[1] - 40.0], Condition::Always).position_pivot([0.5, 1.0]).flags(FLAGS).build(|| ui.text_disabled(&st.status));
    }
}

fn gear(ui: &Ui, st: &state::State) {
    ui.text("Enchantments");
    if st.save.enchants.is_empty() {
        ui.text_disabled("none yet");
    }
    for (item, list) in &st.save.enchants {
        let line: Vec<String> = list
            .iter()
            .filter_map(|e| state::enchantment(&e.id).map(|r| format!("{} {}", r.dungeons_name, state::roman(e.level))))
            .collect();
        ui.text_disabled(format!("item #{item:08x}: {}", line.join(", ")));
    }
}
