use eframe::{
    egui::{self, Color32},
    egui_glow,
    glow::{self, HasContext},
};
use glam::{Mat4, Vec3};
use orbbec_v1::PointCloud;
use std::sync::{Arc, Mutex};

pub struct CloudView {
    gpu: Arc<Mutex<PointRenderer>>,
    orbit: Orbit,
    bounds: Option<[Vec3; 2]>,
    pub point_size: f32,
    pub stride: usize,
    pub frozen: bool,
    pub visible_points: usize,
    dirty: bool,
    fit_pending: bool,
    frozen_cloud: Option<Arc<PointCloud>>,
}

impl CloudView {
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        Ok(Self {
            gpu: Arc::new(Mutex::new(PointRenderer::new(gl)?)),
            orbit: Orbit::default(),
            bounds: None,
            point_size: 2.0,
            stride: 2,
            frozen: false,
            visible_points: 0,
            dirty: true,
            fit_pending: true,
            frozen_cloud: None,
        })
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        if ui.button("Fit view").clicked() {
            self.fit_pending = true;
        }
        ui.checkbox(&mut self.frozen, "Freeze cloud");
        ui.add(egui::Slider::new(&mut self.point_size, 1.0..=6.0).text("Point size"));
        self.dirty |= ui
            .add(egui::Slider::new(&mut self.stride, 1..=6).text("Pixel step"))
            .changed();
        ui.label(format!("{} points", self.visible_points));
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        cloud: Option<&Arc<PointCloud>>,
        new_frame: bool,
        near: f32,
        far: f32,
        range_changed: bool,
    ) {
        if self.frozen {
            if self.frozen_cloud.is_none() {
                self.frozen_cloud = cloud.cloned();
            }
        } else {
            self.frozen_cloud = None;
        }
        let cloud = self.frozen_cloud.as_ref().or(cloud);
        if cloud.is_none() && new_frame && !self.frozen {
            self.visible_points = 0;
            self.bounds = None;
            self.gpu.lock().unwrap().pending = Some(Vec::new());
        }
        if let Some(cloud) = cloud
            && ((!self.frozen && new_frame)
                || self.dirty
                || range_changed
                || self.visible_points == 0)
        {
            let (vertices, bounds) = vertices(cloud, self.stride, near, far);
            self.visible_points = vertices.len();
            self.bounds = bounds;
            self.gpu.lock().unwrap().pending = Some(vertices);
            self.dirty = false;
        }
        if self.fit_pending
            && let Some(bounds) = self.bounds
        {
            self.orbit.fit(bounds);
            self.fit_pending = false;
        }
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        if response.hovered() || response.dragged() {
            let (delta, scroll, zoom, shift, fit) = ui.input(|i| {
                (
                    i.pointer.delta(),
                    i.smooth_scroll_delta.y,
                    i.zoom_delta(),
                    i.modifiers.shift,
                    i.key_pressed(egui::Key::F),
                )
            });
            if response.dragged_by(egui::PointerButton::Middle)
                || response.dragged_by(egui::PointerButton::Secondary)
                || (shift && response.dragged_by(egui::PointerButton::Primary))
            {
                self.orbit.pan(delta, rect.height());
            } else if response.dragged_by(egui::PointerButton::Primary) {
                self.orbit.yaw -= delta.x * 0.007;
                self.orbit.pitch = (self.orbit.pitch + delta.y * 0.007).clamp(-1.5, 1.5);
            }
            self.orbit.zoom((-scroll * 0.002).exp() / zoom);
            if fit || response.double_clicked() {
                self.fit_pending = true;
            }
        }
        let matrix = self.orbit.matrix(rect.aspect_ratio()).to_cols_array();
        let point_size = self.point_size * ui.ctx().pixels_per_point();
        let gpu = self.gpu.clone();
        ui.painter().add(egui::PaintCallback {
            rect,
            callback: Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                gpu.lock()
                    .unwrap()
                    .paint(painter.gl(), &matrix, point_size, near, far);
            })),
        });
        ui.painter().text(
            rect.left_bottom() + egui::vec2(12.0, -12.0),
            egui::Align2::LEFT_BOTTOM,
            "Drag: orbit  ·  Scroll: zoom  ·  Right / Shift-drag: pan  ·  F: fit  ·  Units: meters",
            egui::FontId::proportional(12.0),
            Color32::from_gray(190),
        );
        if self.visible_points == 0 {
            let message = if cloud.is_some() {
                "No valid points in range · adjust Near/Far or aim at a farther scene"
            } else {
                "Waiting for calibrated depth points…"
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                message,
                egui::FontId::proportional(16.0),
                Color32::LIGHT_GRAY,
            );
        }
    }

    pub fn destroy(&self, gl: &glow::Context) {
        self.gpu.lock().unwrap().destroy(gl);
    }

    pub fn reset(&mut self) {
        self.frozen = false;
        self.frozen_cloud = None;
        self.visible_points = 0;
        self.bounds = None;
        self.fit_pending = true;
        self.dirty = true;
        self.gpu.lock().unwrap().pending = Some(Vec::new());
    }
}

