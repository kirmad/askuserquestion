use clap::Parser;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::mpsc;

#[derive(Parser, Debug)]
#[command(name = "ask-user")]
struct Args {
    #[arg(short, long)]
    input: String,
}

#[derive(Deserialize, Debug, Clone)]
struct QuestionOption {
    label: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Question {
    question: String,
    #[serde(default)]
    header: String,
    options: Vec<QuestionOption>,
    #[serde(default, rename = "multiSelect")]
    multi_select: bool,
}

#[derive(Deserialize, Debug)]
struct InputData {
    questions: Vec<Question>,
}

#[derive(Serialize, Clone)]
struct QuestionAnswer {
    question: String,
    header: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_index: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct Response {
    status: String,
    answers: Vec<QuestionAnswer>,
}

#[derive(Clone)]
struct AnswerData {
    question: String,
    header: String,
    selected: Vec<String>,
    indices: Vec<i32>,
    multi: bool,
}

struct Theme {
    // Base colors
    bg: egui::Color32,

    // Surface colors
    surface: egui::Color32,
    surface_hover: egui::Color32,
    surface_active: egui::Color32,

    // Text colors
    text_primary: egui::Color32,
    text_secondary: egui::Color32,
    text_muted: egui::Color32,
    text_inverse: egui::Color32,

    // Accent colors
    accent: egui::Color32,
    accent_hover: egui::Color32,
    accent_muted: egui::Color32,

    // Status colors
    success: egui::Color32,
    success_muted: egui::Color32,

    // Border
    border: egui::Color32,
    border_subtle: egui::Color32,
}

impl Theme {
    fn new() -> Self {
        Self {
            // Deep, rich background
            bg: egui::Color32::from_rgb(8, 8, 12),

            // Elevated surfaces
            surface: egui::Color32::from_rgb(18, 18, 24),
            surface_hover: egui::Color32::from_rgb(26, 26, 34),
            surface_active: egui::Color32::from_rgb(32, 32, 44),

            // Text hierarchy
            text_primary: egui::Color32::from_rgb(248, 248, 252),
            text_secondary: egui::Color32::from_rgb(180, 180, 195),
            text_muted: egui::Color32::from_rgb(100, 100, 120),
            text_inverse: egui::Color32::from_rgb(8, 8, 12),

            // Primary accent - sophisticated blue-violet
            accent: egui::Color32::from_rgb(99, 102, 241),
            accent_hover: egui::Color32::from_rgb(129, 132, 255),
            accent_muted: egui::Color32::from_rgb(55, 58, 110),

            // Success - refined green
            success: egui::Color32::from_rgb(74, 222, 128),
            success_muted: egui::Color32::from_rgb(30, 70, 50),

            // Borders
            border: egui::Color32::from_rgb(45, 45, 60),
            border_subtle: egui::Color32::from_rgb(30, 30, 42),
        }
    }
}

#[derive(Clone)]
enum Screen {
    Question(usize),
    Review,
}

struct HoverState {
    target: f32,
    current: f32,
}

impl HoverState {
    fn new() -> Self {
        Self { target: 0.0, current: 0.0 }
    }

    fn update(&mut self, hovered: bool, dt: f32) {
        self.target = if hovered { 1.0 } else { 0.0 };
        let speed = 12.0;
        self.current += (self.target - self.current) * (speed * dt).min(1.0);
    }

    fn value(&self) -> f32 {
        self.current
    }
}

struct App {
    questions: Vec<Question>,
    screen: Screen,
    answers: Vec<AnswerData>,
    selected: Vec<bool>,
    hover_states: Vec<HoverState>,
    custom: String,
    custom_focused: bool,
    tx: mpsc::Sender<Option<Vec<QuestionAnswer>>>,
    theme: Theme,
    transition_progress: f32,
}

impl App {
    fn new(questions: Vec<Question>, tx: mpsc::Sender<Option<Vec<QuestionAnswer>>>) -> Self {
        let n = questions.first().map(|q| q.options.len()).unwrap_or(0);
        Self {
            questions,
            screen: Screen::Question(0),
            answers: Vec::new(),
            selected: vec![false; n],
            hover_states: (0..n + 1).map(|_| HoverState::new()).collect(),
            custom: String::new(),
            custom_focused: false,
            tx,
            theme: Theme::new(),
            transition_progress: 0.0,
        }
    }

