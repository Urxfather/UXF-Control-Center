use eframe::egui::{self, Color32, RichText};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FanMode {
    Auto,
    Max,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AnimationMode {
    Off,
    Breathing,
    Rainbow,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BrightnessLevel {
    Off,
    Level1,
    Level2,
    Level3,
    Level4,
}

#[allow(dead_code)]
impl BrightnessLevel {
    pub const fn to_pwm(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Level1 => 64,
            Self::Level2 => 128,
            Self::Level3 => 192,
            Self::Level4 => 255,
        }
    }

    pub const fn from_pwm(val: u8) -> Self {
        match val {
            0..=31 => Self::Off,
            32..=95 => Self::Level1,
            96..=159 => Self::Level2,
            160..=223 => Self::Level3,
            224..=255 => Self::Level4,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Level1 => "1",
            Self::Level2 => "2",
            Self::Level3 => "3",
            Self::Level4 => "4",
        }
    }
}

pub fn segmented_choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    current: T,
    options: &[(T, &str)],
) -> Option<T> {
    let mut choice = None;
    let gap = 4.0;
    let n = options.len().max(1) as f32;
    let width = ((ui.available_width() - gap * (n - 1.0)) / n).clamp(24.0, 190.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (value, label) in options {
            let selected = current == *value;
            let text = if selected {
                RichText::new(*label).strong().color(crate::theme::PRIMARY_TEXT)
            } else {
                RichText::new(*label).color(crate::theme::SECONDARY_TEXT)
            };
            let fill = if selected {
                crate::theme::SURFACE_ACTIVE
            } else {
                ui.visuals().widgets.inactive.bg_fill
            };
            let button = egui::Button::new(text)
                .selected(selected)
                .fill(fill)
                .stroke(if selected { egui::Stroke::new(1.0_f32, crate::theme::PRIMARY_ACCENT) } else { ui.visuals().widgets.inactive.bg_stroke })
                .corner_radius(crate::theme::RADIUS_MD)
                .min_size(egui::vec2(width, 34.0));
            if ui.add_sized([width, 34.0], button).clicked() {
                choice = Some(*value);
            }
        }
    });
    choice
}

pub fn sidebar_button(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, 40.0),
        egui::Sense::click(),
    );
    let fill = if selected {
        crate::theme::SURFACE_ACTIVE
    } else if response.hovered() {
        ui.visuals().widgets.hovered.bg_fill
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, crate::theme::RADIUS_MD, fill);
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.left_top(), egui::vec2(3.0, rect.height())),
            crate::theme::RADIUS_SM,
            crate::theme::PRIMARY_ACCENT,
        );
    }
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::TextStyle::Button.resolve(ui.style()).clone(),
        if selected { crate::theme::PRIMARY_TEXT } else { crate::theme::SECONDARY_TEXT },
    );
    response.clicked()
}

/// Reusable Circular Arc Progress Gauge drawn via `egui::Painter`.
///
/// Configurable:
/// - Progress value and maximum value (safely clamped; non-finite inputs handled cleanly).
/// - Size (radius) and stroke width.
/// - Start angle and sweep angle (presets for 270° tachometer and 360° ring).
/// - Background track arc and foreground progress arc with custom colors.
/// - Rounded arc end caps.
/// - Optional center text: primary value and secondary label.
/// - 100% vector graphics with zero external image assets.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct CircularArcGauge<'a> {
    pub value: f32,
    pub max_value: f32,
    pub radius: f32,
    pub stroke_width: f32,
    pub start_angle: f32,
    pub sweep_angle: f32,
    pub track_color: Color32,
    pub bar_color: Color32,
    pub center_value: Option<&'a str>,
    pub center_label: Option<&'a str>,
    pub rounded_caps: bool,
}

#[allow(dead_code)]
impl<'a> CircularArcGauge<'a> {
    /// Default tachometer-style circular arc gauge (270° sweep, neon lime accent)
    pub fn new(value: f32, max_value: f32) -> Self {
        Self::tachometer(value, max_value)
    }

    /// Tachometer preset: 270° sweep arc with bottom gap (classic automotive/hardware dial)
    pub fn tachometer(value: f32, max_value: f32) -> Self {
        Self {
            value,
            max_value,
            radius: 36.0,
            stroke_width: 6.0,
            start_angle: std::f32::consts::PI * 0.75, // 135 degrees (bottom-left)
            sweep_angle: std::f32::consts::PI * 1.5,  // 270 degrees sweep
            track_color: crate::theme::BORDER,
            bar_color: crate::theme::PRIMARY_ACCENT,
            center_value: None,
            center_label: None,
            rounded_caps: true,
        }
    }

