//! Window › Timeline: a floating panel that creates and drives the document timeline. All actions
//! run the engine `timeline.*` commands, so the control channel drives it the same way.

use egui::{RichText, vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::theme::Tokens;

type Cache = std::collections::HashMap<usize, (u64, egui::TextureHandle)>;

fn yes() -> bool {
    true
}
fn one() -> u8 {
    1
}
fn ghost() -> f32 {
    0.35
}

/// Timeline panel view state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimelineUi {
    pub open: bool,
    /// Playing back (auto-advancing the playhead at the frame rate).
    #[serde(default)]
    pub playing: bool,
    /// egui time (seconds) of the last auto-advance.
    #[serde(default, skip)]
    pub last_time: f64,
    /// Render Video format: "avi", "gif" or "png" (sequence).
    #[serde(default)]
    pub fmt: String,
    /// Frame cells show a preview instead of the number.
    #[serde(default = "yes")]
    pub thumbs: bool,
    /// Show neighbouring frames faintly on the canvas.
    #[serde(default)]
    pub onion: bool,
    #[serde(default = "one")]
    pub onion_before: u8,
    #[serde(default = "one")]
    pub onion_after: u8,
    #[serde(default = "ghost")]
    pub onion_opacity: f32,
    /// Tint past frames red and future frames green.
    #[serde(default = "yes")]
    pub onion_tint: bool,
    /// Show the per-layer track rows.
    #[serde(default = "yes")]
    pub tracks: bool,
    #[serde(skip)]
    thumb_cache: Cache,
    #[serde(skip)]
    onion_cache: Cache,
}

impl Default for TimelineUi {
    fn default() -> Self {
        Self {
            open: false,
            playing: false,
            last_time: 0.0,
            fmt: String::new(),
            thumbs: true,
            onion: false,
            onion_before: 1,
            onion_after: 1,
            onion_opacity: 0.35,
            onion_tint: true,
            tracks: true,
            thumb_cache: Cache::new(),
            onion_cache: Cache::new(),
        }
    }
}

/// A frame preview texture, rendered again only when the frame's look changed.
fn preview(cache: &mut Cache, ctx: &egui::Context, s: &photocraft_engine::Session, frame: usize, w: u32, h: u32) -> Option<egui::TextureHandle> {
    let key = photocraft_engine::frame_key(s, frame) ^ (u64::from(w) << 32 | u64::from(h));
    if let Some((k, t)) = cache.get(&frame)
        && *k == key
    {
        return Some(t.clone());
    }
    let buf = photocraft_engine::frame_preview(s, frame, w, h)?;
    let tex = ctx.load_texture(format!("tl_frame_{frame}"), crate::canvas::buffer_to_image(&buf), egui::TextureOptions::LINEAR);
    cache.insert(frame, (key, tex.clone()));
    Some(tex)
}

/// Onion skin: past and future frames drawn faintly over the canvas.
pub fn draw_onion(app: &mut PhotocraftApp, painter: &egui::Painter, xf: &crate::canvas::ViewXform) {
    let u = &app.ui.timeline;
    if !u.onion || u.playing {
        return;
    }
    let (before, after, op, tint) = (u.onion_before, u.onion_after, u.onion_opacity, u.onion_tint);
    let Some(st) = app.session.active() else { return };
    let Some(t) = &st.doc.timeline else { return };
    let (cur, dur, b) = (t.current, t.duration, st.doc.bounds());
    let side = (b.width().max(b.height()) as f32).max(1.0);
    let k = (512.0 / side).min(1.0);
    let (w, h) = (((b.width() as f32 * k) as u32).max(1), ((b.height() as f32 * k) as u32).max(1));
    let area = xf.doc_rect(b);
    let ctx = painter.ctx().clone();
    for (n, dir) in [(before, -1i64), (after, 1)] {
        for i in 1..=i64::from(n) {
            let f = cur as i64 + dir * i;
            if f < 0 || f >= dur as i64 {
                continue;
            }
            let fade = 1.0 - (i - 1) as f32 / f64::from(n).max(1.0) as f32;
            let a = (op * fade).clamp(0.0, 1.0);
            let c = match (tint, dir < 0) {
                (true, true) => [255.0, 110.0, 110.0],
                (true, false) => [110.0, 255.0, 110.0],
                _ => [255.0; 3],
            };
            let col = egui::Color32::from_rgba_unmultiplied(c[0] as u8, c[1] as u8, c[2] as u8, (a * 255.0) as u8);
            if let Some(tex) = preview(&mut app.ui.timeline.onion_cache, &ctx, &app.session, f as usize, w, h) {
                painter.image(tex.id(), area, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), col);
            }
        }
    }
    app.ui.timeline.onion_cache.retain(|f, _| *f < dur);
}