    fn current_q(&self) -> Option<&Question> {
        if let Screen::Question(idx) = self.screen {
            self.questions.get(idx)
        } else {
            None
        }
    }

    fn idx(&self) -> usize {
        if let Screen::Question(idx) = self.screen { idx } else { self.questions.len() }
    }

    fn get_selection(&self) -> (Vec<String>, Vec<i32>) {
        let q = match self.current_q() {
            Some(q) => q,
            None => return (vec![], vec![]),
        };

        let mut labels = Vec::new();
        let mut indices = Vec::new();

        for (i, opt) in q.options.iter().enumerate() {
            if self.selected.get(i).copied().unwrap_or(false) {
                labels.push(opt.label.clone());
                indices.push(i as i32);
            }
        }

        if !self.custom.trim().is_empty() {
            labels.push(self.custom.trim().to_string());
            indices.push(-1);
        }

        (labels, indices)
    }

    fn has_selection(&self) -> bool {
        self.selected.iter().any(|&s| s) || !self.custom.trim().is_empty()
    }

    fn save_answer(&mut self) {
        if let Some(q) = self.current_q().cloned() {
            let (labels, indices) = self.get_selection();
            if !labels.is_empty() {
                self.answers.push(AnswerData {
                    question: q.question,
                    header: q.header,
                    selected: labels,
                    indices,
                    multi: q.multi_select,
                });
            }
        }
    }

    fn go_next(&mut self) {
        self.save_answer();
        let next_idx = self.idx() + 1;
        if next_idx < self.questions.len() {
            self.screen = Screen::Question(next_idx);
            let n = self.questions[next_idx].options.len();
            self.selected = vec![false; n];
            self.hover_states = (0..n + 1).map(|_| HoverState::new()).collect();
            self.custom.clear();
            self.transition_progress = 0.0;
        } else {
            self.screen = Screen::Review;
            self.transition_progress = 0.0;
        }
    }

    fn go_back(&mut self) {
        match self.screen {
            Screen::Question(idx) if idx > 0 => {
                self.answers.pop();
                self.screen = Screen::Question(idx - 1);
                let n = self.questions[idx - 1].options.len();
                self.selected = vec![false; n];
                self.hover_states = (0..n + 1).map(|_| HoverState::new()).collect();
                self.custom.clear();
                self.transition_progress = 0.0;
            }
            Screen::Review => {
                self.answers.pop();
                let last = self.questions.len() - 1;
                self.screen = Screen::Question(last);
                let n = self.questions[last].options.len();
                self.selected = vec![false; n];
                self.hover_states = (0..n + 1).map(|_| HoverState::new()).collect();
                self.custom.clear();
                self.transition_progress = 0.0;
            }
            _ => {}
        }
    }