    /// Circular ring preset: full 360° sweep starting at 12 o'clock
    pub fn ring(value: f32, max_value: f32) -> Self {
        Self {
            value,
            max_value,
            radius: 36.0,
            stroke_width: 6.0,
            start_angle: -std::f32::consts::FRAC_PI_2, // -90 degrees (top / 12 o'clock)
            sweep_angle: std::f32::consts::TAU,        // 360 degrees sweep
            track_color: crate::theme::BORDER,
            bar_color: crate::theme::PRIMARY_ACCENT,
            center_value: None,
            center_label: None,
            rounded_caps: true,
        }
    }

    /// Set gauge radius in logical points
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Set stroke width in logical points
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }

    /// Set custom start and sweep angles in radians
    pub fn angles(mut self, start_rad: f32, sweep_rad: f32) -> Self {
        self.start_angle = start_rad;
        self.sweep_angle = sweep_rad;
        self
    }

    /// Set custom start and sweep angles in degrees
    pub fn angles_deg(mut self, start_deg: f32, sweep_deg: f32) -> Self {
        self.start_angle = start_deg.to_radians();
        self.sweep_angle = sweep_deg.to_radians();
        self
    }

    /// Set foreground bar and background track colors
    pub fn colors(mut self, bar: Color32, track: Color32) -> Self {
        self.bar_color = bar;
        self.track_color = track;
        self
    }

    /// Set foreground progress bar color
    pub fn bar_color(mut self, bar: Color32) -> Self {
        self.bar_color = bar;
        self
    }

    /// Set background track color
    pub fn track_color(mut self, track: Color32) -> Self {
        self.track_color = track;
        self
    }

    /// Set center primary value string (e.g. "45%", "2470")
    pub fn center_value(mut self, value: &'a str) -> Self {
        self.center_value = Some(value);
        self
    }

    /// Set center secondary label string (e.g. "CPU", "RPM")
    pub fn center_label(mut self, label: &'a str) -> Self {
        self.center_label = Some(label);
        self
    }

    /// Set both center primary value and secondary label
    pub fn center_text(mut self, value: &'a str, label: &'a str) -> Self {
        self.center_value = Some(value);
        self.center_label = Some(label);
        self
    }

    /// Enable or disable rounded caps at the ends of the arcs
    pub fn rounded_caps(mut self, rounded: bool) -> Self {
        self.rounded_caps = rounded;
        self
    }

    /// Calculate the normalized clamped progress ratio [0.0, 1.0], safely handling non-finite values and zero/negative max
    pub fn calculate_ratio(&self) -> f32 {
        if !self.value.is_finite() || !self.max_value.is_finite() || self.max_value <= 0.0 {
            0.0
        } else {
            (self.value / self.max_value).clamp(0.0, 1.0)
        }
    }

    /// Render the circular arc gauge into the UI
    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let diameter = (self.radius + self.stroke_width * 0.5) * 2.0;
        let total_size = egui::vec2(diameter + 4.0, diameter + 4.0);
        let (rect, response) = ui.allocate_exact_size(total_size, egui::Sense::hover());

        if self.radius <= 0.0 || self.stroke_width <= 0.0 {
            return response;
        }

        let painter = ui.painter_at(rect);
        let center = rect.center();

        let ratio = self.calculate_ratio();

        let is_full_circle = (self.sweep_angle.abs() - std::f32::consts::TAU).abs() < 0.01;

        // 1. Background Track Arc
        if is_full_circle {
            painter.circle_stroke(center, self.radius, egui::Stroke::new(self.stroke_width, self.track_color));
        } else {
            let steps = ((self.radius * self.sweep_angle.abs()) / 2.5).ceil().max(12.0) as usize;
            let track_pts: Vec<egui::Pos2> = (0..=steps)
                .map(|i| {
                    let t = i as f32 / steps as f32;
                    let a = self.start_angle + t * self.sweep_angle;
                    center + egui::vec2(a.cos() * self.radius, a.sin() * self.radius)
                })
                .collect();

            if track_pts.len() >= 2 {
                painter.add(egui::Shape::Path(eframe::epaint::PathShape::line(
                    track_pts.clone(),
                    egui::Stroke::new(self.stroke_width, self.track_color),
                )));
            }
            if self.rounded_caps && !track_pts.is_empty() {
                painter.circle_filled(track_pts[0], self.stroke_width * 0.5, self.track_color);
                painter.circle_filled(*track_pts.last().unwrap(), self.stroke_width * 0.5, self.track_color);
            }
        }

        // 2. Foreground Progress Arc
        if ratio > 0.0 {
            if is_full_circle && ratio >= 1.0 {
                painter.circle_stroke(center, self.radius, egui::Stroke::new(self.stroke_width, self.bar_color));
            } else {
                let prog_sweep = self.sweep_angle * ratio;
                let prog_steps = ((self.radius * prog_sweep.abs()) / 2.5).ceil().max(2.0) as usize;
                let prog_pts: Vec<egui::Pos2> = (0..=prog_steps)
                    .map(|i| {
                        let t = i as f32 / prog_steps as f32;
                        let a = self.start_angle + t * prog_sweep;
                        center + egui::vec2(a.cos() * self.radius, a.sin() * self.radius)
                    })
                    .collect();

                if prog_pts.len() >= 2 {
                    painter.add(egui::Shape::Path(eframe::epaint::PathShape::line(
                        prog_pts.clone(),
                        egui::Stroke::new(self.stroke_width, self.bar_color),
                    )));
                }
                if self.rounded_caps && !prog_pts.is_empty() {
                    painter.circle_filled(prog_pts[0], self.stroke_width * 0.5, self.bar_color);
                    painter.circle_filled(*prog_pts.last().unwrap(), self.stroke_width * 0.5, self.bar_color);
                }
            }
        }

        // 3. Optional Center Value & Label
        match (self.center_value, self.center_label) {
            (Some(val), Some(lbl)) => {
                let val_size = (self.radius * 0.44).clamp(12.0, 32.0);
                let lbl_size = (self.radius * 0.22).clamp(9.0, 14.0);
                painter.text(
                    center - egui::vec2(0.0, val_size * 0.35),
                    egui::Align2::CENTER_CENTER,
                    val,
                    egui::FontId::proportional(val_size),
                    crate::theme::PRIMARY_TEXT,
                );
                painter.text(
                    center + egui::vec2(0.0, lbl_size * 1.1),
                    egui::Align2::CENTER_CENTER,
                    lbl,
                    egui::FontId::proportional(lbl_size),
                    crate::theme::SECONDARY_TEXT,
                );
            }
            (Some(val), None) => {
                let val_size = (self.radius * 0.46).clamp(13.0, 34.0);
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    val,
                    egui::FontId::proportional(val_size),
                    crate::theme::PRIMARY_TEXT,
                );
            }
            (None, Some(lbl)) => {
                let lbl_size = (self.radius * 0.26).clamp(9.0, 16.0);
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    lbl,
                    egui::FontId::proportional(lbl_size),
                    crate::theme::SECONDARY_TEXT,
                );
            }
            (None, None) => {}
        }

        response
    }
}

