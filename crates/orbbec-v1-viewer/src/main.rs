mod camera;
mod point_cloud;
mod render;

use camera::{CameraState, Capture, Worker};
use eframe::egui::{self, Color32, TextureHandle, TextureOptions, Vec2};
use point_cloud::CloudView;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Depth,
    Color,
    Infrared,
    PointCloud,
}

struct Viewer {
    state: Arc<Mutex<CameraState>>,
    worker: Option<Worker>,
    serial: Option<String>,
    capture: Option<Capture>,
    texture: Option<TextureHandle>,
    cloud: Option<CloudView>,
    cloud_error: Option<String>,
    view: View,
    near_m: f32,
    far_m: f32,
    message: String,
    device: String,
    streams: String,
    error: bool,
    smoke_path: Option<PathBuf>,
    smoke_requested: bool,
    smoke_success: Arc<AtomicBool>,
    started: Instant,
}

impl Viewer {
    fn new(
        cc: &eframe::CreationContext<'_>,
        serial: Option<String>,
        view: View,
        smoke_path: Option<PathBuf>,
        smoke_success: Arc<AtomicBool>,
    ) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        cc.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(12.0, 10.0);
            style.visuals.selection.bg_fill = Color32::from_rgb(26, 106, 100);
            style.visuals.panel_fill = Color32::from_rgb(23, 27, 32);
        });
        let (cloud, cloud_error) = match cc.gl.as_ref() {
            Some(gl) => match CloudView::new(gl) {
                Ok(cloud) => (Some(cloud), None),
                Err(error) => (
                    None,
                    Some(format!("Cannot initialize 3D renderer: {error}")),
                ),
            },
            None => (None, Some("Point cloud requires an OpenGL renderer".into())),
        };
        let state = Arc::new(Mutex::new(CameraState {
            want_point_cloud: view == View::PointCloud,
            ..Default::default()
        }));
        let worker = Worker::spawn(cc.egui_ctx.clone(), state.clone(), serial.clone());
        Self {
            state,
            worker: Some(worker),
            serial,
            capture: None,
            texture: None,
            cloud,
            cloud_error,
            view,
            near_m: 0.3,
            far_m: 4.0,
            message: "Connecting…".into(),
            device: String::new(),
            streams: String::new(),
            error: false,
            smoke_path,
            smoke_requested: false,
            smoke_success,
            started: Instant::now(),
        }
    }

    fn receive(&mut self) -> bool {
        let mut state = self.state.lock().unwrap();
        self.message.clone_from(&state.message);
        self.device.clone_from(&state.device);
        self.streams.clone_from(&state.streams);
        self.error = state.error;
        if state.cloud_error.is_some() {
            self.cloud_error.clone_from(&state.cloud_error);
        }
        let capture = state.latest.take();
        drop(state);
        if let Some(capture) = capture {
            self.capture = Some(capture);
            return true;
        }
        false
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
        let Some(capture) = &self.capture else {
            return;
        };
        let image = match self.view {
            View::Depth => Some(render::depth_image(
                &capture.depth,
                capture.scale_mm,
                self.near_m,
                self.far_m,
            )),
            View::Color => capture
                .color
                .as_ref()
                .map(|plane| egui::ColorImage::from_rgb(plane.size, &plane.pixels)),
            View::Infrared => capture.infrared.as_ref().map(render::infrared_image),
            View::PointCloud => None,
        };
        if let Some(image) = image {
            if let Some(texture) = &mut self.texture {
                texture.set(image, TextureOptions::LINEAR);
            } else {
                self.texture =
                    Some(ctx.load_texture("camera-preview", image, TextureOptions::LINEAR));
            }
        } else {
            self.texture = None;
        }
    }

    fn toolbar(&mut self, ctx: &egui::Context) -> bool {
        let mut changed = false;
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("Orbbec Viewer");
                ui.label(egui::RichText::new("SDK v1").small().weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(worker) = &self.worker {
                        if ui
                            .add_enabled(
                                !worker.stopping(),
                                egui::Button::new(if worker.stopping() {
                                    "Stopping…"
                                } else {
                                    "Stop"
                                }),
                            )
                            .clicked()
                        {
                            worker.stop();
                        }
                    } else if ui.button("▶ Start").clicked() {
                        *self.state.lock().unwrap() = CameraState::default();
                        self.capture = None;
                        self.texture = None;
                        if let Some(cloud) = &mut self.cloud {
                            cloud.reset();
                            self.cloud_error = None;
                        }
                        self.worker = Some(Worker::spawn(
                            ctx.clone(),
                            self.state.clone(),
                            self.serial.clone(),
                        ));
                    }
                });
            });
            if !self.device.is_empty() {
                ui.label(egui::RichText::new(&self.device).small().weak());
            }
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                changed |= ui
                    .selectable_value(&mut self.view, View::Depth, "Depth")
                    .changed();
                if ui
                    .add_enabled(
                        self.capture.as_ref().is_some_and(|c| c.color.is_some()),
                        egui::Button::selectable(self.view == View::Color, "RGB"),
                    )
                    .clicked()
                {
                    self.view = View::Color;
                    changed = true;
                }
                if ui
                    .add_enabled(
                        self.capture.as_ref().is_some_and(|c| c.infrared.is_some()),
                        egui::Button::selectable(self.view == View::Infrared, "Infrared"),
                    )
                    .clicked()
                {
                    self.view = View::Infrared;
                    changed = true;
                }
                ui.separator();
                changed |= ui
                    .selectable_value(&mut self.view, View::PointCloud, "Point cloud")
                    .changed();
                ui.separator();
                let color = if self.error {
                    Color32::from_rgb(255, 120, 110)
                } else {
                    Color32::from_rgb(88, 210, 174)
                };
                ui.colored_label(color, &self.message);
            });
            if matches!(self.view, View::Depth | View::PointCloud) {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Near");
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut self.near_m, 0.1..=self.far_m - 0.1)
                                .suffix(" m"),
                        )
                        .changed();
                    ui.label("Far");
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut self.far_m, self.near_m + 0.1..=10.0)
                                .suffix(" m"),
                        )
                        .changed();
                    ui.label(
                        egui::RichText::new("Warm = near · cool = far · black = invalid")
                            .small()
                            .weak(),
                    );
                });
            }
            if self.view == View::PointCloud
                && let Some(cloud) = &mut self.cloud
            {
                ui.horizontal_wrapped(|ui| cloud.controls(ui));
            }
            ui.add_space(4.0);
        });
        changed
    }

    fn preview(&mut self, ctx: &egui::Context, new_frame: bool, controls_changed: bool) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(10, 13, 17))
                    .inner_margin(12.0),
            )
            .show(ctx, |ui| {
                if self.view == View::PointCloud {
                    if let Some(error) = &self.cloud_error {
                        ui.centered_and_justified(|ui| {
                            ui.colored_label(Color32::LIGHT_RED, error);
                        });
                    } else if let Some(cloud) = &mut self.cloud {
                        let points = self
                            .capture
                            .as_ref()
                            .and_then(|capture| capture.point_cloud.as_ref());
                        cloud.show(
                            ui,
                            points,
                            new_frame,
                            self.near_m,
                            self.far_m,
                            controls_changed,
                        );
                    }
                } else if let Some(texture) = &self.texture {
                    let source = texture.size_vec2();
                    let available = ui.available_size();
                    let size = source
                        * (available.x / source.x)
                            .min(available.y / source.y)
                            .max(0.01);
                    ui.centered_and_justified(|ui| {
                        let response = ui.add(
                            egui::Image::new(texture)
                                .fit_to_exact_size(size)
                                .sense(egui::Sense::hover()),
                        );
                        if self.view == View::Depth {
                            let center = response.rect.center();
                            let stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
                            ui.painter().line_segment(
                                [center - Vec2::new(8.0, 0.0), center + Vec2::new(8.0, 0.0)],
                                stroke,
                            );
                            ui.painter().line_segment(
                                [center - Vec2::new(0.0, 8.0), center + Vec2::new(0.0, 8.0)],
                                stroke,
                            );
                            if let (Some(position), Some(capture)) =
                                (response.hover_pos(), &self.capture)
                            {
                                let [width, height] = capture.depth.size;
                                let point = (position - response.rect.min) / response.rect.size();
                                let x = ((point.x * width as f32) as usize).min(width - 1);
                                let y = ((point.y * height as f32) as usize).min(height - 1);
                                let raw = capture.depth.pixels[y * width + x];
                                response.on_hover_text(if raw == 0 {
                                    format!("({x}, {y}) · invalid depth")
                                } else {
                                    format!(
                                        "({x}, {y}) · {:.0} mm",
                                        f32::from(raw) * capture.scale_mm
                                    )
                                });
                            }
                        }
                    });
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label(if self.error {
                            &self.message
                        } else {
                            "Waiting for camera frames…"
                        });
                    });
                }
            });
    }

    fn smoke_test(&mut self, ctx: &egui::Context) {
        if self.smoke_path.is_none() {
            return;
        }
        let screenshots = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        });
        for screenshot in screenshots {
            let path = self.smoke_path.as_ref().unwrap();
            let save = || -> Result<(), Box<dyn std::error::Error>> {
                if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent)?;
                }
                let rgba: Vec<u8> = screenshot
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                image::save_buffer(
                    path,
                    &rgba,
                    screenshot.size[0] as u32,
                    screenshot.size[1] as u32,
                    image::ColorType::Rgba8,
                )?;
                Ok(())
            };
            match save() {
                Ok(()) => {
                    self.smoke_success.store(true, Ordering::Relaxed);
                    eprintln!("Viewer smoke test passed; screenshot: {}", path.display());
                }
                Err(error) => eprintln!("Screenshot failed: {error}"),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let preview_ready = if self.view == View::PointCloud {
            self.cloud
                .as_ref()
                .is_some_and(|cloud| cloud.visible_points > 0)
        } else {
            self.texture.is_some()
        };
        if !self.smoke_requested
            && preview_ready
            && self.capture.as_ref().is_some_and(|c| c.count >= 30)
        {
            self.smoke_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        if self.started.elapsed() > Duration::from_secs(20) {
            eprintln!("Viewer smoke test timed out: {}", self.message);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for Viewer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let new_capture = self.receive();
        if self.worker.as_ref().is_some_and(Worker::finished) {
            self.worker = None;
        }
        let changed = self.toolbar(ctx);
        self.state.lock().unwrap().want_point_cloud =
            self.view == View::PointCloud && self.cloud.is_some();
        if new_capture || changed {
            self.update_texture(ctx);
        }
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(capture) = &self.capture {
                    ui.label(format!("{:.1} FPS", capture.fps));
                    ui.separator();
                    ui.label(format!("Frame {}", capture.frame_index));
                    ui.separator();
                    let [width, height] = capture.depth.size;
                    let raw = capture.depth.pixels[(height / 2) * width + width / 2];
                    ui.label(if raw == 0 {
                        "Center: invalid depth".into()
                    } else {
                        format!("Center: {:.0} mm", f32::from(raw) * capture.scale_mm)
                    });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.small("RGB / depth exposures are not synchronized");
                });
            });
            if !self.streams.is_empty() {
                ui.small(&self.streams);
            }
        });
        self.preview(ctx, new_capture, changed);
        self.smoke_test(ctx);
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        if let Some(worker) = &self.worker {
            worker.stop();
        }
        if let (Some(cloud), Some(gl)) = (&self.cloud, gl) {
            cloud.destroy(gl);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut serial = None;
    let mut view = View::Depth;
    let mut smoke_path = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--view" => {
                view = match args.next().as_deref() {
                    Some("depth") => View::Depth,
                    Some("rgb") => View::Color,
                    Some("ir") => View::Infrared,
                    Some("cloud") => View::PointCloud,
                    _ => return Err("--view must be depth, rgb, ir, or cloud".into()),
                }
            }
            "--serial" => serial = Some(args.next().ok_or("--serial requires a camera serial")?),
            "--smoke-test" => {
                smoke_path = Some(PathBuf::from(
                    args.next()
                        .ok_or("--smoke-test requires a screenshot PNG path")?,
                ))
            }
            "--help" | "-h" => {
                println!(
                    "orbbec-v1-viewer [--serial SERIAL] [--view depth|rgb|ir|cloud] [--smoke-test FILE.png]"
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    let smoke = smoke_path.is_some();
    let success = Arc::new(AtomicBool::new(false));
    let app_success = success.clone();
    eframe::run_native(
        "Orbbec Viewer",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1100.0, 800.0])
                .with_min_inner_size([720.0, 500.0]),
            renderer: eframe::Renderer::Glow,
            depth_buffer: 24,
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(Viewer::new(
                cc,
                serial,
                view,
                smoke_path,
                app_success,
            )))
        }),
    )?;
    if smoke && !success.load(Ordering::Relaxed) {
        return Err("Viewer smoke test failed before a screenshot was saved".into());
    }
    Ok(())
}
