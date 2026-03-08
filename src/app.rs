use glam::{Mat4, Vec3, Vec4};
use shared::PointCloudUniforms;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

use super::data::PointCloud;
use super::render::{Graphics, Rc, create_graphics};

enum State {
    Ready(Graphics),
    Init(Option<EventLoopProxy<Graphics>>),
}

pub struct App {
    state: State,
    time: f32,
    camera_phi: f32,   // Azimuthal angle (radians)
    camera_theta: f32, // Polar angle (radians)
    camera_distance: f32,
    last_mouse_x: f32,
    last_mouse_y: f32,
    mouse_pressed: bool,
}

impl App {
    pub fn new(event_loop: &EventLoop<Graphics>) -> Self {
        Self {
            state: State::Init(Some(event_loop.create_proxy())),
            time: 0.0,
            camera_phi: 0.5,
            camera_theta: std::f32::consts::PI / 3.0,
            camera_distance: 2400.0,
            last_mouse_x: 0.0,
            last_mouse_y: 0.0,
            mouse_pressed: false,
        }
    }

    fn update(&mut self, delta_time: f32) {
        self.time += delta_time;
    }

    fn get_camera_position(&self) -> Vec3 {
        let sin_theta = self.camera_theta.sin();
        let cos_theta = self.camera_theta.cos();
        let sin_phi = self.camera_phi.sin();
        let cos_phi = self.camera_phi.cos();

        Vec3::new(
            self.camera_distance * sin_theta * cos_phi,
            self.camera_distance * cos_theta,
            self.camera_distance * sin_theta * sin_phi,
        )
    }

    fn get_view_matrix(&self) -> Mat4 {
        let eye = self.get_camera_position();
        let center = Vec3::ZERO;
        let up = Vec3::Y;
        Mat4::look_at_rh(eye, center, up)
    }

    fn get_projection_matrix(&self, aspect_ratio: f32) -> Mat4 {
        let fov = std::f32::consts::PI / 3.0;
        let near = 0.1;
        let far = 3000.0;
        Mat4::perspective_rh(fov, aspect_ratio, near, far)
    }

    fn draw(&mut self) {
        let (width, height) = match &self.state {
            State::Ready(gfx) => {
                let w = gfx.surface_config().width as f32;
                let h = gfx.surface_config().height as f32;
                (w, h)
            }
            _ => return,
        };

        let aspect_ratio = width / height;
        let view = self.get_view_matrix();
        let proj = self.get_projection_matrix(aspect_ratio);
        let model_view = view; // In this case, model is identity

        // Create uniforms with dynamic values
        let uniforms = PointCloudUniforms {
            model_view,
            projection: proj,
            scan_line_y1: 500.0 + (self.time * 200.0).sin() * 100.0,
            scan_line_y2: 300.0 + (self.time * 180.0 + 2.0).sin() * 100.0,
            scan_line_y3: 100.0 + (self.time * 160.0 + 4.0).sin() * 100.0,
            scan_line_width: 50.0,
            camera_fade_distance: 2000.0,
            camera_fade_start: 100.0,
            feather_width: 0.5,
            core_radius: 0.3,
            inner_glow_strength: 0.8,
            compress_strength: 0.6,
            point_size_scale: 0.1,
            is_active: 1,
            resolution_x: width,
            resolution_y: height,
            glitch_y_range: 10.0,
            glitch_x_offset: 20.0,
            glitch_effects_0: Vec4::ZERO,
            glitch_effects_1: Vec4::ZERO,
            glitch_effects_2: Vec4::ZERO,
            glitch_effects_3: Vec4::ZERO,
        };

        if let State::Ready(gfx) = &mut self.state {
            gfx.update_point_uniforms(&uniforms);
            gfx.draw();
        }
    }

    fn resized(&mut self, size: PhysicalSize<u32>) {
        if let State::Ready(gfx) = &mut self.state {
            gfx.resize(size);
        }
    }

    fn handle_mouse_motion(&mut self, x: f32, y: f32) {
        if self.mouse_pressed {
            let delta_x = x - self.last_mouse_x;
            let delta_y = y - self.last_mouse_y;

            self.camera_phi += delta_x * 0.01;
            self.camera_theta -= delta_y * 0.01;

            // Clamp theta to avoid gimbal lock
            self.camera_theta = self.camera_theta.clamp(0.1, std::f32::consts::PI - 0.1);
        }

        self.last_mouse_x = x;
        self.last_mouse_y = y;
    }
}

impl ApplicationHandler<Graphics> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let State::Init(proxy) = &mut self.state {
            if let Some(proxy) = proxy.take() {
                let mut win_attr = Window::default_attributes();

                #[cfg(not(target_arch = "wasm32"))]
                {
                    win_attr = win_attr.with_title("Tianshi Point Cloud");
                }

                #[cfg(target_arch = "wasm32")]
                {
                    use winit::platform::web::WindowAttributesExtWebSys;
                    win_attr = win_attr.with_append(true);
                }

                let window = Rc::new(
                    event_loop
                        .create_window(win_attr)
                        .expect("create window err."),
                );

                #[cfg(target_arch = "wasm32")]
                wasm_bindgen_futures::spawn_local(create_graphics(window, proxy));

                #[cfg(not(target_arch = "wasm32"))]
                pollster::block_on(create_graphics(window, proxy));
            }
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, mut graphics: Graphics) {
        // Load point cloud data
        if let Ok(cloud) = PointCloud::from_bytes(include_bytes!("../assets/pile.251dc1.bin")) {
            let transformed = cloud.transform_normalized();
            graphics.load_point_cloud(&transformed.points, &transformed.attributes);
        } else {
            eprintln!("Could not load model.");
        }

        graphics.request_redraw();
        self.state = State::Ready(graphics);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => self.resized(size),
            WindowEvent::RedrawRequested => {
                self.update(1.0 / 60.0); // Assume 60 FPS
                self.draw();
                if let State::Ready(gfx) = &self.state {
                    gfx.request_redraw();
                }
            }
            WindowEvent::MouseInput { state, .. } => {
                self.mouse_pressed = matches!(state, winit::event::ElementState::Pressed);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.handle_mouse_motion(position.x as f32, position.y as f32);
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }
}