impl<'a> egui::Widget for CircularArcGauge<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui)
    }
}

/// Helper to draw a horizontal progress bar with label and value
#[allow(dead_code)]
pub fn telemetry_bar(ui: &mut egui::Ui, label: &str, value_text: &str, ratio: f32, color: Color32) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).small().color(crate::theme::SECONDARY_TEXT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(value_text).small().strong().color(crate::theme::PRIMARY_TEXT));
            });
        });
        ui.add_space(2.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 5.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.5, crate::theme::BORDER);
        let clamped = if ratio.is_finite() { ratio.clamp(0.0, 1.0) } else { 0.0 };
        if clamped > 0.0 {
            let fill_rect = egui::Rect::from_min_size(
                rect.min,
                egui::vec2(rect.width() * clamped, rect.height()),
            );
            painter.rect_filled(fill_rect, 2.5, color);
        }
    });
}

/// High-fidelity Tachometer / Speedometer Hero Gauge modeled after the Clevo/Acer Control Center
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct HeroTachometerGauge<'a> {
    pub value: f32,
    pub max_value: f32,
    pub radius: f32,
    pub center_title: &'a str,
    pub clock_mhz: Option<u32>,
    pub utilization_pct: f32,
    pub temp_c: Option<i32>,
    pub accent_color: Color32,
}