    fn submit(&mut self, ctx: &egui::Context) {
        let answers: Vec<QuestionAnswer> = self.answers.iter().map(|a| {
            if a.multi {
                QuestionAnswer {
                    question: a.question.clone(),
                    header: a.header.clone(),
                    selected: Some(serde_json::json!(a.selected)),
                    selected_index: Some(serde_json::json!(a.indices)),
                }
            } else {
                QuestionAnswer {
                    question: a.question.clone(),
                    header: a.header.clone(),
                    selected: a.selected.first().map(|s| serde_json::json!(s)),
                    selected_index: a.indices.first().map(|i| serde_json::json!(i)),
                }
            }
        }).collect();

        let _ = self.tx.send(Some(answers));
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn cancel(&self, ctx: &egui::Context) {
        let _ = self.tx.send(None);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
        let t = t.clamp(0.0, 1.0);
        egui::Color32::from_rgba_unmultiplied(
            (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
            (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
            (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
            (a.a() as f32 + (b.a() as f32 - a.a() as f32) * t) as u8,
        )
    }

    fn render_question(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, q: Question, idx: usize) {
        let total = self.questions.len();
        let dt = ctx.input(|i| i.stable_dt);

        // Update transition
        self.transition_progress = (self.transition_progress + dt * 4.0).min(1.0);
        let fade = ease_out_cubic(self.transition_progress);

        // Top bar with header and step indicator
        ui.horizontal(|ui| {
            if !q.header.is_empty() {
                egui::Frame::new()
                    .fill(self.theme.accent_muted)
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(8, 3))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&q.header)
                            .color(self.theme.accent_hover)
                            .size(10.0)
                            .strong());
                    });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("{} of {}", idx + 1, total))
                    .color(self.theme.text_muted)
                    .size(11.0));
            });
        });

        ui.add_space(16.0);

        // Progress bar - elegant segmented design
        let avail_w = ui.available_width();
        let segment_gap = 4.0;
        let segment_w = (avail_w - (total as f32 - 1.0) * segment_gap) / total as f32;

        ui.horizontal(|ui| {
            for i in 0..total {
                let progress = if i < idx { 1.0 }
                              else if i == idx { 0.4 + 0.6 * fade }
                              else { 0.0 };

                let color = if i < idx {
                    self.theme.success
                } else if i == idx {
                    self.theme.accent
                } else {
                    self.theme.border_subtle
                };

                let (rect, _) = ui.allocate_exact_size(egui::vec2(segment_w, 3.0), egui::Sense::hover());

                // Background
                ui.painter().rect_filled(rect, 2.0, self.theme.border_subtle);

                // Fill
                if progress > 0.0 {
                    let fill_rect = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(segment_w * progress, 3.0)
                    );
                    ui.painter().rect_filled(fill_rect, 2.0, color);
                }

                if i < total - 1 { ui.add_space(segment_gap); }
            }
        });

        ui.add_space(24.0);

        // Question text with fade animation
        let alpha = (fade * 255.0) as u8;
        ui.label(egui::RichText::new(&q.question)
            .color(egui::Color32::from_rgba_unmultiplied(
                self.theme.text_primary.r(),
                self.theme.text_primary.g(),
                self.theme.text_primary.b(),
                alpha
            ))
            .size(17.0));

        if q.multi_select {
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Select all that apply")
                .color(self.theme.text_muted)
                .size(11.0)
                .italics());
        }

        ui.add_space(18.0);

        // Options with smooth hover animations
        egui::ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;

                for (i, opt) in q.options.iter().enumerate() {
                    let sel = self.selected.get(i).copied().unwrap_or(false);
                    let id = ui.id().with(("opt", i));
                    let resp = ui.interact(
                        ui.cursor(),
                        id.with("sense"),
                        egui::Sense::hover()
                    );

                    if let Some(state) = self.hover_states.get_mut(i) {
                        state.update(resp.hovered(), dt);
                    }
                    let hover_t = self.hover_states.get(i).map(|s| s.value()).unwrap_or(0.0);

                    // Compute colors based on state
                    let bg = if sel {
                        Self::lerp_color(self.theme.surface_active, self.theme.accent_muted, 0.3)
                    } else {
                        Self::lerp_color(self.theme.surface, self.theme.surface_hover, hover_t)
                    };

                    let border_color = if sel {
                        Self::lerp_color(self.theme.accent, self.theme.accent_hover, hover_t)
                    } else {
                        Self::lerp_color(self.theme.border_subtle, self.theme.border, hover_t)
                    };

                    let resp = egui::Frame::new()
                        .fill(bg)
                        .stroke(egui::Stroke::new(1.0, border_color))
                        .corner_radius(10)
                        .inner_margin(egui::Margin::symmetric(14, 12))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                self.draw_indicator(ui, sel, q.multi_select, hover_t);
                                ui.add_space(12.0);
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    let text_color = if sel {
                                        self.theme.text_primary
                                    } else {
                                        Self::lerp_color(self.theme.text_secondary, self.theme.text_primary, hover_t)
                                    };
                                    ui.label(egui::RichText::new(&opt.label)
                                        .color(text_color)
                                        .size(13.0));
                                    if !opt.description.is_empty() {
                                        ui.label(egui::RichText::new(&opt.description)
                                            .color(self.theme.text_muted)
                                            .size(11.0));
                                    }
                                });
                            });
                        });

                    let click_resp = ui.interact(resp.response.rect, id, egui::Sense::click());
                    if click_resp.clicked() {
                        if q.multi_select {
                            if let Some(s) = self.selected.get_mut(i) { *s = !*s; }
                        } else {
                            self.selected.iter_mut().enumerate().for_each(|(j, s)| *s = j == i);
                            self.custom.clear();
                        }
                    }
                }
            });

        ui.add_space(10.0);

        // Custom input with refined styling
        let custom_idx = q.options.len();
        let custom_hover_t = self.hover_states.get(custom_idx).map(|s| s.value()).unwrap_or(0.0);
        let has_custom = !self.custom.trim().is_empty();

        let custom_bg = if has_custom || self.custom_focused {
            Self::lerp_color(self.theme.surface_active, self.theme.accent_muted, 0.2)
        } else {
            Self::lerp_color(self.theme.surface, self.theme.surface_hover, custom_hover_t)
        };

        let custom_border = if has_custom || self.custom_focused {
            self.theme.accent
        } else {
            Self::lerp_color(self.theme.border_subtle, self.theme.border, custom_hover_t)
        };

        egui::Frame::new()
            .fill(custom_bg)
            .stroke(egui::Stroke::new(1.0, custom_border))
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if q.multi_select {
                        self.draw_indicator(ui, has_custom, true, custom_hover_t);
                        ui.add_space(12.0);
                    }

                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        ui.label(egui::RichText::new("Other")
                            .color(if has_custom { self.theme.text_primary } else { self.theme.text_secondary })
                            .size(13.0));

                        let te = egui::TextEdit::singleline(&mut self.custom)
                            .hint_text("Type a custom response...")
                            .desired_width(ui.available_width())
                            .text_color(self.theme.text_primary)
                            .frame(false);
                        let te_resp = ui.add(te);
                        self.custom_focused = te_resp.has_focus();

                        // Clear predefined selection when typing custom (single select)
                        if !q.multi_select && te_resp.changed() && !self.custom.is_empty() {
                            self.selected.iter_mut().for_each(|s| *s = false);
                        }
                    });
                });
            });

        // Update custom hover state
        if let Some(state) = self.hover_states.get_mut(custom_idx) {
            state.update(self.custom_focused || has_custom, dt);
        }

        ui.add_space(20.0);

        // Footer buttons
        ui.horizontal(|ui| {
            // Back/Cancel button
            let back_text = if idx > 0 { "Back" } else { "Cancel" };
            let back_resp = ui.add(
                egui::Button::new(egui::RichText::new(back_text).color(self.theme.text_muted).size(12.0))
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE)
                    .min_size(egui::vec2(70.0, 38.0))
            );
            if back_resp.clicked() {
                if idx > 0 { self.go_back(); } else { self.cancel(ctx); }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let has = self.has_selection();
                let is_last = idx == total - 1;
                let txt = if is_last { "Review" } else { "Continue" };

                let btn_color = if has { self.theme.accent } else { self.theme.surface_hover };
                let text_color = if has { self.theme.text_inverse } else { self.theme.text_muted };

                // Button with custom arrow icon
                let btn_resp = egui::Frame::new()
                    .fill(btn_color)
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(16, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(txt).color(text_color).size(12.0));
                            ui.add_space(6.0);
                            // Draw arrow icon
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                            let c = rect.center();
                            let stroke = egui::Stroke::new(1.5, text_color);
                            ui.painter().line_segment([c + egui::vec2(-4.0, 0.0), c + egui::vec2(3.0, 0.0)], stroke);
                            ui.painter().line_segment([c + egui::vec2(0.0, -3.0), c + egui::vec2(3.0, 0.0)], stroke);
                            ui.painter().line_segment([c + egui::vec2(0.0, 3.0), c + egui::vec2(3.0, 0.0)], stroke);
                        });
                    });

                let btn_click = ui.interact(btn_resp.response.rect, ui.id().with("next_btn"), egui::Sense::click());
                if has && btn_click.clicked() {
                    self.go_next();
                }
            });
        });
    }

    fn render_review(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let dt = ctx.input(|i| i.stable_dt);
        self.transition_progress = (self.transition_progress + dt * 4.0).min(1.0);
        let fade = ease_out_cubic(self.transition_progress);

        // Header
        ui.horizontal(|ui| {
            egui::Frame::new()
                .fill(self.theme.success_muted)
                .corner_radius(4)
                .inner_margin(egui::Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        // Draw small checkmark
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                        let c = rect.center();
                        let stroke = egui::Stroke::new(1.5, self.theme.success);
                        ui.painter().line_segment([c + egui::vec2(-3.0, 0.0), c + egui::vec2(-1.0, 2.0)], stroke);
                        ui.painter().line_segment([c + egui::vec2(-1.0, 2.0), c + egui::vec2(3.0, -2.0)], stroke);
                        ui.label(egui::RichText::new("Complete")
                            .color(self.theme.success)
                            .size(10.0)
                            .strong());
                    });
                });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("{} questions", self.answers.len()))
                    .color(self.theme.text_muted)
                    .size(11.0));
            });
        });

        ui.add_space(16.0);

        // Full progress bar with success color
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 3.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, self.theme.success);

        ui.add_space(24.0);

        // Title
        let alpha = (fade * 255.0) as u8;
        ui.label(egui::RichText::new("Review your answers")
            .color(egui::Color32::from_rgba_unmultiplied(
                self.theme.text_primary.r(),
                self.theme.text_primary.g(),
                self.theme.text_primary.b(),
                alpha
            ))
            .size(17.0));

        ui.add_space(4.0);
        ui.label(egui::RichText::new("Make sure everything looks right before submitting")
            .color(self.theme.text_muted)
            .size(11.0));

        ui.add_space(18.0);

        // Answers list
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;

                for (i, answer) in self.answers.iter().enumerate() {
                    let delay = i as f32 * 0.08;
                    let item_fade = ((self.transition_progress - delay).max(0.0) * 5.0).min(1.0);
                    let item_alpha = (ease_out_cubic(item_fade) * 255.0) as u8;

                    egui::Frame::new()
                        .fill(self.theme.surface)
                        .stroke(egui::Stroke::new(1.0, self.theme.border_subtle))
                        .corner_radius(10)
                        .inner_margin(egui::Margin::symmetric(14, 12))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());

                            // Question indicator
                            ui.horizontal(|ui| {
                                egui::Frame::new()
                                    .fill(egui::Color32::from_rgba_unmultiplied(
                                        self.theme.accent_muted.r(),
                                        self.theme.accent_muted.g(),
                                        self.theme.accent_muted.b(),
                                        item_alpha
                                    ))
                                    .corner_radius(4)
                                    .inner_margin(egui::Margin::symmetric(6, 2))
                                    .show(ui, |ui| {
                                        ui.label(egui::RichText::new(format!("Q{}", i + 1))
                                            .color(egui::Color32::from_rgba_unmultiplied(
                                                self.theme.accent.r(),
                                                self.theme.accent.g(),
                                                self.theme.accent.b(),
                                                item_alpha
                                            ))
                                            .size(10.0));
                                    });

                                if !answer.header.is_empty() {
                                    ui.label(egui::RichText::new(&answer.header)
                                        .color(egui::Color32::from_rgba_unmultiplied(
                                            self.theme.text_muted.r(),
                                            self.theme.text_muted.g(),
                                            self.theme.text_muted.b(),
                                            item_alpha
                                        ))
                                        .size(10.0));
                                }
                            });

                            ui.add_space(8.0);

                            // Selected values as chips
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                for sel in &answer.selected {
                                    egui::Frame::new()
                                        .fill(egui::Color32::from_rgba_unmultiplied(
                                            self.theme.success_muted.r(),
                                            self.theme.success_muted.g(),
                                            self.theme.success_muted.b(),
                                            item_alpha
                                        ))
                                        .corner_radius(6)
                                        .inner_margin(egui::Margin::symmetric(10, 5))
                                        .show(ui, |ui| {
                                            ui.label(egui::RichText::new(sel)
                                                .color(egui::Color32::from_rgba_unmultiplied(
                                                    self.theme.success.r(),
                                                    self.theme.success.g(),
                                                    self.theme.success.b(),
                                                    item_alpha
                                                ))
                                                .size(12.0));
                                        });
                                }
                            });
                        });
                }
            });

        ui.add_space(20.0);

        // Footer buttons
        ui.horizontal(|ui| {
            let back_resp = ui.add(
                egui::Button::new(egui::RichText::new("Edit").color(self.theme.text_muted).size(12.0))
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE)
                    .min_size(egui::vec2(70.0, 38.0))
            );
            if back_resp.clicked() {
                self.go_back();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Submit button with custom checkmark icon
                let btn_resp = egui::Frame::new()
                    .fill(self.theme.success)
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(16, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Submit").color(self.theme.text_inverse).size(12.0));
                            ui.add_space(6.0);
                            // Draw checkmark icon
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                            let c = rect.center();
                            let stroke = egui::Stroke::new(1.8, self.theme.text_inverse);
                            ui.painter().line_segment([c + egui::vec2(-4.0, 0.0), c + egui::vec2(-1.0, 3.0)], stroke);
                            ui.painter().line_segment([c + egui::vec2(-1.0, 3.0), c + egui::vec2(4.0, -3.0)], stroke);
                        });
                    });

                let btn_click = ui.interact(btn_resp.response.rect, ui.id().with("submit_btn"), egui::Sense::click());
                if btn_click.clicked() {
                    self.submit(ctx);
                }
            });
        });
    }

    fn draw_indicator(&self, ui: &mut egui::Ui, selected: bool, is_checkbox: bool, hover_t: f32) {
        let sz = 18.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(sz, sz), egui::Sense::hover());
        let c = rect.center();

        if is_checkbox {
            let rounding = 5.0;
            if selected {
                ui.painter().rect_filled(rect.shrink(1.0), rounding, self.theme.accent);
                // Animated checkmark
                let check_progress = 1.0;
                ui.painter().line_segment(
                    [c + egui::vec2(-4.0 * check_progress, 0.0), c + egui::vec2(-1.5 * check_progress, 3.0 * check_progress)],
                    egui::Stroke::new(2.0, self.theme.text_inverse));
                ui.painter().line_segment(
                    [c + egui::vec2(-1.5 * check_progress, 3.0 * check_progress), c + egui::vec2(4.0 * check_progress, -3.0 * check_progress)],
                    egui::Stroke::new(2.0, self.theme.text_inverse));
            } else {
                let border_color = Self::lerp_color(self.theme.border, self.theme.text_secondary, hover_t);
                ui.painter().rect_stroke(
                    rect.shrink(1.0),
                    rounding,
                    egui::Stroke::new(1.5, border_color),
                    egui::StrokeKind::Inside
                );
            }
        } else {
            let r = sz / 2.0 - 2.0;
            if selected {
                ui.painter().circle_stroke(c, r, egui::Stroke::new(2.0, self.theme.accent));
                ui.painter().circle_filled(c, r - 4.0, self.theme.accent);
            } else {
                let border_color = Self::lerp_color(self.theme.border, self.theme.text_secondary, hover_t);
                ui.painter().circle_stroke(c, r, egui::Stroke::new(1.5, border_color));
            }
        }
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous repainting for smooth animations
        ctx.request_repaint();

        let mut v = egui::Visuals::dark();
        v.panel_fill = self.theme.bg;
        v.window_fill = self.theme.bg;
        v.widgets.noninteractive.bg_fill = self.theme.surface;
        v.widgets.inactive.bg_fill = self.theme.surface;
        v.widgets.hovered.bg_fill = self.theme.surface_hover;
        v.widgets.active.bg_fill = self.theme.accent;
        v.selection.bg_fill = self.theme.accent.linear_multiply(0.15);
        v.selection.stroke = egui::Stroke::new(1.0, self.theme.accent);

        // Customize text cursor
        v.text_cursor = egui::style::TextCursorStyle {
            stroke: egui::Stroke::new(2.0, self.theme.accent),
            ..Default::default()
        };

        ctx.set_visuals(v);

        egui::CentralPanel::default()
            .frame(egui::Frame::new()
                .fill(self.theme.bg)
                .inner_margin(egui::Margin::symmetric(28, 24)))
            .show(ctx, |ui| {
                match self.screen.clone() {
                    Screen::Question(idx) => {
                        if let Some(q) = self.questions.get(idx).cloned() {
                            self.render_question(ui, ctx, q, idx);
                        }
                    }
                    Screen::Review => {
                        self.render_review(ui, ctx);
                    }
                }
            });

        // Keyboard shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && !self.custom_focused {
            match &self.screen {
                Screen::Question(_) if self.has_selection() => self.go_next(),
                Screen::Review => self.submit(ctx),
                _ => {}
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            match &self.screen {
                Screen::Question(0) => self.cancel(ctx),
                _ => self.go_back(),
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    let args = Args::parse();

    let content = fs::read_to_string(&args.input).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });

    let input: InputData = serde_json::from_str(&content).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });

    if input.questions.is_empty() {
        eprintln!("No questions");
        std::process::exit(1);
    }

    let (tx, rx) = mpsc::channel();

    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([420.0, 520.0])
            .with_min_inner_size([360.0, 420.0])
            .with_title("")
            .with_decorations(true)
            .with_transparent(false)
            .with_always_on_top(),
        centered: true,
        ..Default::default()
    };

    let questions = input.questions;

    eframe::run_native("ask-user", opts, Box::new(move |_| {
        Ok(Box::new(App::new(questions.clone(), tx.clone())))
    }))?;

    let result = rx.recv().ok().flatten();
    println!("{}", serde_json::to_string(&Response {
        status: if result.is_some() { "selected" } else { "cancelled" }.into(),
        answers: result.unwrap_or_default(),
    }).unwrap());

    Ok(())
}
