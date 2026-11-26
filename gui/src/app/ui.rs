use super::*;

impl ControlCenter {
    pub(super) fn page_content(&mut self, ui: &mut egui::Ui) {
        match self.page {
            Page::Dashboard => {
                let snapshot = self.telemetry_collector.snapshot();

                // 1. Dashboard Sub-Header (Hardware model badges)
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&self.cpu_model)
                            .small()
                            .strong()
                            .color(crate::theme::PRIMARY_TEXT),
                    );
                    ui.label(RichText::new("•").small().color(crate::theme::BORDER_LIGHT));
                    ui.label(
                        RichText::new(&self.gpu_model)
                            .small()
                            .strong()
                            .color(crate::theme::SECONDARY_TEXT),
                    );
                });
                ui.add_space(12.0);

                // 2. Main 2-Column Dashboard Layout
                ui.columns(2, |columns| {
                    // LEFT COLUMN: Hero Speedometer & Fan Mode Control
                    let left = &mut columns[0];

                    // Performance mode pills at top of dial (visual reference faithful)
                    left.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let modes = [("Quiet", false), ("Balance", true), ("Performance", false)];
                        for (mode_name, active) in modes {
                            let stroke = if active {
                                egui::Stroke::new(1.0_f32, crate::theme::PRIMARY_ACCENT)
                            } else {
                                egui::Stroke::new(1.0_f32, crate::theme::BORDER)
                            };
                            let fill = if active {
                                crate::theme::SURFACE_ACTIVE
                            } else {
                                crate::theme::SURFACE
                            };
                            let text_color = if active {
                                crate::theme::PRIMARY_ACCENT
                            } else {
                                crate::theme::MUTED_TEXT
                            };
                            egui::Frame::new()
                                .fill(fill)
                                .stroke(stroke)
                                .corner_radius(crate::theme::RADIUS_SM)
                                .inner_margin(egui::Margin::symmetric(14, 5))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(mode_name).size(10.0).color(text_color));
                                });
                        }
                    });
                    left.add_space(10.0);

                    // Hero Dial Container
                    crate::theme::card_frame().show(left, |ui| {
                        ui.vertical_centered(|ui| {
                            let cpu_freq = if snapshot.cpu.avg_freq_mhz > 0 {
                                Some(snapshot.cpu.avg_freq_mhz)
                            } else {
                                None
                            };

                            let hero_gauge = components::HeroTachometerGauge::new(
                                snapshot.cpu.utilization_pct,
                                100.0,
                                "CPU",
                            )
                            .radius(84.0)
                            .clock_mhz(cpu_freq)
                            .utilization_pct(snapshot.cpu.utilization_pct)
                            .temp_c(snapshot.cpu.temp_c)
                            .accent_color(crate::theme::SECONDARY_ACCENT);

                            ui.add(hero_gauge);
                            ui.add_space(14.0);

                            // Fan Mode Selection Bar directly below hero dial
                            ui.label(
                                RichText::new("FAN CONTROL MODE")
                                    .size(9.0)
                                    .strong()
                                    .color(crate::theme::MUTED_TEXT),
                            );
                            ui.add_space(6.0);

                            let current_fan_mode = self.fan_mode;
                            let selected = components::segmented_choice(
                                ui,
                                current_fan_mode,
                                &[
                                    (components::FanMode::Auto, "Auto Mode"),
                                    (components::FanMode::Max, "Maximum Fan"),
                                ],
                            );
                            if let Some(mode) = selected {
                                let success = match mode {
                                    components::FanMode::Auto => self.action("Auto fan", || {
                                        libacercontrol::dbus::AcerControlDBus::new()?.set_fan_auto()
                                    }),
                                    components::FanMode::Max => self.action("Maximum fan", || {
                                        libacercontrol::dbus::AcerControlDBus::new()?.set_fan_max()
                                    }),
                                };
                                if success {
                                    self.fan_mode = mode;
                                    self.fan_mode_confirmed = true;
                                }
                            }
                            ui.add_space(4.0);
                            let status_text = if self.fan_mode_confirmed {
                                format!("Active Fan Policy: {}", match self.fan_mode {
                                    components::FanMode::Auto => "Dynamic Auto Curve",
                                    components::FanMode::Max => "100% Maximum Cooling",
                                })
                            } else {
                                "Startup default: Auto Mode active".to_string()
                            };
                            ui.label(RichText::new(status_text).size(9.0).color(crate::theme::MUTED_TEXT));
                        });
                    });

                    // RIGHT COLUMN: Modular Telemetry Cards Grid
                    let right = &mut columns[1];
                    right.spacing_mut().item_spacing.y = 10.0;

                    // Card 1: Memory & Storage Card
                    crate::theme::card_frame().show(right, |ui| {
                        ui.label(
                            RichText::new("MEMORY & STORAGE")
                                .small()
                                .strong()
                                .color(crate::theme::SECONDARY_TEXT),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            let col_w = (ui.available_width() - 12.0) * 0.5;

                            // RAM Gauge
                            ui.allocate_ui(egui::vec2(col_w, 105.0), |ui| {
                                ui.vertical_centered(|ui| {
                                    let ram_pct_str = format!("{:.0}%", snapshot.memory.ram_percent);
                                    components::CircularArcGauge::ring(snapshot.memory.ram_percent, 100.0)
                                        .radius(28.0)
                                        .stroke_width(5.0)
                                        .bar_color(crate::theme::PRIMARY_ACCENT)
                                        .center_text(&ram_pct_str, "RAM")
                                        .show(ui);
                                    ui.add_space(2.0);
                                    ui.label(
                                        RichText::new(format!(
                                            "{:.1} / {:.1} GB",
                                            snapshot.memory.ram_used_mb as f32 / 1024.0,
                                            snapshot.memory.ram_total_mb as f32 / 1024.0
                                        ))
                                        .size(9.5)
                                        .color(crate::theme::MUTED_TEXT),
                                    );
                                });
                            });

                            // Storage Gauge
                            ui.allocate_ui(egui::vec2(col_w, 105.0), |ui| {
                                ui.vertical_centered(|ui| {
                                    let disk_pct_str = format!("{:.0}%", snapshot.memory.disk_percent);
                                    components::CircularArcGauge::ring(snapshot.memory.disk_percent, 100.0)
                                        .radius(28.0)
                                        .stroke_width(5.0)
                                        .bar_color(crate::theme::PRIMARY_ACCENT)
                                        .center_text(&disk_pct_str, "SSD")
                                        .show(ui);
                                    ui.add_space(2.0);
                                    ui.label(
                                        RichText::new(format!(
                                            "{} / {} GB",
                                            snapshot.memory.disk_used_gb,
                                            snapshot.memory.disk_total_gb
                                        ))
                                        .size(9.5)
                                        .color(crate::theme::MUTED_TEXT),
                                    );
                                });
                            });
                        });
                    });

                    // Card 2: Fan Speed Telemetry Card
                    crate::theme::card_frame().show(right, |ui| {
                        ui.label(
                            RichText::new("FAN TELEMETRY")
                                .small()
                                .strong()
                                .color(crate::theme::SECONDARY_TEXT),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            let col_w = (ui.available_width() - 12.0) * 0.5;

                            // CPU Fan
                            ui.allocate_ui(egui::vec2(col_w, 105.0), |ui| {
                                ui.vertical_centered(|ui| {
                                    let cpu_rpm = snapshot.fans.cpu_fan_rpm.unwrap_or(0);
                                    let cpu_pct = ((cpu_rpm as f32 / 5000.0) * 100.0).clamp(0.0, 100.0);
                                    let cpu_pct_str = if snapshot.fans.cpu_fan_rpm.is_some() {
                                        format!("{:.0}%", cpu_pct)
                                    } else {
                                        "---".into()
                                    };

                                    components::CircularArcGauge::ring(cpu_pct, 100.0)
                                        .radius(28.0)
                                        .stroke_width(5.0)
                                        .bar_color(crate::theme::PRIMARY_ACCENT)
                                        .center_text(&cpu_pct_str, "CPU")
                                        .show(ui);
                                    ui.add_space(2.0);
                                    let rpm_label = snapshot
                                        .fans
                                        .cpu_fan_rpm
                                        .map(|r| format!("{r} RPM"))
                                        .unwrap_or_else(|| "Unavailable".into());
                                    ui.label(RichText::new(rpm_label).size(9.5).color(crate::theme::MUTED_TEXT));
                                });
                            });

                            // GPU Fan
                            ui.allocate_ui(egui::vec2(col_w, 105.0), |ui| {
                                ui.vertical_centered(|ui| {
                                    let gpu_rpm = snapshot.fans.gpu_fan_rpm.unwrap_or(0);
                                    let gpu_pct = ((gpu_rpm as f32 / 5000.0) * 100.0).clamp(0.0, 100.0);
                                    let gpu_pct_str = if snapshot.fans.gpu_fan_rpm.is_some() {
                                        format!("{:.0}%", gpu_pct)
                                    } else {
                                        "---".into()
                                    };

                                    components::CircularArcGauge::ring(gpu_pct, 100.0)
                                        .radius(28.0)
                                        .stroke_width(5.0)
                                        .bar_color(crate::theme::PRIMARY_ACCENT)
                                        .center_text(&gpu_pct_str, "GPU")
                                        .show(ui);
                                    ui.add_space(2.0);
                                    let rpm_label = snapshot
                                        .fans
                                        .gpu_fan_rpm
                                        .map(|r| format!("{r} RPM"))
                                        .unwrap_or_else(|| "Unavailable".into());
                                    ui.label(RichText::new(rpm_label).size(9.5).color(crate::theme::MUTED_TEXT));
                                });
                            });
                        });
                    });

                    // Card 3: GPU Telemetry Card
                    crate::theme::card_frame().show(right, |ui| {
                        ui.label(
                            RichText::new("GPU TELEMETRY")
                                .small()
                                .strong()
                                .color(crate::theme::SECONDARY_TEXT),
                        );
                        ui.add_space(6.0);

                        match &snapshot.gpu {
                            crate::telemetry::GpuTelemetry::Active {
                                core_clock_mhz,
                                temp_c,
                                utilization_pct,
                                vram_used_mb,
                                vram_total_mb,
                            } => {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("CORE CLOCK").small().color(crate::theme::SECONDARY_TEXT));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new(format!("{core_clock_mhz} MHz")).small().strong().color(crate::theme::PRIMARY_TEXT));
                                    });
                                });
                                ui.add_space(4.0);

                                components::telemetry_bar(
                                    ui,
                                    "TEMPERATURE",
                                    &format!("{temp_c} °C"),
                                    *temp_c as f32 / 100.0,
                                    crate::theme::PRIMARY_ACCENT,
                                );
                                ui.add_space(4.0);

                                components::telemetry_bar(
                                    ui,
                                    "UTILIZATION",
                                    &format!("{utilization_pct} %"),
                                    *utilization_pct as f32 / 100.0,
                                    crate::theme::PRIMARY_ACCENT,
                                );
                                ui.add_space(4.0);

                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("VRAM USAGE").small().color(crate::theme::SECONDARY_TEXT));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new(format!("{vram_used_mb} / {vram_total_mb} MB")).small().color(crate::theme::PRIMARY_TEXT));
                                    });
                                });
                            }
                            crate::telemetry::GpuTelemetry::Sleeping => {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("POWER STATE").small().color(crate::theme::SECONDARY_TEXT));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new("Suspended (Power Saving)").small().strong().color(crate::theme::SECONDARY_ACCENT));
                                    });
                                });
                                ui.add_space(4.0);
                                ui.label(RichText::new("dGPU is currently asleep. Telemetry polling is paused to conserve battery.").size(9.5).color(crate::theme::MUTED_TEXT));
                            }
                            crate::telemetry::GpuTelemetry::Unavailable(err) => {
                                ui.label(RichText::new(format!("Telemetry unavailable: {err}")).small().color(crate::theme::MUTED_TEXT));
                            }
                        }
                    });

                    // Card 4: Quick Hardware Controls (Keyboard Backlight Brightness)
                    crate::theme::card_frame().show(right, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("KEYBOARD BACKLIGHT")
                                    .small()
                                    .strong()
                                    .color(crate::theme::SECONDARY_TEXT),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let level_enum = components::BrightnessLevel::from_pwm(self.brightness);
                                ui.label(RichText::new(format!("Level {}", level_enum.label())).small().color(crate::theme::PRIMARY_ACCENT));
                            });
                        });
                        ui.add_space(6.0);

                        let current_level = components::BrightnessLevel::from_pwm(self.brightness);
                        let levels = [
                            (components::BrightnessLevel::Off, "OFF"),
                            (components::BrightnessLevel::Level1, "1"),
                            (components::BrightnessLevel::Level2, "2"),
                            (components::BrightnessLevel::Level3, "3"),
                            (components::BrightnessLevel::Level4, "4"),
                        ];

                        if let Some(new_level) = components::segmented_choice(ui, current_level, &levels) {
                            let new_pwm = new_level.to_pwm();
                            if self.action("Keyboard brightness", || {
                                libacercontrol::dbus::AcerControlDBus::new()?.set_keyboard_brightness(new_pwm)
                            }) {
                                self.brightness = new_pwm;
                                if let Some(ctrl) = &self.idle_controller {
                                    ctrl.set_brightness(new_pwm);
                                }
                            }
                        }
                    });
                });
            }
            Page::Cooling => {
                let snapshot = self.telemetry_collector.snapshot();

                // 1. Sub-Header: Hardware model badges & cooling controller info
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&self.cpu_model)
                            .small()
                            .strong()
                            .color(crate::theme::PRIMARY_TEXT),
                    );
                    ui.label(RichText::new("•").small().color(crate::theme::BORDER_LIGHT));
                    ui.label(
                        RichText::new(&self.gpu_model)
                            .small()
                            .strong()
                            .color(crate::theme::SECONDARY_TEXT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let status_text = if self.fan_mode_confirmed {
                            match self.fan_mode {
                                components::FanMode::Auto => "Mode: Auto (Confirmed)",
                                components::FanMode::Max => "Mode: Maximum (Confirmed)",
                            }
                        } else {
                            "Mode: Auto (Assumed)"
                        };
                        let status_color = if self.fan_mode_confirmed {
                            crate::theme::PRIMARY_ACCENT
                        } else {
                            crate::theme::MUTED_TEXT
                        };
                        ui.label(RichText::new(status_text).size(10.0).strong().color(status_color));
                    });
                });
                ui.add_space(8.0);

                let cpu_rpm = snapshot.fans.cpu_fan_rpm;
                let gpu_rpm = snapshot.fans.gpu_fan_rpm;
                let cpu_temp = snapshot.cpu.temp_c;

                // 2. Main 2-Column Clevo Cooling Layout
                ui.columns(2, |columns| {
                    // LEFT COLUMN: Primary Clevo Hardware Console
                    let left = &mut columns[0];
                    crate::theme::card_frame().show(left, |ui| {
                        ui.set_min_height(380.0);

                        // Console Header with Fan Switcher Tabs (CPU Fan / GPU Fan)
                        ui.horizontal(|ui| {
                            let tab_w = (ui.available_width() - 8.0) * 0.5;

                            // CPU Fan Tab
                            let cpu_sel = self.cooling_tab == 0;
                            let cpu_btn = egui::Button::new(
                                RichText::new("CPU FAN")
                                    .small()
                                    .strong()
                                    .color(if cpu_sel { crate::theme::PRIMARY_ACCENT } else { crate::theme::SECONDARY_TEXT }),
                            )
                            .fill(if cpu_sel { crate::theme::SURFACE_ACTIVE } else { crate::theme::SURFACE })
                            .stroke(if cpu_sel { egui::Stroke::new(1.5_f32, crate::theme::PRIMARY_ACCENT) } else { egui::Stroke::new(1.0_f32, crate::theme::BORDER) })
                            .corner_radius(crate::theme::RADIUS_MD)
                            .min_size(egui::vec2(tab_w, 28.0));
                            if ui.add(cpu_btn).clicked() {
                                self.cooling_tab = 0;
                            }

                            // GPU Fan Tab
                            let gpu_sel = self.cooling_tab == 1;
                            let gpu_btn = egui::Button::new(
                                RichText::new("GPU FAN")
                                    .small()
                                    .strong()
                                    .color(if gpu_sel { crate::theme::SECONDARY_ACCENT } else { crate::theme::SECONDARY_TEXT }),
                            )
                            .fill(if gpu_sel { crate::theme::SURFACE_ACTIVE } else { crate::theme::SURFACE })
                            .stroke(if gpu_sel { egui::Stroke::new(1.5_f32, crate::theme::SECONDARY_ACCENT) } else { egui::Stroke::new(1.0_f32, crate::theme::BORDER) })
                            .corner_radius(crate::theme::RADIUS_MD)
                            .min_size(egui::vec2(tab_w, 28.0));
                            if ui.add(gpu_btn).clicked() {
                                self.cooling_tab = 1;
                            }
                        });
                        ui.add_space(14.0);

                        // Center: Hero Tachometer Display for Focused Fan
                        ui.vertical_centered(|ui| {
                            let (active_rpm, active_temp_str, active_lbl, active_color) = if self.cooling_tab == 0 {
                                (
                                    cpu_rpm,
                                    cpu_temp.map(|t| format!("{t} °C")).unwrap_or_else(|| "Unavailable".into()),
                                    "CPU COOLING FAN",
                                    crate::theme::PRIMARY_ACCENT,
                                )
                            } else {
                                (
                                    gpu_rpm,
                                    match &snapshot.gpu {
                                        crate::telemetry::GpuTelemetry::Active { temp_c, .. } => format!("{temp_c} °C"),
                                        crate::telemetry::GpuTelemetry::Sleeping => "Suspended / Idle".into(),
                                        crate::telemetry::GpuTelemetry::Unavailable(_) => "Unavailable".into(),
                                    },
                                    "GPU COOLING FAN",
                                    crate::theme::SECONDARY_ACCENT,
                                )
                            };

                            let rpm_val = active_rpm.unwrap_or(0).max(0) as f32;
                            let rpm_display = active_rpm
                                .map(|r| format!("{r}"))
                                .unwrap_or_else(|| "---".into());

                            // Tachometer Gauge (Max scale 5500 RPM)
                            components::CircularArcGauge::tachometer(rpm_val, 5500.0)
                                .radius(64.0)
                                .stroke_width(9.0)
                                .bar_color(active_color)
                                .center_text(&rpm_display, "RPM")
                                .show(ui);
                            ui.add_space(6.0);

                            ui.label(
                                RichText::new(active_lbl)
                                    .small()
                                    .strong()
                                    .color(crate::theme::PRIMARY_TEXT),
                            );

                            // Thermal sensor & rotation status
                            let status_text = match active_rpm {
                                Some(r) if r > 0 => format!("● Operating Normally ({r} RPM)"),
                                Some(_) => "● Passive Mode (Zero-RPM)".into(),
                                None => "○ Hardware Sensor Offline".into(),
                            };
                            let status_col = match active_rpm {
                                Some(r) if r > 0 => crate::theme::PRIMARY_ACCENT,
                                Some(_) => crate::theme::SECONDARY_TEXT,
                                None => crate::theme::MUTED_TEXT,
                            };
                            ui.label(RichText::new(status_text).size(10.5).color(status_col));
                            ui.label(
                                RichText::new(format!("Coupled Thermal Sensor: {active_temp_str}"))
                                    .size(10.0)
                                    .color(crate::theme::SECONDARY_TEXT),
                            );
                        });
                        ui.add_space(14.0);

                        // Mode Selector Strip matching Clevo reference
                        ui.separator();
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("FAN SPEED MODE")
                                .small()
                                .strong()
                                .color(crate::theme::SECONDARY_TEXT),
                        );
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            let btn_w = (ui.available_width() - 8.0) * 0.5;

                            // Auto Mode Button
                            let is_auto = self.fan_mode == components::FanMode::Auto;
                            let auto_btn = egui::Button::new(
                                RichText::new("AUTO")
                                    .strong()
                                    .color(if is_auto { crate::theme::PRIMARY_TEXT } else { crate::theme::SECONDARY_TEXT }),
                            )
                            .fill(if is_auto { crate::theme::SURFACE_ACTIVE } else { crate::theme::SURFACE })
                            .stroke(if is_auto { egui::Stroke::new(1.5_f32, crate::theme::PRIMARY_ACCENT) } else { egui::Stroke::new(1.0_f32, crate::theme::BORDER) })
                            .corner_radius(crate::theme::RADIUS_MD)
                            .min_size(egui::vec2(btn_w, 36.0));
                            if ui.add(auto_btn).clicked() {
                                if self.action("Auto fan", || libacercontrol::dbus::AcerControlDBus::new()?.set_fan_auto()) {
                                    self.fan_mode = components::FanMode::Auto;
                                    self.fan_mode_confirmed = true;
                                }
                            }

                            // Maximum Mode Button
                            let is_max = self.fan_mode == components::FanMode::Max;
                            let max_btn = egui::Button::new(
                                RichText::new("MAXIMUM")
                                    .strong()
                                    .color(if is_max { crate::theme::PRIMARY_TEXT } else { crate::theme::SECONDARY_TEXT }),
                            )
                            .fill(if is_max { crate::theme::SURFACE_ACTIVE } else { crate::theme::SURFACE })
                            .stroke(if is_max { egui::Stroke::new(1.5_f32, Color32::from_rgb(0xf5, 0x9e, 0x0b)) } else { egui::Stroke::new(1.0_f32, crate::theme::BORDER) })
                            .corner_radius(crate::theme::RADIUS_MD)
                            .min_size(egui::vec2(btn_w, 36.0));
                            if ui.add(max_btn).clicked() {
                                if self.action("Maximum fan", || libacercontrol::dbus::AcerControlDBus::new()?.set_fan_max()) {
                                    self.fan_mode = components::FanMode::Max;
                                    self.fan_mode_confirmed = true;
                                }
                            }
                        });
                        ui.add_space(8.0);

                        // Mode Description & Hardware State Status
                        let (mode_badge, mode_info, badge_col) = match (self.fan_mode, self.fan_mode_confirmed) {
                            (components::FanMode::Auto, true) => (
                                "● Active Mode: AUTO (Firmware EC Managed)",
                                "Embedded Controller balances thermals and acoustics dynamically based on thermal load.",
                                crate::theme::PRIMARY_ACCENT,
                            ),
                            (components::FanMode::Max, true) => (
                                "● Active Mode: MAXIMUM (Forced 100% Speed)",
                                "Both CPU and GPU fans forced to full speed (~5000+ RPM). Automatically restores Auto on reboot or exit.",
                                Color32::from_rgb(0xf5, 0x9e, 0x0b),
                            ),
                            (components::FanMode::Auto, false) => (
                                "○ Active Mode: AUTO (Assumed / Unconfirmed)",
                                "Auto is assumed at startup; daemon has not confirmed the hardware state.",
                                crate::theme::MUTED_TEXT,
                            ),
                            (components::FanMode::Max, false) => (
                                "○ Active Mode: MAXIMUM (Unconfirmed)",
                                "Maximum mode requested, waiting for hardware daemon confirmation.",
                                crate::theme::MUTED_TEXT,
                            ),
                        };

                        ui.label(RichText::new(mode_badge).size(10.5).strong().color(badge_col));
                        ui.label(RichText::new(mode_info).size(9.5).color(crate::theme::MUTED_TEXT));
                    });

                    // RIGHT COLUMN: Telemetry, Thermal Sensors, and Hardware Policies
                    let right = &mut columns[1];

                    // Card 1: Dual Fan Telemetry (风扇转速 in Clevo reference)
                    crate::theme::card_frame().show(right, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("FAN TELEMETRY")
                                    .small()
                                    .strong()
                                    .color(crate::theme::SECONDARY_TEXT),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new("Dual-Channel RPM").size(9.5).color(crate::theme::MUTED_TEXT));
                            });
                        });
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            let gauge_col_w = (ui.available_width() - 12.0) * 0.5;

                            // CPU Fan Gauge
                            ui.allocate_ui(egui::vec2(gauge_col_w, 110.0), |ui| {
                                ui.vertical_centered(|ui| {
                                    let cpu_rpm_val = cpu_rpm.unwrap_or(0).max(0) as f32;
                                    let cpu_str = cpu_rpm.map(|r| format!("{r}")).unwrap_or_else(|| "---".into());

                                    components::CircularArcGauge::ring(cpu_rpm_val, 5500.0)
                                        .radius(28.0)
                                        .stroke_width(5.0)
                                        .bar_color(crate::theme::PRIMARY_ACCENT)
                                        .center_text(&cpu_str, "CPU")
                                        .show(ui);
                                    ui.add_space(2.0);
                                    let label = cpu_rpm.map(|r| format!("{r} RPM")).unwrap_or_else(|| "Unavailable".into());
                                    ui.label(RichText::new(label).size(10.0).strong().color(crate::theme::PRIMARY_TEXT));
                                    ui.label(RichText::new("CPU Cooling Fan").size(8.5).color(crate::theme::MUTED_TEXT));
                                });
                            });

                            // GPU Fan Gauge
                            ui.allocate_ui(egui::vec2(gauge_col_w, 110.0), |ui| {
                                ui.vertical_centered(|ui| {
                                    let gpu_rpm_val = gpu_rpm.unwrap_or(0).max(0) as f32;
                                    let gpu_str = gpu_rpm.map(|r| format!("{r}")).unwrap_or_else(|| "---".into());

                                    components::CircularArcGauge::ring(gpu_rpm_val, 5500.0)
                                        .radius(28.0)
                                        .stroke_width(5.0)
                                        .bar_color(crate::theme::SECONDARY_ACCENT)
                                        .center_text(&gpu_str, "GPU")
                                        .show(ui);
                                    ui.add_space(2.0);
                                    let label = gpu_rpm.map(|r| format!("{r} RPM")).unwrap_or_else(|| "Unavailable".into());
                                    ui.label(RichText::new(label).size(10.0).strong().color(crate::theme::PRIMARY_TEXT));
                                    ui.label(RichText::new("GPU Cooling Fan").size(8.5).color(crate::theme::MUTED_TEXT));
                                });
                            });
                        });
                    });
                    right.add_space(10.0);

                    // Card 2: Thermal Sensors
                    crate::theme::card_frame().show(right, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("THERMAL SENSORS")
                                    .small()
                                    .strong()
                                    .color(crate::theme::SECONDARY_TEXT),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new("Hardware Temperatures").size(9.5).color(crate::theme::MUTED_TEXT));
                            });
                        });
                        ui.add_space(8.0);

                        // CPU Temp Bar
                        let cpu_t = cpu_temp.unwrap_or(0);
                        let cpu_t_ratio = (cpu_t as f32 / 100.0).clamp(0.0, 1.0);
                        let cpu_t_str = cpu_temp.map(|t| format!("{t} °C")).unwrap_or_else(|| "Unavailable".into());
                        components::telemetry_bar(
                            ui,
                            "CPU PACKAGE TEMP",
                            &cpu_t_str,
                            cpu_t_ratio,
                            crate::theme::PRIMARY_ACCENT,
                        );
                        ui.add_space(6.0);

                        // GPU Temp Bar
                        match &snapshot.gpu {
                            crate::telemetry::GpuTelemetry::Active { temp_c, .. } => {
                                let gpu_t_ratio = (*temp_c as f32 / 100.0).clamp(0.0, 1.0);
                                components::telemetry_bar(
                                    ui,
                                    "GPU DIE TEMP",
                                    &format!("{temp_c} °C"),
                                    gpu_t_ratio,
                                    crate::theme::SECONDARY_ACCENT,
                                );
                            }
                            crate::telemetry::GpuTelemetry::Sleeping => {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("GPU DIE TEMP").small().color(crate::theme::SECONDARY_TEXT));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new("Suspended (D3hot / Power Saving)").small().color(crate::theme::MUTED_TEXT));
                                    });
                                });
                            }
                            crate::telemetry::GpuTelemetry::Unavailable(_) => {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("GPU DIE TEMP").small().color(crate::theme::SECONDARY_TEXT));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new("Unavailable").small().color(crate::theme::MUTED_TEXT));
                                    });
                                });
                            }
                        }
                    });
                    right.add_space(10.0);

                    // Card 3: Hardware Safety & Daemon Policies
                    crate::theme::card_frame().show(right, |ui| {
                        ui.label(
                            RichText::new("HARDWARE SAFETY POLICIES")
                                .small()
                                .strong()
                                .color(crate::theme::SECONDARY_TEXT),
                        );
                        ui.add_space(6.0);
                        ui.label(RichText::new("• Auto Mode Default: Fans initialize to Auto when daemon starts.").size(9.5).color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new("• Non-Persistence: Maximum mode is temporary and never restored across reboots.").size(9.5).color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new("• Clean Exit: Daemon restores Auto mode synchronously upon termination.").size(9.5).color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new("• D-Bus Service: io.github.stanlysilas.AcerControl on system bus.").size(9.5).color(crate::theme::MUTED_TEXT));
                    });
                });
            }
            Page::Keyboard => {
                const PRESET_SWATCHES: [Color32; 12] = [
                    Color32::from_rgb(255, 30, 30),    // Vivid Red
                    Color32::from_rgb(255, 120, 0),   // Orange
                    Color32::from_rgb(255, 190, 0),   // Amber
                    Color32::from_rgb(250, 230, 0),   // Yellow
                    Color32::from_rgb(163, 230, 53),  // Lime
                    Color32::from_rgb(34, 197, 94),   // Green
                    Color32::from_rgb(6, 182, 212),   // Cyan
                    Color32::from_rgb(14, 165, 233),  // Sky Blue
                    Color32::from_rgb(59, 130, 246),  // Deep Blue
                    Color32::from_rgb(168, 85, 247),  // Purple
                    Color32::from_rgb(236, 72, 153),  // Magenta / Pink
                    Color32::from_rgb(255, 255, 255), // Pure White
                ];

                let mut changed = false;

                // Sub-header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("RGB BACKLIGHT & ILLUMINATION")
                            .small()
                            .strong()
                            .color(crate::theme::SECONDARY_TEXT),
                    );
                    ui.label(RichText::new("•").small().color(crate::theme::BORDER_LIGHT));
                    ui.label(
                        RichText::new("Single-Zone Keyboard Controller")
                            .small()
                            .color(crate::theme::MUTED_TEXT),
                    );
                });
                ui.add_space(8.0);

                // Top 3 Cards in 3 Columns
                ui.columns(3, |columns| {
                    // COLUMN 1: Inactivity Sleep Timer
                    let col0 = &mut columns[0];
                    crate::theme::card_frame().show(col0, |ui| {
                        ui.set_min_height(255.0);
                        ui.label(
                            RichText::new("INACTIVITY SLEEP TIMER")
                                .small()
                                .strong()
                                .color(crate::theme::SECONDARY_TEXT),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Fade after Inactivity")
                                .strong()
                                .color(crate::theme::PRIMARY_TEXT),
                        );
                        ui.label(
                            RichText::new("Automatically dims the keyboard backlight when idle to preserve laptop battery.")
                                .size(10.0)
                                .color(crate::theme::MUTED_TEXT),
                        );
                        ui.add_space(10.0);

                        let mut idle_timeout = self.idle_timeout_seconds;
                        let timeout_presets = [(0u32, "Off"), (10u32, "10s"), (20u32, "20s"), (30u32, "30s")];
                        if let Some(t) = components::segmented_choice(ui, idle_timeout, &timeout_presets) {
                            idle_timeout = t;
                        }
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Duration:").small().color(crate::theme::SECONDARY_TEXT));
                            egui::ComboBox::from_id_salt("keyboard-idle-timeout-combo")
                                .selected_text(if idle_timeout == 0 {
                                    "Off".to_owned()
                                } else {
                                    format!("{} seconds", idle_timeout)
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut idle_timeout, 0, "Off");
                                    for seconds in (5..=60).step_by(5) {
                                        ui.selectable_value(
                                            &mut idle_timeout,
                                            seconds,
                                            format!("{seconds} seconds"),
                                        );
                                    }
                                });
                        });

                        if idle_timeout != self.idle_timeout_seconds {
                            let previous = self.idle_timeout_seconds;
                            let result = keyboard_idle::save_timeout(idle_timeout)
                                .and_then(|()| {
                                    keyboard_idle::set_login_autostart(
                                        idle_timeout > 0 || self.minimize_to_tray,
                                    )
                                });
                            match result {
                                Ok(()) => {
                                    if idle_timeout > 0 && !self.tray_ready {
                                        match instance::start_background_service() {
                                            Ok(()) => {
                                                self.tray_ready = true;
                                                self.idle_controller = None;
                                            }
                                            Err(error) => {
                                                self.idle_controller = Some(keyboard_idle::KeyboardIdleController::new(idle_timeout, self.brightness));
                                                self.set_notice(false, format!("Inactivity dimming is active only while this window stays open because the tray service could not start: {error}"));
                                            }
                                        }
                                    } else if idle_timeout == 0 && !self.minimize_to_tray {
                                        instance::request_background_quit();
                                        self.tray_ready = false;
                                        self.idle_controller = None;
                                    } else if let Some(controller) = &self.idle_controller {
                                        controller.set_timeout(idle_timeout);
                                    }
                                    self.idle_timeout_seconds = idle_timeout;
                                    if self.notice.as_ref().is_none_or(|(ok, _, _)| *ok) {
                                        self.set_notice(true, if idle_timeout > 0 {
                                            "Inactivity timeout saved. The tray service keeps it running after the window closes.".to_owned()
                                        } else {
                                            "Inactivity timeout disabled and login startup removed.".to_owned()
                                        });
                                    }
                                }
                                Err(error) => {
                                    let _ = keyboard_idle::save_timeout(previous);
                                    self.set_notice(false, format!("Could not save inactivity settings: {error}"));
                                }
                            }
                        }

                        ui.add_space(8.0);
                        let timeout_status = self.idle_controller.as_ref().map(|controller| controller.status())
                            .unwrap_or_else(|| if self.idle_timeout_seconds > 0 { "Running via background tray service".to_owned() } else { "Inactivity dimming disabled".to_owned() });
                        ui.label(
                            RichText::new(format!("• {timeout_status}"))
                                .size(10.0)
                                .color(if self.idle_timeout_seconds > 0 { crate::theme::PRIMARY_ACCENT } else { crate::theme::MUTED_TEXT }),
                        );
                    });

                    // COLUMN 2: Color Backlight & Spectrum Wheel
                    let col1 = &mut columns[1];
                    crate::theme::card_frame().show(col1, |ui| {
                        ui.set_min_height(255.0);
                        ui.label(
                            RichText::new("COLOR BACKLIGHT")
                                .small()
                                .strong()
                                .color(crate::theme::SECONDARY_TEXT),
                        );
                        ui.add_space(4.0);

                        ui.vertical_centered(|ui| {
                            if components::color_wheel_ring(ui, &mut self.color, 44.0) {
                                changed = true;
                            }
                            ui.add_space(6.0);

                            // 12 Preset Swatches in 2 rows of 6
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                for &swatch in &PRESET_SWATCHES[0..6] {
                                    let is_sel = self.color == swatch;
                                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                                    let painter = ui.painter_at(rect);
                                    painter.rect_filled(rect, 4.0, swatch);
                                    let stroke_col = if is_sel { Color32::WHITE } else if resp.hovered() { crate::theme::PRIMARY_ACCENT } else { crate::theme::BORDER };
                                    let stroke_w = if is_sel { 2.0_f32 } else { 1.0_f32 };
                                    painter.rect_stroke(rect, 4.0, egui::Stroke::new(stroke_w, stroke_col), egui::StrokeKind::Inside);
                                    if resp.clicked() {
                                        self.color = swatch;
                                        changed = true;
                                    }
                                }
                            });
                            ui.add_space(3.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                for &swatch in &PRESET_SWATCHES[6..12] {
                                    let is_sel = self.color == swatch;
                                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                                    let painter = ui.painter_at(rect);
                                    painter.rect_filled(rect, 4.0, swatch);
                                    let stroke_col = if is_sel { Color32::WHITE } else if resp.hovered() { crate::theme::PRIMARY_ACCENT } else { crate::theme::BORDER };
                                    let stroke_w = if is_sel { 2.0_f32 } else { 1.0_f32 };
                                    painter.rect_stroke(rect, 4.0, egui::Stroke::new(stroke_w, stroke_col), egui::StrokeKind::Inside);
                                    if resp.clicked() {
                                        self.color = swatch;
                                        changed = true;
                                    }
                                }
                            });

                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Fine Color:").size(10.0).color(crate::theme::SECONDARY_TEXT));
                                if egui::color_picker::color_edit_button_srgba(ui, &mut self.color, egui::color_picker::Alpha::Opaque).changed() {
                                    changed = true;
                                }
                            });
                        });
                    });

                    // COLUMN 3: Lighting Effects & 5-Position Brightness
                    let col2 = &mut columns[2];
                    crate::theme::card_frame().show(col2, |ui| {
                        ui.set_min_height(255.0);
                        ui.label(
                            RichText::new("LIGHTING & BRIGHTNESS")
                                .small()
                                .strong()
                                .color(crate::theme::SECONDARY_TEXT),
                        );
                        ui.add_space(4.0);

                        ui.label(
                            RichText::new("Dynamic Effect")
                                .strong()
                                .color(crate::theme::PRIMARY_TEXT),
                        );
                        let anim_choice = components::segmented_choice(
                            ui,
                            self.animation_mode,
                            &[
                                (components::AnimationMode::Off, "Static"),
                                (components::AnimationMode::Breathing, "Breathing"),
                                (components::AnimationMode::Rainbow, "Rainbow"),
                            ],
                        );
                        if let Some(mode) = anim_choice {
                            let success = match mode {
                                components::AnimationMode::Off => self.action("Keyboard animation", || libacercontrol::dbus::AcerControlDBus::new()?.stop_animation()),
                                components::AnimationMode::Breathing => self.action("Keyboard animation", || libacercontrol::dbus::AcerControlDBus::new()?.start_breathing()),
                                components::AnimationMode::Rainbow => self.action("Keyboard animation", || libacercontrol::dbus::AcerControlDBus::new()?.start_rainbow()),
                            };
                            if success {
                                self.animation_mode = mode;
                                self.animation_mode_confirmed = true;
                            }
                        }

                        ui.add_space(2.0);
                        let anim_desc = if self.animation_mode_confirmed {
                            match self.animation_mode {
                                components::AnimationMode::Off => "Solid single-zone color active",
                                components::AnimationMode::Breathing => "Breathing pulse animation active",
                                components::AnimationMode::Rainbow => "Rainbow spectrum cycle active",
                            }
                        } else {
                            "Hardware state not confirmed by daemon"
                        };
                        ui.label(RichText::new(anim_desc).size(9.5).color(crate::theme::MUTED_TEXT));

                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("BRIGHTNESS (5 POSITIONS)")
                                .small()
                                .strong()
                                .color(crate::theme::SECONDARY_TEXT),
                        );
                        ui.add_space(2.0);
                        let cur_level = if self.brightness == 0 {
                            0u8
                        } else if self.brightness <= 96 {
                            64u8
                        } else if self.brightness <= 160 {
                            128u8
                        } else if self.brightness <= 224 {
                            192u8
                        } else {
                            255u8
                        };

                        let bright_options = [
                            (0u8, "OFF"),
                            (64u8, "1"),
                            (128u8, "2"),
                            (192u8, "3"),
                            (255u8, "4"),
                        ];
                        if let Some(lvl) = components::segmented_choice(ui, cur_level, &bright_options) {
                            if self.brightness != lvl {
                                self.brightness = lvl;
                                changed = true;
                            }
                        }

                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Duty Cycle:").size(9.5).color(crate::theme::SECONDARY_TEXT));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(format!("{} / 255", self.brightness)).size(9.5).strong().color(crate::theme::PRIMARY_TEXT));
                            });
                        });
                    });
                });

                // Apply changes to daemon
                if changed {
                    let (r, g, b) = (self.color.r(), self.color.g(), self.color.b());
                    let level = self.brightness;
                    if self.action("Keyboard lighting", move || {
                        let bus = libacercontrol::dbus::AcerControlDBus::new()?;
                        bus.set_keyboard_brightness(level)?;
                        bus.set_keyboard_color(r, g, b)
                    }) {
                        if let Some(controller) = &self.idle_controller {
                            controller.set_brightness(level);
                        }
                        self.animation_mode = components::AnimationMode::Off;
                        self.animation_mode_confirmed = true;
                    }
                }

                // Bottom Card: Single-Zone Physical Keyboard Vector Preview
                ui.add_space(12.0);
                crate::theme::card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("LIVE KEYBOARD PREVIEW")
                                .small()
                                .strong()
                                .color(crate::theme::SECONDARY_TEXT),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new("Physical Single-Zone Vector Preview • 16.7M Colors")
                                    .size(9.5)
                                    .color(crate::theme::PRIMARY_ACCENT),
                            );
                        });
                    });
                    ui.add_space(8.0);
                    components::single_zone_keyboard_preview(ui, self.color, self.brightness, self.animation_mode);
                });
            }
            Page::Performance => { ui.label(RichText::new("Performance profiles are planned for a future release.").color(MUTED)); }
            Page::Battery => { ui.label(RichText::new("Battery charge management is planned for a future release.").color(MUTED)); }
            Page::Settings => {
                self.ui_settings(ui);
            }
        }
    }

    pub(crate) fn ui_settings(&mut self, ui: &mut egui::Ui) {
        // 1. Settings & Diagnostics Header
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("SETTINGS & DIAGNOSTICS")
                    .heading()
                    .strong()
                    .color(crate::theme::PRIMARY_TEXT),
            );
            ui.add_space(8.0);
            ui_status_pill(ui, &format!("APP {}", self.about_info.summary.app_version), crate::theme::PRIMARY_ACCENT);
            ui_status_pill(ui, &format!("ABI {}", self.about_info.summary.driver_abi), crate::theme::SECONDARY_TEXT);
        });
        ui.label(
            RichText::new(format!(
                "{} • {}",
                self.about_info.summary.laptop_model,
                self.about_info.summary.os_name
            ))
            .small()
            .color(crate::theme::SECONDARY_TEXT),
        );
        ui.add_space(10.0);

        // 2. 4-Tab Clevo Hardware Console Navigation
        ui.horizontal(|ui| {
            let tabs = [
                "PREFERENCES",
                "SYSTEM INFO",
                "DRIVERS & DAEMON",
                "DIAGNOSTICS",
            ];
            let tab_w = 175.0;
            for (idx, &title) in tabs.iter().enumerate() {
                let is_sel = self.settings_tab == idx;
                let btn = egui::Button::new(
                    RichText::new(title)
                        .small()
                        .strong()
                        .color(if is_sel {
                            crate::theme::PRIMARY_ACCENT
                        } else {
                            crate::theme::SECONDARY_TEXT
                        }),
                )
                .fill(if is_sel {
                    crate::theme::SURFACE_ACTIVE
                } else {
                    crate::theme::SURFACE
                })
                .stroke(if is_sel {
                    egui::Stroke::new(1.5_f32, crate::theme::PRIMARY_ACCENT)
                } else {
                    egui::Stroke::new(1.0_f32, crate::theme::BORDER)
                })
                .corner_radius(crate::theme::RADIUS_MD)
                .min_size(egui::vec2(tab_w, 28.0));

                if ui.add(btn).clicked() {
                    self.settings_tab = idx;
                }
            }
        });
        ui.add_space(12.0);

        // 3. Tab Contents
        match self.settings_tab {
            0 => self.ui_settings_preferences(ui),
            1 => self.ui_settings_sysinfo(ui),
            2 => self.ui_settings_drivers(ui),
            3 => self.ui_settings_diagnostics(ui),
            _ => self.ui_settings_preferences(ui),
        }
    }

    fn ui_settings_preferences(&mut self, ui: &mut egui::Ui) {
        // Card 1: Window & System Tray Behavior
        crate::theme::card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                if self.tray_ready {
                    ui_status_pill(ui, "TRAY ACTIVE", crate::theme::PRIMARY_ACCENT);
                } else {
                    ui_status_pill(ui, "TRAY IDLE", crate::theme::MUTED_TEXT);
                }
                ui.label(
                    RichText::new("WINDOW & SYSTEM TRAY")
                        .small()
                        .strong()
                        .color(crate::theme::PRIMARY_TEXT),
                );
            });
            ui.add_space(8.0);

            let previous = self.minimize_to_tray;
            let mut minimize_to_tray = previous;
            ui.checkbox(&mut minimize_to_tray, "Minimize to system tray when closing the window");
            if !self.tray_ready {
                ui.label(RichText::new("Enabling this starts the background tray service for this session.").weak());
            }
            if minimize_to_tray != previous {
                let result = keyboard_idle::save_minimize_to_tray(minimize_to_tray)
                    .and_then(|()| {
                        keyboard_idle::set_login_autostart(
                            minimize_to_tray || self.idle_timeout_seconds > 0,
                        )
                    });
                match result {
                    Ok(()) => {
                        self.minimize_to_tray = minimize_to_tray;
                        if minimize_to_tray {
                            match instance::start_background_service() {
                                Ok(()) => {
                                    self.tray_ready = true;
                                    self.idle_controller = None;
                                }
                                Err(error) => {
                                    self.set_notice(false, format!("Minimize to tray is saved and will be retried at login, but the tray service could not start: {error}"));
                                }
                            }
                        } else if self.idle_timeout_seconds == 0 {
                            instance::request_background_quit();
                            self.tray_ready = false;
                        }
                        if self.tray_ready || !minimize_to_tray {
                            self.set_notice(true, if minimize_to_tray {
                                "Closing the window will close its window while the background service remains in the system tray.".to_owned()
                            } else {
                                "Closing the window will exit Acer Control Center when no background timeout is active.".to_owned()
                            });
                        }
                    }
                    Err(error) => {
                        let _ = keyboard_idle::save_minimize_to_tray(previous);
                        let _ = keyboard_idle::set_login_autostart(
                            previous || self.idle_timeout_seconds > 0,
                        );
                        self.set_notice(false, format!("Could not save tray setting: {error}"));
                    }
                }
            }
            ui.add_space(4.0);
            ui.label(RichText::new("Login startup stays enabled while tray minimization or keyboard inactivity dimming is on. The background service starts hidden with a tray icon.").small().weak());
        });

        ui.add_space(10.0);

        // Card 2: Keyboard Inactivity & Power
        crate::theme::card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                if self.idle_timeout_seconds > 0 {
                    ui_status_pill(ui, &format!("IDLE DIM {}s", self.idle_timeout_seconds), crate::theme::PRIMARY_ACCENT);
                } else {
                    ui_status_pill(ui, "CONTINUOUS ON", crate::theme::MUTED_TEXT);
                }
                ui.label(
                    RichText::new("KEYBOARD INACTIVITY & POWER")
                        .small()
                        .strong()
                        .color(crate::theme::PRIMARY_TEXT),
                );
            });
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    if self.idle_timeout_seconds > 0 {
                        ui.label(format!("Backlight automatically powers off after {} seconds of input inactivity.", self.idle_timeout_seconds));
                    } else {
                        ui.label("Keyboard backlight remains continuously illuminated while laptop is awake.");
                    }
                    ui.label(RichText::new("Inactivity detection monitors input events via libinput.").small().weak());
                });
                ui.add_space(16.0);
                if ui.button("Open Keyboard RGB Settings >>").clicked() {
                    self.page = Page::Keyboard;
                }
            });
        });

        ui.add_space(10.0);

        // Card 3: Application Updates
        crate::theme::card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui_status_pill(ui, "GITHUB RELEASES", crate::theme::SECONDARY_ACCENT);
                ui.label(
                    RichText::new("APPLICATION UPDATES")
                        .small()
                        .strong()
                        .color(crate::theme::PRIMARY_TEXT),
                );
            });
            ui.add_space(8.0);
            ui.label("Check GitHub for a newer version. Download and installation stay under your control.");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(!self.update_checking, egui::Button::new("Check for updates")).clicked() {
                    self.update_checking = true;
                    self.update_status = "Checking GitHub Releases…".to_owned();
                    self.latest_release = None;
                    let sender = self.update_sender.clone();
                    std::thread::spawn(move || {
                        let _ = sender.send(about::check_latest_release());
                    });
                }
                ui.label(RichText::new(&self.update_status).color(if self.update_checking {
                    crate::theme::SECONDARY_ACCENT
                } else {
                    crate::theme::PRIMARY_TEXT
                }));
            });
            if let Some(release) = &self.latest_release {
                ui.add_space(4.0);
                ui.hyperlink_to(format!("View release {} and downloads (GitHub)", release.tag_name), &release.html_url);
            } else if !self.update_checking
                && self.update_status != "Updates are checked only when you ask."
                && self.update_status != "Checking GitHub Releases…"
            {
                ui.add_space(4.0);
                ui.hyperlink_to("Open project releases on GitHub", about::RELEASES_PAGE_URL);
            }
        });
    }

    fn ui_settings_sysinfo(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |columns| {
            // LEFT COLUMN: Platform Identity & Daemon Stack
            let left = &mut columns[0];
            left.spacing_mut().item_spacing.y = 10.0;

            crate::theme::card_frame().show(left, |ui| {
                ui.label(
                    RichText::new("COMPUTER & OPERATING SYSTEM")
                        .small()
                        .strong()
                        .color(crate::theme::SECONDARY_TEXT),
                );
                ui.add_space(6.0);
                egui::Grid::new("grid-sysinfo-os")
                    .num_columns(2)
                    .spacing([20.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Laptop Model").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new(&self.about_info.summary.laptop_model).small().strong().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();

                        ui.label(RichText::new("Operating System").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new(&self.about_info.summary.os_name).small().strong().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();

                        ui.label(RichText::new("Linux Kernel").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new(&self.about_info.summary.kernel).small().monospace().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();

                        ui.label(RichText::new("CPU Architecture").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new(&self.about_info.summary.arch).small().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();
                    });
            });

            crate::theme::card_frame().show(left, |ui| {
                ui.label(
                    RichText::new("SOFTWARE & DAEMON STACK")
                        .small()
                        .strong()
                        .color(crate::theme::SECONDARY_TEXT),
                );
                ui.add_space(6.0);
                egui::Grid::new("grid-sysinfo-soft")
                    .num_columns(2)
                    .spacing([20.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Control Center App").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new(&self.about_info.summary.app_version).small().strong().color(crate::theme::PRIMARY_ACCENT));
                        ui.end_row();

                        ui.label(RichText::new("Control Daemon").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new(&self.about_info.summary.daemon_status.1).small().strong().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();

                        ui.label(RichText::new("Driver ABI Version").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new(&self.about_info.summary.driver_abi).small().monospace().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();

                        ui.label(RichText::new("Firmware Interface").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new(&self.about_info.summary.interface_name).small().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();
                    });
            });

            // RIGHT COLUMN: Processor, Graphics & Live Telemetry
            let right = &mut columns[1];
            right.spacing_mut().item_spacing.y = 10.0;

            crate::theme::card_frame().show(right, |ui| {
                ui.label(
                    RichText::new("PROCESSOR & GRAPHICS HARDWARE")
                        .small()
                        .strong()
                        .color(crate::theme::SECONDARY_TEXT),
                );
                ui.add_space(6.0);
                egui::Grid::new("grid-sysinfo-hw")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Processor (CPU)").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new(&self.cpu_model).small().strong().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();

                        ui.label(RichText::new("Discrete GPU").small().color(crate::theme::SECONDARY_TEXT));
                        let display_gpu = if self.gpu_model.starts_with("NVIDIA GeForce ") {
                            self.gpu_model.replace("NVIDIA GeForce ", "NVIDIA ")
                        } else {
                            self.gpu_model.clone()
                        };
                        ui.label(RichText::new(&display_gpu).small().strong().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();

                        ui.label(RichText::new("Hardware Control").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(
                            RichText::new(if self.about_info.summary.hardware_supported {
                                "Supported & Active"
                            } else {
                                "Limited / Fallback"
                            })
                            .small()
                            .strong()
                            .color(if self.about_info.summary.hardware_supported {
                                crate::theme::PRIMARY_ACCENT
                            } else {
                                crate::theme::MUTED_TEXT
                            }),
                        );
                        ui.end_row();

                        ui.label(RichText::new("Device Node").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(
                            RichText::new(if self.about_info.summary.device_node_accessible {
                                "/dev/acer_io (rw)"
                            } else {
                                "/dev/acer_io (inaccessible)"
                            })
                            .small()
                            .strong()
                            .color(if self.about_info.summary.device_node_accessible {
                                crate::theme::PRIMARY_ACCENT
                            } else {
                                Color32::from_rgb(0xef, 0x44, 0x44)
                            }),
                        );
                        ui.end_row();
                    });
            });

            crate::theme::card_frame().show(right, |ui| {
                ui.label(
                    RichText::new("LIVE TELEMETRY SENSORS")
                        .small()
                        .strong()
                        .color(crate::theme::SECONDARY_TEXT),
                );
                ui.add_space(6.0);
                let cpu_fan = libacercontrol::telemetry::cpu_fan_rpm()
                    .map(|v| format!("{v} RPM"))
                    .unwrap_or_else(|_| "Unavailable".into());
                let gpu_fan = libacercontrol::telemetry::gpu_fan_rpm()
                    .map(|v| format!("{v} RPM"))
                    .unwrap_or_else(|_| "Unavailable".into());
                let cpu_temp = libacercontrol::telemetry::cpu_temperature()
                    .map(|v| format!("{v} °C"))
                    .unwrap_or_else(|_| "Unavailable".into());
                let gpu_temp = libacercontrol::telemetry::gpu_temperature()
                    .map(|v| format!("{v} °C"))
                    .unwrap_or_else(|_| "Unavailable".into());

                egui::Grid::new("grid-sysinfo-sensors")
                    .num_columns(2)
                    .spacing([20.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("CPU Fan Tachometer").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new(&cpu_fan).small().monospace().color(crate::theme::PRIMARY_ACCENT));
                        ui.end_row();

                        ui.label(RichText::new("GPU Fan Tachometer").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new(&gpu_fan).small().monospace().color(crate::theme::SECONDARY_ACCENT));
                        ui.end_row();

                        ui.label(RichText::new("CPU Thermal Diode").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new(&cpu_temp).small().monospace().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();

                        ui.label(RichText::new("GPU Thermal Diode").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new(&gpu_temp).small().monospace().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();
                    });
            });
        });
    }

    fn ui_settings_drivers(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |columns| {
            // LEFT COLUMN: Daemon and Userspace Character Device
            let left = &mut columns[0];
            left.spacing_mut().item_spacing.y = 10.0;

            // Card 1: Daemon Status
            crate::theme::card_frame().show(left, |ui| {
                ui.horizontal(|ui| {
                    if self.about_info.summary.daemon_status.0 {
                        ui_status_pill(ui, "RUNNING", crate::theme::PRIMARY_ACCENT);
                    } else {
                        ui_status_pill(ui, "STOPPED", Color32::from_rgb(0xef, 0x44, 0x44));
                    }
                    ui.label(
                        RichText::new("ACER CONTROL DAEMON (io.github.stanlysilas.AcerControl)")
                            .small()
                            .strong()
                            .color(crate::theme::PRIMARY_TEXT),
                    );
                });
                ui.add_space(8.0);
                egui::Grid::new("grid-daemon-info")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("System D-Bus Service").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new("io.github.stanlysilas.AcerControl").small().monospace().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();

                        ui.label(RichText::new("D-Bus Object Path").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new("/io/github/stanlysilas/AcerControl").small().monospace().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();

                        ui.label(RichText::new("Systemd Service Unit").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new("acer-control-daemon.service").small().monospace().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();

                        ui.label(RichText::new("Connection Status").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(
                            RichText::new(if self.about_info.summary.daemon_status.0 {
                                "Connected • Synchronized"
                            } else {
                                "Daemon unreachable"
                            })
                            .small()
                            .strong()
                            .color(if self.about_info.summary.daemon_status.0 {
                                crate::theme::PRIMARY_ACCENT
                            } else {
                                Color32::from_rgb(0xef, 0x44, 0x44)
                            }),
                        );
                        ui.end_row();
                    });
            });

            // Card 2: Kernel Character Device Node
            crate::theme::card_frame().show(left, |ui| {
                ui.horizontal(|ui| {
                    if self.about_info.summary.device_node_accessible {
                        ui_status_pill(ui, "ACCESSIBLE (rw)", crate::theme::PRIMARY_ACCENT);
                    } else if std::path::Path::new("/dev/acer_io").exists() {
                        ui_status_pill(ui, "PERMISSION DENIED", Color32::from_rgb(0xf5, 0x9e, 0x0b));
                    } else {
                        ui_status_pill(ui, "MISSING", Color32::from_rgb(0xef, 0x44, 0x44));
                    }
                    ui.label(
                        RichText::new("CHARACTER DEVICE (/dev/acer_io)")
                            .small()
                            .strong()
                            .color(crate::theme::PRIMARY_TEXT),
                    );
                });
                ui.add_space(8.0);
                egui::Grid::new("grid-chardev-info")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Device Node Path").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(RichText::new("/dev/acer_io").small().monospace().color(crate::theme::PRIMARY_TEXT));
                        ui.end_row();

                        ui.label(RichText::new("Driver ABI").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(
                            RichText::new(format!(
                                "Major {}, Minor {}",
                                libacercontrol::driver::abi_major(),
                                libacercontrol::driver::abi_minor()
                            ))
                            .small()
                            .monospace()
                            .color(crate::theme::PRIMARY_TEXT),
                        );
                        ui.end_row();

                        ui.label(RichText::new("Access Permissions").small().color(crate::theme::SECONDARY_TEXT));
                        ui.label(
                            RichText::new(if self.about_info.summary.device_node_accessible {
                                "Direct ioctl access verified (group 'acercontrol')"
                            } else if std::path::Path::new("/dev/acer_io").exists() {
                                "Permission denied (add user to 'acercontrol')"
                            } else {
                                "Node missing (ensure acer_io module is loaded)"
                            })
                            .small()
                            .strong()
                            .color(if self.about_info.summary.device_node_accessible {
                                crate::theme::PRIMARY_ACCENT
                            } else {
                                Color32::from_rgb(0xf5, 0x9e, 0x0b)
                            }),
                        );
                        ui.end_row();
                    });
            });

            // RIGHT COLUMN: Kernel Driver Modules Matrix
            let right = &mut columns[1];
            crate::theme::card_frame().show(right, |ui| {
                ui.label(
                    RichText::new("KERNEL DRIVER MODULES MATRIX")
                        .small()
                        .strong()
                        .color(crate::theme::SECONDARY_TEXT),
                );
                ui.add_space(8.0);

                for module in &self.about_info.summary.modules {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            if module.loaded {
                                let label = module
                                    .version
                                    .as_ref()
                                    .map(|v| format!("LOADED v{v}"))
                                    .unwrap_or_else(|| "LOADED".to_owned());
                                ui_status_pill(ui, &label, crate::theme::PRIMARY_ACCENT);
                            } else {
                                ui_status_pill(ui, "NOT LOADED", crate::theme::MUTED_TEXT);
                            }
                            ui.label(
                                RichText::new(&module.name)
                                    .small()
                                    .monospace()
                                    .strong()
                                    .color(crate::theme::PRIMARY_TEXT),
                            );
                        });
                        ui.add(
                            egui::Label::new(
                                RichText::new(&module.description)
                                    .small()
                                    .color(crate::theme::SECONDARY_TEXT),
                            )
                            .wrap(),
                        );
                    });
                    ui.separator();
                }
            });
        });
    }

    fn ui_settings_diagnostics(&mut self, ui: &mut egui::Ui) {
        // 1. Diagnostics Header & Overall Health Summary Pill
        let mut passed_count = 0;
        let mut warning_count = 0;
        let mut failure_count = 0;
        for check in &self.about_info.summary.diagnostics {
            match check.status {
                about::DiagnosticStatus::Passed => passed_count += 1,
                about::DiagnosticStatus::Warning => warning_count += 1,
                about::DiagnosticStatus::Failed => failure_count += 1,
                about::DiagnosticStatus::Info => (),
            }
        }

        ui.horizontal(|ui| {
            if failure_count == 0 && warning_count == 0 {
                ui_status_pill(ui, "HEALTHY", crate::theme::PRIMARY_ACCENT);
                ui.label(
                    RichText::new(format!("All Subsystems Operational ({passed_count}/{} Checks Passed)", self.about_info.summary.diagnostics.len()))
                        .small()
                        .strong()
                        .color(crate::theme::PRIMARY_TEXT),
                );
            } else if failure_count == 0 {
                ui_status_pill(ui, "ADVISORY", Color32::from_rgb(0xf5, 0x9e, 0x0b));
                ui.label(
                    RichText::new(format!("System Operational with {warning_count} Advisory Notice(s)"))
                        .small()
                        .strong()
                        .color(Color32::from_rgb(0xf5, 0x9e, 0x0b)),
                );
            } else {
                ui_status_pill(ui, "ATTENTION", Color32::from_rgb(0xef, 0x44, 0x44));
                ui.label(
                    RichText::new(format!("Critical Attention Required ({failure_count} Failure(s))"))
                        .small()
                        .strong()
                        .color(Color32::from_rgb(0xef, 0x44, 0x44)),
                );
            }

            ui.add_space(20.0);
            if ui.button("Refresh Diagnostics").clicked() {
                self.about_info = about::AboutInfo::collect();
            }
        });
        ui.add_space(10.0);

        // 2. Diagnostic Cards List (2-column layout: 3 checks on left, 3 checks on right)
        ui.columns(2, |columns| {
            let mid = (self.about_info.summary.diagnostics.len() + 1) / 2;
            let left_checks = &self.about_info.summary.diagnostics[..mid];
            let right_checks = &self.about_info.summary.diagnostics[mid..];

            let left = &mut columns[0];
            left.spacing_mut().item_spacing.y = 6.0;
            for check in left_checks {
                ui_render_diagnostic_card(left, check);
            }

            let right = &mut columns[1];
            right.spacing_mut().item_spacing.y = 6.0;
            for check in right_checks {
                ui_render_diagnostic_card(right, check);
            }
        });
    }
}

fn ui_render_diagnostic_card(ui: &mut egui::Ui, check: &about::DiagnosticCheck) {
    crate::theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            match check.status {
                about::DiagnosticStatus::Passed => {
                    ui_status_pill(ui, "PASS", crate::theme::PRIMARY_ACCENT);
                }
                about::DiagnosticStatus::Warning => {
                    ui_status_pill(ui, "WARN", Color32::from_rgb(0xf5, 0x9e, 0x0b));
                }
                about::DiagnosticStatus::Failed => {
                    ui_status_pill(ui, "FAIL", Color32::from_rgb(0xef, 0x44, 0x44));
                }
                about::DiagnosticStatus::Info => {
                    ui_status_pill(ui, "INFO", crate::theme::SECONDARY_ACCENT);
                }
            }
            ui.label(
                RichText::new(&check.name)
                    .small()
                    .strong()
                    .color(crate::theme::PRIMARY_TEXT),
            );
        });
        ui.add_space(2.0);
        ui.add(
            egui::Label::new(
                RichText::new(&check.description)
                    .small()
                    .color(crate::theme::SECONDARY_TEXT),
            )
            .wrap(),
        );
        ui.add_space(4.0);
        egui::Frame::new()
            .fill(crate::theme::SURFACE_HOVER)
            .corner_radius(crate::theme::RADIUS_SM)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(&check.detail)
                            .small()
                            .monospace()
                            .color(crate::theme::PRIMARY_TEXT),
                    )
                    .wrap(),
                );
            });
    });
}

fn ui_status_pill(ui: &mut egui::Ui, label: &str, color: Color32) {
    egui::Frame::new()
        .fill(crate::theme::SURFACE_ACTIVE)
        .stroke(egui::Stroke::new(1.0_f32, color))
        .corner_radius(crate::theme::RADIUS_SM)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(label)
                    .small()
                    .strong()
                    .color(color),
            );
        });
}