#[allow(dead_code)]
impl<'a> HeroTachometerGauge<'a> {
    pub fn new(value: f32, max_value: f32, center_title: &'a str) -> Self {
        Self {
            value,
            max_value,
            radius: 82.0,
            center_title,
            clock_mhz: None,
            utilization_pct: value,
            temp_c: None,
            accent_color: crate::theme::PRIMARY_ACCENT,
        }
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn clock_mhz(mut self, mhz: Option<u32>) -> Self {
        self.clock_mhz = mhz;
        self
    }

    pub fn utilization_pct(mut self, pct: f32) -> Self {
        self.utilization_pct = pct;
        self
    }

    pub fn temp_c(mut self, temp: Option<i32>) -> Self {
        self.temp_c = temp;
        self
    }

    pub fn accent_color(mut self, color: Color32) -> Self {
        self.accent_color = color;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let width = (self.radius + 36.0) * 2.0;
        let height = self.radius * 2.0 + 82.0;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());

        if self.radius <= 0.0 {
            return response;
        }

        let painter = ui.painter_at(rect);
        let center = egui::pos2(rect.center().x, rect.top() + self.radius + 26.0);

        let ratio = if !self.value.is_finite() || !self.max_value.is_finite() || self.max_value <= 0.0 {
            0.0
        } else {
            (self.value / self.max_value).clamp(0.0, 1.0)
        };

        let start_angle = std::f32::consts::PI * 0.75; // 135 degrees (bottom-left)
        let sweep_angle = std::f32::consts::PI * 1.5;  // 270 degrees sweep

        // 1. Outer Ray Ticks Ring
        let tick_count = 30;
        for i in 0..=tick_count {
            let t = i as f32 / tick_count as f32;
            let a = start_angle + t * sweep_angle;
            let is_major = i % 6 == 0;
            let r_in = self.radius + 7.0;
            let r_out = if is_major { self.radius + 17.0 } else { self.radius + 11.0 };
            let stroke = if is_major { 1.5_f32 } else { 1.0_f32 };
            let color = if is_major {
                crate::theme::PRIMARY_TEXT
            } else {
                crate::theme::BORDER_LIGHT
            };

            let p_in = center + egui::vec2(a.cos() * r_in, a.sin() * r_in);
            let p_out = center + egui::vec2(a.cos() * r_out, a.sin() * r_out);
            painter.line_segment([p_in, p_out], egui::Stroke::new(stroke, color));

            if is_major {
                let label_val = i / 6 + 1;
                let p_lbl = center + egui::vec2(a.cos() * (self.radius + 26.0), a.sin() * (self.radius + 26.0));
                painter.text(
                    p_lbl,
                    egui::Align2::CENTER_CENTER,
                    label_val.to_string(),
                    egui::FontId::proportional(11.0),
                    crate::theme::SECONDARY_TEXT,
                );
            }
        }

        // 2. Main Track Arc (8.0 stroke)
        let steps = ((self.radius * sweep_angle) / 2.5).ceil().max(16.0) as usize;
        let track_pts: Vec<egui::Pos2> = (0..=steps)
            .map(|i| {
                let t = i as f32 / steps as f32;
                let a = start_angle + t * sweep_angle;
                center + egui::vec2(a.cos() * self.radius, a.sin() * self.radius)
            })
            .collect();

        if track_pts.len() >= 2 {
            painter.add(egui::Shape::Path(eframe::epaint::PathShape::line(
                track_pts.clone(),
                egui::Stroke::new(8.0_f32, crate::theme::BORDER),
            )));
            painter.circle_filled(track_pts[0], 4.0, crate::theme::BORDER);
            painter.circle_filled(*track_pts.last().unwrap(), 4.0, crate::theme::BORDER);
        }

        // 3. Progress Arc
        if ratio > 0.0 {
            let prog_sweep = sweep_angle * ratio;
            let prog_steps = ((self.radius * prog_sweep) / 2.5).ceil().max(2.0) as usize;
            let prog_pts: Vec<egui::Pos2> = (0..=prog_steps)
                .map(|i| {
                    let t = i as f32 / prog_steps as f32;
                    let a = start_angle + t * prog_sweep;
                    center + egui::vec2(a.cos() * self.radius, a.sin() * self.radius)
                })
                .collect();

            if prog_pts.len() >= 2 {
                painter.add(egui::Shape::Path(eframe::epaint::PathShape::line(
                    prog_pts.clone(),
                    egui::Stroke::new(8.0_f32, self.accent_color),
                )));
                painter.circle_filled(prog_pts[0], 4.0, self.accent_color);
                painter.circle_filled(*prog_pts.last().unwrap(), 4.0, self.accent_color);
            }
        }

        // 4. Inner Concentric Decorative Arc
        let inner_radius = self.radius - 18.0;
        if inner_radius > 0.0 {
            let inner_pts: Vec<egui::Pos2> = (0..=steps)
                .map(|i| {
                    let t = i as f32 / steps as f32;
                    let a = start_angle + t * sweep_angle;
                    center + egui::vec2(a.cos() * inner_radius, a.sin() * inner_radius)
                })
                .collect();
            painter.add(egui::Shape::Path(eframe::epaint::PathShape::line(
                inner_pts,
                egui::Stroke::new(1.0_f32, crate::theme::BORDER),
            )));
        }

        // 5. Center Title
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            self.center_title,
            egui::FontId::proportional(19.0),
            crate::theme::PRIMARY_TEXT,
        );

        // 6. Angled Callout Lines (Bottom branches)
        let joint_left = center + egui::vec2(-14.0, self.radius - 16.0);
        let end_left = center + egui::vec2(-78.0, self.radius + 24.0);
        painter.line_segment([joint_left, end_left], egui::Stroke::new(1.2_f32, crate::theme::BORDER_LIGHT));
        painter.circle_filled(joint_left, 2.5, self.accent_color);

