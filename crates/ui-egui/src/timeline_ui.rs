//! Window › Timeline: a floating panel that creates and drives the document timeline. All actions
//! run the engine `timeline.*` commands, so the control channel drives it the same way.

use egui::{RichText, vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::theme::Tokens;

/// Timeline panel view state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
        ui.add_space(4.0);
        egui::ScrollArea::horizontal().id_salt("tl_strip").show(ui, |ui| {
            ui.horizontal(|ui| {
                for i in 0..tl.duration {
                    let cur = i == tl.current;
                    let (rect, resp) = ui.allocate_exact_size(vec2(44.0, 34.0), egui::Sense::click());
                    let fill = if cur { t.accent_soft } else if resp.hovered() { t.text_dim.gamma_multiply(0.15) } else { t.text_dim.gamma_multiply(0.07) };
                    ui.painter().rect_filled(rect, 3.0, fill);
                    if cur {
                        ui.painter().rect_stroke(rect, 3.0, egui::Stroke::new(1.5, t.text), egui::StrokeKind::Inside);
                        resp.scroll_to_me(None);
                    }
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, (i + 1).to_string(), egui::FontId::monospace(12.0), t.text);
                    if resp.clicked() {
                        act = Some(("timeline.setFrame", json!({ "frame": i as u64 })));
                    }
                }
            });
        });
    });

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