struct Orbit {
    target: Vec3,
    distance: f32,
    yaw: f32,
    pitch: f32,
}

impl Default for Orbit {
    fn default() -> Self {
        Self {
            target: Vec3::new(0.0, 0.0, -2.0),
            distance: 3.0,
            yaw: 0.2,
            pitch: 0.12,
        }
    }
}

impl Orbit {
    fn fit(&mut self, [min, max]: [Vec3; 2]) {
        self.target = (min + max) * 0.5;
        self.distance = ((max - min).length() * 0.5 / (45.0_f32.to_radians() * 0.5).tan() * 1.2)
            .clamp(0.2, 50.0);
        self.yaw = 0.2;
        self.pitch = 0.12;
    }
    fn eye(&self) -> Vec3 {
        self.target
            + Vec3::new(
                self.yaw.sin() * self.pitch.cos(),
                self.pitch.sin(),
                self.yaw.cos() * self.pitch.cos(),
            ) * self.distance
    }
    fn matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh_gl(45.0_f32.to_radians(), aspect.max(0.01), 0.01, 150.0)
            * Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }
    fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(0.05, 100.0);
    }
    fn pan(&mut self, delta: egui::Vec2, height: f32) {
        let forward = (self.target - self.eye()).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();
        let scale = 2.0 * self.distance * (45.0_f32.to_radians() * 0.5).tan() / height.max(1.0);
        self.target += (right * -delta.x + up * delta.y) * scale;
    }
}

fn vertices(
    cloud: &PointCloud,
    step: usize,
    near: f32,
    far: f32,
) -> (Vec<[f32; 3]>, Option<[Vec3; 2]>) {
    let width = cloud.width as usize;
    let mut output = Vec::new();
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    if width == 0 || width.checked_mul(cloud.height as usize) != Some(cloud.points.len()) {
        return (output, None);
    }
    let step = step.max(1);
    for y in (0..cloud.height as usize).step_by(step) {
        for x in (0..width).step_by(step) {
            let [x, y, z] = cloud.points[y * width + x];
            if ![x, y, z].iter().all(|v| v.is_finite()) || z <= 0.0 || z < near || z > far {
                continue;
            }
            // SDK camera: X right, Y down, Z forward. OpenGL world: Y up,
            // camera looks along -Z. This is a rotation, not a mirrored cloud.
            let point = Vec3::new(x, -y, -z);
            min = min.min(point);
            max = max.max(point);
            output.push(point.to_array());
        }
    }
    let bounds = (!output.is_empty()).then_some([min, max]);
    (output, bounds)
}

struct PointRenderer {
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    matrix: glow::UniformLocation,
    size: glow::UniformLocation,
    range: glow::UniformLocation,
    count: i32,
    desktop: bool,
    pending: Option<Vec<[f32; 3]>>,
}