        let joint_right = center + egui::vec2(14.0, self.radius - 16.0);
        let end_right = center + egui::vec2(78.0, self.radius + 24.0);
        painter.line_segment([joint_right, end_right], egui::Stroke::new(1.2_f32, crate::theme::BORDER_LIGHT));
        painter.circle_filled(joint_right, 2.5, self.accent_color);

        // 7. Lower Metric Triad
        // Left: Frequency (MHz)
        let left_center = center + egui::vec2(-78.0, self.radius + 40.0);
        let freq_str = self.clock_mhz.map(|m| format!("{m}")).unwrap_or_else(|| "---".into());
        painter.text(
            left_center - egui::vec2(0.0, 5.0),
            egui::Align2::CENTER_CENTER,
            freq_str,
            egui::FontId::proportional(16.0),
            crate::theme::PRIMARY_TEXT,
        );
        painter.text(
            left_center + egui::vec2(0.0, 10.0),
            egui::Align2::CENTER_CENTER,
            "MHz  CLOCK",
            egui::FontId::proportional(9.0),
            crate::theme::SECONDARY_TEXT,
        );

        // Center: Utilization (%)
        let mid_center = center + egui::vec2(0.0, self.radius + 32.0);
        painter.text(
            mid_center - egui::vec2(0.0, 5.0),
            egui::Align2::CENTER_CENTER,
            format!("{:.0}%", self.utilization_pct),
            egui::FontId::proportional(18.0),
            crate::theme::PRIMARY_TEXT,
        );
        painter.text(
            mid_center + egui::vec2(0.0, 10.0),
            egui::Align2::CENTER_CENTER,
            "LOAD",
            egui::FontId::proportional(9.0),
            crate::theme::SECONDARY_TEXT,
        );

        // Right: Temperature (°C)
        let right_center = center + egui::vec2(78.0, self.radius + 40.0);
        let temp_str = self.temp_c.map(|c| format!("{c}°C")).unwrap_or_else(|| "---".into());
        painter.text(
            right_center - egui::vec2(0.0, 5.0),
            egui::Align2::CENTER_CENTER,
            temp_str,
            egui::FontId::proportional(16.0),
            crate::theme::PRIMARY_TEXT,
        );
        painter.text(
            right_center + egui::vec2(0.0, 10.0),
            egui::Align2::CENTER_CENTER,
            "TEMP",
            egui::FontId::proportional(9.0),
            crate::theme::SECONDARY_TEXT,
        );

        response
    }
}

impl<'a> egui::Widget for HeroTachometerGauge<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui)
    }
}

/// Convert HSV to egui Color32
#[allow(dead_code)]
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color32 {
    let h = (h % 1.0 + 1.0) % 1.0;
    let i = (h * 6.0).floor() as i32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);

    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };

    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Extract hue [0.0, 1.0] from egui Color32
#[allow(dead_code)]
pub fn rgb_to_hue(color: Color32) -> f32 {
    let r = color.r() as f32 / 255.0;
    let g = color.g() as f32 / 255.0;
    let b = color.b() as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    if delta < 1e-4 {
        return 0.0;
    }

    let h = if (max - r).abs() < 1e-4 {
        (g - b) / delta
    } else if (max - g).abs() < 1e-4 {
        2.0 + (b - r) / delta
    } else {
        4.0 + (r - g) / delta
    };

    let h = (h / 6.0) % 1.0;
    if h < 0.0 { h + 1.0 } else { h }
}