/// (id, name, visible, per-frame has-content) of every video layer, top first.
fn tracks(layers: &[photocraft_doc::Layer], out: &mut Vec<(u64, String, bool, Vec<bool>)>) {
    for l in layers.iter().rev() {
        if let Some(v) = &l.video {
            out.push((l.id.0, l.name.clone(), l.visible, v.frames.iter().map(|f| f.tile_count() > 0).collect()));
        }
        if let Some(ch) = l.children() {
            tracks(ch, out);
        }
    }
}

/// Menu checkmark for Window › Timeline.
pub fn checked(app: &PhotocraftApp, id: &str) -> Option<bool> {
    (id == "window.panel.timeline").then_some(app.ui.timeline.open)
}

/// Ids this module owns (for parity / enablement).
pub fn handles(id: &str) -> bool {
    id == "window.panel.timeline"
}

/// Toggle the Timeline panel.
pub fn menu(app: &mut PhotocraftApp, id: &str, _params: &Value) -> Option<Result<Value, String>> {
    if id == "window.panel.timeline" {
        app.ui.timeline.open = !app.ui.timeline.open;
        return Some(Ok(json!({ "open": app.ui.timeline.open })));
    }
    None
}

/// Render the Timeline panel.
pub fn windows(app: &mut PhotocraftApp, ctx: &egui::Context) {
    if !app.ui.timeline.open {
        return;
    }
    let tl = app.session.active().and_then(|d| d.doc.timeline.clone());
    let have_doc = app.session.active().is_some();
    let mut act: Option<(&str, Value)> = None;
    let mut close = false;
    let mut toggle_play = false;
    let mut render = false;
    let playing = app.ui.timeline.playing;
    let mut fmt = if app.ui.timeline.fmt.is_empty() { "avi".to_string() } else { app.ui.timeline.fmt.clone() };
    let t = Tokens::get(ctx);
    let (mut thumbs_on, mut onion_on, mut tracks_on) = (app.ui.timeline.thumbs, app.ui.timeline.onion, app.ui.timeline.tracks);
    let (mut ob, mut oa, mut oo, mut ot) = (app.ui.timeline.onion_before, app.ui.timeline.onion_after, app.ui.timeline.onion_opacity, app.ui.timeline.onion_tint);
    let mut thumbs: std::collections::HashMap<usize, egui::TextureHandle> = Default::default();
    let mut trk = Vec::new();
    let (mut active_layer, mut aspect) = (None, 1.0f32);
    if let (Some(st), Some(tlv)) = (app.session.active(), &tl) {
        tracks(&st.doc.layers, &mut trk);
        active_layer = st.active_layer.map(|l| l.0);
        let b = st.doc.bounds();
        aspect = (b.width().max(1) as f32 / b.height().max(1) as f32).clamp(0.4, 3.0);
        if thumbs_on {
            let (cw, ch) = (((34.0 * aspect).round() as u32).clamp(24, 96), 34u32);
            let mut order: Vec<usize> = (0..tlv.duration).collect();
            order.sort_by_key(|f| f.abs_diff(tlv.current));
            order.truncate(300);
            let mut budget = if playing { 0 } else { 3 };
            for f in order {
                let fresh = app.ui.timeline.thumb_cache.get(&f).is_some_and(|(k, _)| *k == (photocraft_engine::frame_key(&app.session, f) ^ (u64::from(cw) << 32 | u64::from(ch))));
                if !fresh {
                    if budget == 0 {
                        if let Some((_, tx)) = app.ui.timeline.thumb_cache.get(&f) {
                            thumbs.insert(f, tx.clone());
                        }
                        continue;
                    }
                    budget -= 1;
                    ctx.request_repaint();
                }
                if let Some(tx) = preview(&mut app.ui.timeline.thumb_cache, ctx, &app.session, f, cw, ch) {
                    thumbs.insert(f, tx);
                }
            }
            app.ui.timeline.thumb_cache.retain(|f, _| *f < tlv.duration);
        }
    }
    let cell_w = if thumbs_on { ((34.0 * aspect).round() as f32).clamp(24.0, 96.0) } else { 44.0 };

    crate::analysis_ui::panel_window(app, ctx, "timeline", tl!("Timeline"), vec2(0.0, 520.0), 660.0, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(tl!("Timeline")).strong().color(t.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::icons::button(ui, "x", 20.0, false, tl!("Close")).clicked() {
                    close = true;
                }
            });
        });
        ui.separator();

        if !have_doc {
            ui.weak(tl!("Open a document to use the timeline."));
            return;
        }
        let Some(tl) = &tl else {
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                if crate::widgets::primary_button(ui, tl!("Create Frame Animation"), 200.0).clicked() {
                    act = Some(("timeline.newFrame", json!({})));
                }
            });
            return;
        };
        ui.horizontal(|ui| {
            if crate::icons::button(ui, "chevrons-left", 22.0, false, tl!("First frame")).clicked() {
                act = Some(("timeline.setFrame", json!({ "frame": 0 })));
            }
            if crate::icons::button(ui, "chevron-left", 22.0, false, tl!("Previous frame")).clicked() {
                act = Some(("timeline.previousFrame", json!({})));
            }
            let play_icon = if playing { "pause" } else { "play" };
            if crate::icons::button(ui, play_icon, 22.0, playing, if playing { tl!("Pause") } else { tl!("Play") }).clicked() {
                toggle_play = true;
            }
            if crate::icons::button(ui, "chevron-right", 22.0, false, tl!("Next frame")).clicked() {
                act = Some(("timeline.nextFrame", json!({})));
            }
            if crate::icons::button(ui, "chevrons-right", 22.0, false, tl!("Last frame")).clicked() {
                act = Some(("timeline.setFrame", json!({ "frame": tl.duration.saturating_sub(1) as u64 })));
            }
            ui.separator();
            ui.label(
                RichText::new(crate::i18n::fmt(
                    crate::i18n::t("Frame {current} / {total}"),
                    &[("current", &(tl.current + 1).to_string()), ("total", &tl.duration.to_string())],
                ))
                .color(t.text)
                .monospace(),
            );
            ui.label(RichText::new(format!("{:.2}s", tl.time())).color(t.text_dim).monospace());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut fps = f64::from(tl.fps);
                if ui.add(egui::DragValue::new(&mut fps).range(0.01..=1000.0).speed(0.2).suffix(" fps")).changed() {
                    act = Some(("timeline.setProps", json!({ "fps": fps })));
                }
            });
        });
        ui.horizontal(|ui| {
            if crate::icons::button(ui, "plus", 22.0, false, tl!("New frame")).clicked() {
                act = Some(("timeline.newFrame", json!({})));
            }
            if crate::icons::button(ui, "copy", 22.0, false, tl!("Duplicate frame")).clicked() {
                act = Some(("timeline.duplicateFrame", json!({})));
            }
            if crate::icons::button(ui, "trash", 22.0, false, tl!("Delete frame")).clicked() {
                act = Some(("timeline.deleteFrame", json!({})));
            }
            if crate::icons::button(ui, "arrow-left-right", 22.0, false, tl!("Move frame left")).clicked() {
                act = Some(("timeline.moveFrame", json!({ "delta": -1 })));
            }
            if crate::icons::button(ui, "chevron-right", 22.0, false, tl!("Move frame right")).clicked() {
                act = Some(("timeline.moveFrame", json!({ "delta": 1 })));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(tl!("Render Video…")).clicked() {
                    render = true;
                }
                egui::ComboBox::from_id_salt("tl_fmt").width(70.0).selected_text(fmt.to_uppercase()).show_ui(ui, |ui| {
                    for f in ["avi", "gif", "png"] {
                        ui.selectable_value(&mut fmt, f.to_string(), f.to_uppercase());
                    }
                });
            });
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut thumbs_on, tl!("Thumbnails"));
            ui.checkbox(&mut tracks_on, tl!("Layers"));
            ui.checkbox(&mut onion_on, tl!("Onion skin"));
            if onion_on {
                ui.label(tl!("Before"));
                ui.add(egui::DragValue::new(&mut ob).range(0..=6));
                ui.label(tl!("After"));
                ui.add(egui::DragValue::new(&mut oa).range(0..=6));
                ui.add(egui::Slider::new(&mut oo, 0.05..=0.9).show_value(false).text(tl!("Opacity")));
                ui.checkbox(&mut ot, tl!("Tint"));
            }
        });
        ui.add_space(4.0);
        let (th, rh) = (34.0, 14.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            if tracks_on {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.set_width(110.0);
                    ui.add_space(th);
                    for (id, name, vis, _) in &trk {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            let (r, e) = ui.allocate_exact_size(vec2(rh, rh), egui::Sense::click());
                            let c = if *vis { t.text } else { t.text_dim.gamma_multiply(0.4) };
                            ui.painter().circle_filled(r.center(), 4.0, c);
                            if e.clicked() {
                                act = Some((if *vis { "layer.hideLayers" } else { "layer.showLayers" }, json!({ "layer": id })));
                            }
                            let txt = RichText::new(name).size(11.0).color(if Some(*id) == active_layer { t.text } else { t.text_dim });
                            if ui.add(egui::Label::new(txt).truncate().sense(egui::Sense::click())).clicked() {
                                act = Some(("layer.select", json!({ "layer": id })));
                            }
                        });
                    }
                });
            }
            egui::ScrollArea::horizontal().id_salt("tl_strip").show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for i in 0..tl.duration {
                            let cur = i == tl.current;
                            let (rect, resp) = ui.allocate_exact_size(vec2(cell_w, th), egui::Sense::click());
                            if !ui.is_rect_visible(rect) {
                                continue;
                            }
                            let fill = if resp.hovered() { t.text_dim.gamma_multiply(0.2) } else { t.text_dim.gamma_multiply(0.08) };
                            ui.painter().rect_filled(rect, 3.0, fill);
                            match thumbs.get(&i).filter(|_| thumbs_on) {
                                Some(tx) => {
                                    ui.painter().image(tx.id(), rect.shrink(1.0), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                                    let tag = egui::Rect::from_min_size(rect.min, vec2(14.0, 11.0));
                                    ui.painter().rect_filled(tag, 2.0, egui::Color32::from_black_alpha(150));
                                    ui.painter().text(tag.center(), egui::Align2::CENTER_CENTER, (i + 1).to_string(), egui::FontId::monospace(9.0), egui::Color32::WHITE);
                                }
                                None => {
                                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, (i + 1).to_string(), egui::FontId::monospace(12.0), t.text);
                                }
                            }
                            if cur {
                                ui.painter().rect_stroke(rect, 3.0, egui::Stroke::new(2.0, t.text), egui::StrokeKind::Inside);
                                resp.scroll_to_me(None);
                            }
                            if resp.clicked() {
                                act = Some(("timeline.setFrame", json!({ "frame": i as u64 })));
                            }
                        }
                    });
                    if tracks_on {
                        for (id, _, _, has) in &trk {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                for i in 0..tl.duration {
                                    let (rect, resp) = ui.allocate_exact_size(vec2(cell_w, rh), egui::Sense::click());
                                    if !ui.is_rect_visible(rect) {
                                        continue;
                                    }
                                    let on = has.get(i).copied().unwrap_or(false);
                                    let mut c = if on { t.accent_soft } else { t.text_dim.gamma_multiply(0.06) };
                                    if i == tl.current {
                                        c = c.gamma_multiply(1.6);
                                    }
                                    ui.painter().rect_filled(rect.shrink2(vec2(0.0, 1.0)), 2.0, c);
                                    if Some(*id) == active_layer && i == tl.current {
                                        ui.painter().rect_stroke(rect.shrink2(vec2(0.0, 1.0)), 2.0, egui::Stroke::new(1.0, t.text), egui::StrokeKind::Inside);
                                    }
                                    if resp.clicked() {
                                        act = Some(("timeline.setFrame", json!({ "frame": i as u64 })));
                                    }
                                }
                            });
                        }
                    }
                });
            });
        });
    });

    let u = &mut app.ui.timeline;
    (u.thumbs, u.onion, u.tracks, u.onion_before, u.onion_after, u.onion_opacity, u.onion_tint) = (thumbs_on, onion_on, tracks_on, ob, oa, oo, ot);
    app.ui.timeline.fmt = fmt.clone();
    if close {
        app.ui.timeline.open = false;
        app.ui.timeline.playing = false;
    }
    if toggle_play {
        app.ui.timeline.playing = !app.ui.timeline.playing;
        app.ui.timeline.last_time = ctx.input(|i| i.time);
    }
    if render {
        let _ = crate::menus::invoke(app, ctx, "file.export.renderVideo", json!({ "format": fmt }));
    }
    if let Some((cmd, p)) = act {
        let _ = app.run(cmd, p);
    }
    // Playback: auto-advance the playhead at the frame rate (loops).
    if app.ui.timeline.playing {
        if let Some(t) = app.session.active().and_then(|d| d.doc.timeline.clone()) {
            let now = ctx.input(|i| i.time);
            let period = 1.0 / f64::from(t.fps.max(0.01));
            if now - app.ui.timeline.last_time >= period {
                app.ui.timeline.last_time = now;
                let next = (t.current + 1) % t.duration.max(1);
                let _ = app.run("timeline.setFrame", json!({ "frame": next as u64 }));
            }
            ctx.request_repaint();
        } else {
            app.ui.timeline.playing = false;
        }
    }
}