impl PointRenderer {
    fn new(gl: &glow::Context) -> Result<Self, String> {
        let desktop = !gl.version().is_embedded;
        let prefix = if desktop {
            "#version 330 core\n"
        } else {
            "#version 300 es\nprecision highp float;\n"
        };
        // SAFETY: Initialization runs on eframe's rendering thread while its GL
        // context is current. All buffers/programs are owned until on_exit.
        unsafe {
            let program = gl.create_program()?;
            for (kind, source) in [
                (glow::VERTEX_SHADER, VERTEX),
                (glow::FRAGMENT_SHADER, FRAGMENT),
            ] {
                let shader = gl.create_shader(kind)?;
                gl.shader_source(shader, &format!("{prefix}{source}"));
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    let message = gl.get_shader_info_log(shader);
                    gl.delete_shader(shader);
                    gl.delete_program(program);
                    return Err(message);
                }
                gl.attach_shader(program, shader);
                gl.delete_shader(shader);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                let message = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(message);
            }
            let vao = gl.create_vertex_array()?;
            let vbo = gl.create_buffer()?;
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 12, 0);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            Ok(Self {
                program,
                vao,
                vbo,
                desktop,
                count: 0,
                pending: None,
                matrix: gl
                    .get_uniform_location(program, "u_matrix")
                    .ok_or("Missing point cloud matrix uniform")?,
                size: gl
                    .get_uniform_location(program, "u_size")
                    .ok_or("Missing point cloud size uniform")?,
                range: gl
                    .get_uniform_location(program, "u_range")
                    .ok_or("Missing point cloud range uniform")?,
            })
        }
    }

    fn paint(&mut self, gl: &glow::Context, matrix: &[f32; 16], size: f32, near: f32, far: f32) {
        // SAFETY: egui invokes this callback with the owning GL context current
        // and the viewport/scissor set to the cloud rectangle. Only owned GPU
        // buffers and initialized float arrays are passed to OpenGL.
        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            if let Some(points) = self.pending.take() {
                self.count = points.len().min(i32::MAX as usize) as i32;
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    bytemuck::cast_slice(&points),
                    glow::DYNAMIC_DRAW,
                );
            }
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.depth_mask(true);
            gl.clear_depth_f32(1.0);
            gl.clear(glow::DEPTH_BUFFER_BIT);
            gl.disable(glow::BLEND);
            if self.desktop {
                gl.enable(glow::PROGRAM_POINT_SIZE);
            }
            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(Some(&self.matrix), false, matrix);
            gl.uniform_1_f32(Some(&self.size), size);
            gl.uniform_2_f32(Some(&self.range), near, far);
            gl.draw_arrays(glow::POINTS, 0, self.count);
            if self.desktop {
                gl.disable(glow::PROGRAM_POINT_SIZE);
            }
            gl.depth_mask(false);
            gl.disable(glow::DEPTH_TEST);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            // egui_glow restores its shader, vertex array, blend and viewport.
        }
    }

    fn destroy(&self, gl: &glow::Context) {
        // SAFETY: on_exit runs on the rendering thread with its GL context current.
        unsafe {
            gl.delete_buffer(self.vbo);
            gl.delete_vertex_array(self.vao);
            gl.delete_program(self.program);
        }
    }
}

const VERTEX: &str = r#"
layout(location = 0) in vec3 a_position;
uniform mat4 u_matrix;
uniform float u_size;
out float v_depth;
void main() {
    gl_Position = u_matrix * vec4(a_position, 1.0);
    gl_PointSize = u_size;
    v_depth = -a_position.z;
}
"#;

const FRAGMENT: &str = r#"
uniform vec2 u_range;
in float v_depth;
out vec4 out_color;
void main() {
    if (v_depth < u_range.x || v_depth > u_range.y || length(gl_PointCoord - vec2(0.5)) > 0.5) discard;
    float t = clamp((v_depth - u_range.x) / (u_range.y - u_range.x), 0.0, 1.0) * 4.0;
    vec3 a = vec3(244.0, 76.0, 68.0);
    vec3 b = vec3(255.0, 206.0, 80.0);
    vec3 c = vec3(56.0, 208.0, 153.0);
    vec3 d = vec3(43.0, 182.0, 219.0);
    vec3 e = vec3(66.0, 91.0, 210.0);
    vec3 rgb = t < 1.0 ? mix(a, b, t) : t < 2.0 ? mix(b, c, t - 1.0) : t < 3.0 ? mix(c, d, t - 2.0) : mix(d, e, t - 3.0);
    out_color = vec4(rgb / 255.0, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloud_selection_filters_invalid_points_and_keeps_metric_orientation() {
        let cloud = PointCloud {
            width: 3,
            height: 1,
            points: vec![[1.0, 2.0, 3.0], [0.0; 3], [0.0, 0.0, 10.0]],
        };
        let (points, bounds) = vertices(&cloud, 1, 0.3, 4.0);
        assert_eq!(points, [[1.0, -2.0, -3.0]]);
        assert_eq!(bounds, Some([Vec3::new(1.0, -2.0, -3.0); 2]));
        assert!(vertices(&cloud, 1, 20.0, 30.0).0.is_empty());
    }
    #[test]
    fn fitted_orbit_centers_the_cloud_and_clamps_zoom() {
        let mut orbit = Orbit::default();
        orbit.fit([Vec3::new(-1.0, -1.0, -3.0), Vec3::new(1.0, 1.0, -1.0)]);
        let projected = orbit.matrix(1.5).project_point3(orbit.target);
        assert!(projected.x.abs() < 0.0001 && projected.y.abs() < 0.0001);
        assert!((-1.0..1.0).contains(&projected.z));
        orbit.zoom(0.0);
        assert_eq!(orbit.distance, 0.05);
        orbit.zoom(1e9);
        assert_eq!(orbit.distance, 100.0);
    }
}