/// Interactive Clevo-style Color Wheel / Spectrum Ring
#[allow(dead_code)]
pub fn color_wheel_ring(
    ui: &mut egui::Ui,
    current_color: &mut Color32,
    radius: f32,
) -> bool {
    let size = (radius + 22.0) * 2.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click_and_drag());
    let center = rect.center();
    let painter = ui.painter_at(rect);

    let mut changed = false;

    // 1. Interaction: detect click/drag around the ring
    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let diff = pos - center;
            let dist = diff.length();
            if dist >= radius - 20.0 && dist <= radius + 20.0 {
                let angle = diff.y.atan2(diff.x); // -PI..PI
                let angle_norm = if angle < 0.0 { angle + std::f32::consts::TAU } else { angle };
                let hue = angle_norm / std::f32::consts::TAU;
                *current_color = hsv_to_rgb(hue, 1.0, 1.0);
                changed = true;
            }
        }
    }

    // 2. Radiating Ray Ticks
    let tick_count = 36;
    for i in 0..tick_count {
        let a = i as f32 / tick_count as f32 * std::f32::consts::TAU;
        let p_in = center + egui::vec2(a.cos() * (radius + 10.0), a.sin() * (radius + 10.0));
        let p_out = center + egui::vec2(a.cos() * (radius + 16.0), a.sin() * (radius + 16.0));
        painter.line_segment([p_in, p_out], egui::Stroke::new(1.0_f32, crate::theme::BORDER_LIGHT));
    }

    // 3. Spectrum Hue Ring (72 discrete arc segments around 360°)
    let segments = 72;
    for i in 0..segments {
        let t0 = i as f32 / segments as f32;
        let t1 = (i + 1) as f32 / segments as f32;
        let a0 = t0 * std::f32::consts::TAU;
        let a1 = t1 * std::f32::consts::TAU;

        let col = hsv_to_rgb(t0, 1.0, 1.0);
        let p0 = center + egui::vec2(a0.cos() * radius, a0.sin() * radius);
        let p1 = center + egui::vec2(a1.cos() * radius, a1.sin() * radius);
        painter.line_segment([p0, p1], egui::Stroke::new(14.0_f32, col));
    }

    // 4. Indicator Needle / Pin at current color hue
    let cur_hue = rgb_to_hue(*current_color);
    let cur_angle = cur_hue * std::f32::consts::TAU;
    let pin_pos = center + egui::vec2(cur_angle.cos() * radius, cur_angle.sin() * radius);
    painter.circle_filled(pin_pos, 7.0, Color32::WHITE);
    painter.circle_stroke(pin_pos, 7.0, egui::Stroke::new(2.0_f32, Color32::BLACK));

    // 5. Center Swatch Circle & Text
    let inner_r = radius - 18.0;
    painter.circle_filled(center, inner_r, *current_color);
    painter.circle_stroke(center, inner_r, egui::Stroke::new(2.0_f32, crate::theme::BORDER));

    // Center Label Overlay
    let lum = current_color.r() as u32 * 299 + current_color.g() as u32 * 587 + current_color.b() as u32 * 114;
    let text_col = if lum > 130_000 { Color32::BLACK } else { Color32::WHITE };

    painter.text(
        center - egui::vec2(0.0, 6.0),
        egui::Align2::CENTER_CENTER,
        "Color backlight",
        egui::FontId::proportional(11.0),
        text_col,
    );
    painter.text(
        center + egui::vec2(0.0, 9.0),
        egui::Align2::CENTER_CENTER,
        format!("#{:02X}{:02X}{:02X}", current_color.r(), current_color.g(), current_color.b()),
        egui::FontId::proportional(10.0),
        text_col,
    );

    changed
}

/// Single-zone physical keyboard vector preview with dynamic lighting and effect rendering
#[allow(dead_code)]
pub fn single_zone_keyboard_preview(
    ui: &mut egui::Ui,
    color: Color32,
    brightness: u8,
    animation_mode: AnimationMode,
) {
    let width = ui.available_width().min(740.0).max(520.0);
    let height = width * 0.30;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Dynamic color calculation based on effect mode and time
    let effective_color = match animation_mode {
        AnimationMode::Off => {
            let glow = |c: u8| ((c as u16 * brightness as u16) / 255) as u8;
            Color32::from_rgb(glow(color.r()), glow(color.g()), glow(color.b()))
        }
        AnimationMode::Breathing => {
            let time = ui.input(|i| i.time) as f32;
            let breath = ((time * 2.2).sin() * 0.5 + 0.5).powf(1.6);
            let eff_bright = (brightness as f32 * breath) as u8;
            let glow = |c: u8| ((c as u16 * eff_bright as u16) / 255) as u8;
            Color32::from_rgb(glow(color.r()), glow(color.g()), glow(color.b()))
        }
        AnimationMode::Rainbow => {
            let time = ui.input(|i| i.time) as f32;
            let hue = (time * 0.22) % 1.0;
            let rgb = hsv_to_rgb(hue, 1.0, 1.0);
            let glow = |c: u8| ((c as u16 * brightness as u16) / 255) as u8;
            Color32::from_rgb(glow(rgb.r()), glow(rgb.g()), glow(rgb.b()))
        }
    };

    if animation_mode != AnimationMode::Off {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }

    // Outer Keyboard Chassis Bed
    painter.rect_filled(rect, 8.0, Color32::from_rgb(14, 17, 22));
    painter.rect_stroke(rect, 8.0, egui::Stroke::new(1.0_f32, crate::theme::BORDER), egui::StrokeKind::Inside);

    // Keyboard Layout Definition (ANSI 96% / 100% Clevo Laptop Keyboard)
    let rows: [&[(&str, f32)]; 6] = [
        &[
            ("Esc", 1.0), ("F1", 1.0), ("F2", 1.0), ("F3", 1.0), ("F4", 1.0),
            ("F5", 1.0), ("F6", 1.0), ("F7", 1.0), ("F8", 1.0), ("F9", 1.0),
            ("F10", 1.0), ("F11", 1.0), ("F12", 1.0), ("Prt", 1.0), ("Del", 1.0),
            ("Num", 1.0), ("/", 1.0), ("*", 1.0), ("-", 1.0),
        ],
        &[
            ("`", 1.0), ("1", 1.0), ("2", 1.0), ("3", 1.0), ("4", 1.0),
            ("5", 1.0), ("6", 1.0), ("7", 1.0), ("8", 1.0), ("9", 1.0),
            ("0", 1.0), ("-", 1.0), ("=", 1.0), ("<-", 1.8),
            ("7", 1.0), ("8", 1.0), ("9", 1.0), ("+", 1.0),
        ],
        &[
            ("Tab", 1.4), ("Q", 1.0), ("W", 1.0), ("E", 1.0), ("R", 1.0),
            ("T", 1.0), ("Y", 1.0), ("U", 1.0), ("I", 1.0), ("O", 1.0),
            ("P", 1.0), ("[", 1.0), ("]", 1.0), ("\\", 1.4),
            ("4", 1.0), ("5", 1.0), ("6", 1.0),
        ],
        &[
            ("Caps", 1.7), ("A", 1.0), ("S", 1.0), ("D", 1.0), ("F", 1.0),
            ("G", 1.0), ("H", 1.0), ("J", 1.0), ("K", 1.0), ("L", 1.0),
            (";", 1.0), ("'", 1.0), ("Enter", 2.1),
            ("1", 1.0), ("2", 1.0), ("3", 1.0), ("Ent", 1.0),
        ],
        &[
            ("Shift", 2.2), ("Z", 1.0), ("X", 1.0), ("C", 1.0), ("V", 1.0),
            ("B", 1.0), ("N", 1.0), ("M", 1.0), (",", 1.0), (".", 1.0),
            ("/", 1.0), ("Shift", 1.6), ("^", 1.0),
            ("0", 2.0), (".", 1.0),
        ],
        &[
            ("Ctrl", 1.2), ("Fn", 1.0), ("Win", 1.0), ("Alt", 1.2),
            ("Space", 5.2),
            ("Alt", 1.0), ("Ctrl", 1.0), ("<", 1.0), ("v", 1.0), (">", 1.0),
        ],
    ];

    let pad_x = 10.0;
    let pad_y = 10.0;
    let gap = 3.0;
    let avail_w = rect.width() - pad_x * 2.0;
    let avail_h = rect.height() - pad_y * 2.0;
    let unit_h = (avail_h - gap * 5.0) / 6.0;

    for (row_idx, row) in rows.iter().enumerate() {
        let row_y = rect.top() + pad_y + row_idx as f32 * (unit_h + gap);
        let mut cur_x = rect.left() + pad_x;

        let total_units = row.iter().map(|(_, u)| *u).sum::<f32>();
        let unit_w = (avail_w - gap * (row.len() as f32 - 1.0)) / total_units.max(18.0);

        for (label, units) in row.iter() {
            let key_w = unit_w * units;
            let key_rect = egui::Rect::from_min_size(
                egui::pos2(cur_x, row_y),
                egui::vec2(key_w, unit_h),
            );

            // Keycap body (dark slate)
            painter.rect_filled(key_rect, 3.0, Color32::from_rgb(22, 26, 33));

            // Single-zone illumination: glow border and text
            painter.rect_stroke(
                key_rect,
                3.0,
                egui::Stroke::new(1.0_f32, effective_color),
                egui::StrokeKind::Inside,
            );

            // Key text
            let font_size = if label.len() > 3 { 8.5 } else { 9.5 };
            painter.text(
                key_rect.center(),
                egui::Align2::CENTER_CENTER,
                *label,
                egui::FontId::proportional(font_size),
                effective_color,
            );

            cur_x += key_w + gap;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    #[test]
    fn test_circular_arc_gauge_ratio_math() {
        // Normal range
        let gauge = CircularArcGauge::new(50.0, 100.0);
        assert!((gauge.calculate_ratio() - 0.5).abs() < 1e-5);

        // Clamping negative value to 0.0
        let gauge_neg = CircularArcGauge::new(-20.0, 100.0);
        assert_eq!(gauge_neg.calculate_ratio(), 0.0);

        // Clamping overflow value to 1.0
        let gauge_over = CircularArcGauge::new(150.0, 100.0);
        assert_eq!(gauge_over.calculate_ratio(), 1.0);

        // Safe fallback on zero max
        let gauge_zero_max = CircularArcGauge::new(50.0, 0.0);
        assert_eq!(gauge_zero_max.calculate_ratio(), 0.0);

        // Safe fallback on negative max
        let gauge_neg_max = CircularArcGauge::new(50.0, -100.0);
        assert_eq!(gauge_neg_max.calculate_ratio(), 0.0);

        // Safe fallback on NaN
        let gauge_nan = CircularArcGauge::new(f32::NAN, 100.0);
        assert_eq!(gauge_nan.calculate_ratio(), 0.0);

        // Safe fallback on Infinity
        let gauge_inf = CircularArcGauge::new(f32::INFINITY, 100.0);
        assert_eq!(gauge_inf.calculate_ratio(), 0.0);
    }

    #[test]
    fn test_circular_arc_gauge_presets() {
        let tach = CircularArcGauge::tachometer(25.0, 100.0);
        assert!((tach.start_angle - std::f32::consts::PI * 0.75).abs() < 1e-5);
        assert!((tach.sweep_angle - std::f32::consts::PI * 1.5).abs() < 1e-5);

        let ring = CircularArcGauge::ring(75.0, 100.0);
        assert!((ring.start_angle - (-std::f32::consts::FRAC_PI_2)).abs() < 1e-5);
        assert!((ring.sweep_angle - std::f32::consts::TAU).abs() < 1e-5);
    }

    #[test]
    fn test_circular_arc_gauge_builder_and_render() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                // Tachometer with center text
                CircularArcGauge::tachometer(45.0, 100.0)
                    .radius(40.0)
                    .stroke_width(8.0)
                    .colors(crate::theme::PRIMARY_ACCENT, crate::theme::BORDER)
                    .center_text("45%", "CPU LOAD")
                    .rounded_caps(true)
                    .show(ui);

                // Ring with single value
                CircularArcGauge::ring(90.0, 100.0)
                    .radius(30.0)
                    .stroke_width(5.0)
                    .bar_color(crate::theme::SECONDARY_ACCENT)
                    .center_value("90%")
                    .show(ui);

                // Custom angles in degrees
                CircularArcGauge::new(10.0, 100.0)
                    .angles_deg(180.0, 180.0)
                    .center_label("HALF")
                    .show(ui);
            });
        });
    }

    #[test]
    fn test_hsv_color_conversion() {
        let red = hsv_to_rgb(0.0, 1.0, 1.0);
        assert_eq!(red, Color32::from_rgb(255, 0, 0));

        let green = hsv_to_rgb(1.0 / 3.0, 1.0, 1.0);
        assert_eq!(green, Color32::from_rgb(0, 255, 0));

        let blue = hsv_to_rgb(2.0 / 3.0, 1.0, 1.0);
        assert_eq!(blue, Color32::from_rgb(0, 0, 255));

        assert!((rgb_to_hue(Color32::from_rgb(255, 0, 0)) - 0.0).abs() < 1e-3);
        assert!((rgb_to_hue(Color32::from_rgb(0, 255, 0)) - 1.0 / 3.0).abs() < 1e-3);
        assert!((rgb_to_hue(Color32::from_rgb(0, 0, 255)) - 2.0 / 3.0).abs() < 1e-3);
    }

    #[test]
    fn test_keyboard_preview_and_wheel_render() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut col = Color32::from_rgb(255, 0, 0);
                let _ = color_wheel_ring(ui, &mut col, 50.0);
                single_zone_keyboard_preview(ui, col, 255, AnimationMode::Off);
                single_zone_keyboard_preview(ui, col, 128, AnimationMode::Breathing);
                single_zone_keyboard_preview(ui, col, 255, AnimationMode::Rainbow);
            });
        });
    }

    #[test]
    fn test_cooling_tachometer_and_fan_modes() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                // Test CPU Fan hero tachometer with 2100 RPM
                CircularArcGauge::tachometer(2100.0, 5500.0)
                    .radius(64.0)
                    .stroke_width(9.0)
                    .bar_color(crate::theme::PRIMARY_ACCENT)
                    .center_text("2100", "RPM")
                    .show(ui);

                // Test GPU Fan dual-channel ring with 1950 RPM
                CircularArcGauge::ring(1950.0, 5500.0)
                    .radius(28.0)
                    .stroke_width(5.0)
                    .bar_color(crate::theme::SECONDARY_ACCENT)
                    .center_text("1950", "GPU")
                    .show(ui);

                // Test zero-RPM (passive mode)
                let gauge_zero = CircularArcGauge::tachometer(0.0, 5500.0);
                assert_eq!(gauge_zero.calculate_ratio(), 0.0);

                // Test maximum RPM clamp
                let gauge_max = CircularArcGauge::tachometer(5500.0, 5500.0);
                assert!((gauge_max.calculate_ratio() - 1.0).abs() < 1e-5);
            });
        });
    }
}
