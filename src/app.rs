use core::f32::consts::PI;

use glam::{Mat4, Vec3};
use shared::{LaserUniforms, PointCloudUniforms};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

use super::controller::Controller;
use super::model::load_models;
use super::render::{Graphics, Rc, create_graphics};

enum State {
    Ready {
        controller: Controller,
        graphics: Graphics,
    },
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
            camera_theta: PI / 3.0,
            camera_distance: 2400.0,
            last_mouse_x: 0.0,
            last_mouse_y: 0.0,
            mouse_pressed: false,
        }
    }

    fn update(&mut self, delta_time: f32) {
        self.time += delta_time;

        // Update controller logic
        if let State::Ready { controller, .. } = &mut self.state {
            let delta_ms = delta_time * 1000.0;
            controller.update(delta_ms);
        }
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
        let fov = PI / 3.0;
        let near = 0.1;
        let far = 3000.0;
        Mat4::perspective_rh(fov, aspect_ratio, near, far)
    }

    fn draw(&mut self) {
        let (width, height) = match &self.state {
            State::Ready { graphics, .. } => {
                let w = graphics.surface_config().width as f32;
                let h = graphics.surface_config().height as f32;
                (w, h)
            }
            _ => return,
        };

        let aspect_ratio = width / height;
        let camera_pos = self.get_camera_position();
        let model_view = self.get_view_matrix();
        let projection = self.get_projection_matrix(aspect_ratio);

        if let State::Ready {
            graphics,
            controller,
        } = &mut self.state
        {
            let current = &controller.actor;
            let [scan_line_y1, scan_line_y2, scan_line_y3] = current.get_scanline_ys();

            let glitch_effects = controller.glitch_effects;

            // Create uniforms with dynamic values
            let uniforms = PointCloudUniforms {
                model_view,
                projection,
                scan_line_y1,
                scan_line_y2,
                scan_line_y3,
                scan_line_width: 50.0,
                camera_fade_distance: 2000.0,
                camera_fade_start: 100.0,
                feather_width: 0.5,
                core_radius: 0.3,
                inner_glow_strength: 0.8,
                compress_strength: 0.6,
                point_size_scale: 0.1,
                is_active: current.is_active_uniform,
                resolution_x: width,
                resolution_y: height,
                glitch_y_range: 10.0,
                glitch_x_offset: 20.0,
                glitch_effects_0: glitch_effects[0],
                glitch_effects_1: glitch_effects[1],
                glitch_effects_2: glitch_effects[2],
                glitch_effects_3: glitch_effects[3],
            };
            graphics.update_point_uniforms(&uniforms);

            let laser_uniforms = LaserUniforms {
                model_view,
                projection,
                camera_pos,
                camera_fade_distance: 2000.0,
            };
            graphics.update_laser_uniforms(&laser_uniforms);

            graphics.update_laser_instances(&current.laser_pool.instances);

            graphics.draw();
        }
    }

    fn resized(&mut self, size: PhysicalSize<u32>) {
        if let State::Ready { graphics, .. } = &mut self.state {
            graphics.resize(size);
        }
    }

    fn handle_mouse_motion(&mut self, x: f32, y: f32) {
        if self.mouse_pressed {
            let delta_x = x - self.last_mouse_x;
            let delta_y = y - self.last_mouse_y;

            self.camera_phi += delta_x * 0.01;
            self.camera_theta -= delta_y * 0.01;

            // Clamp theta to avoid gimbal lock
            self.camera_theta = self.camera_theta.clamp(0.1, PI - 0.1);
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
        // Load models and create controller
        let models = load_models();
        let mut controller = Controller::new(models);

        let model = &controller.models[2];
        graphics.load_point_cloud(&model.data.points, &model.data.attributes);
        controller.glitch_loop_active = true;

        graphics.request_redraw();
        self.state = State::Ready {
            graphics,
            controller,
        };
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
                let delta_time = 1.0 / 60.0; // Assume 60 FPS
                self.update(delta_time);
                self.draw();
                if let State::Ready { graphics, .. } = &self.state {
                    graphics.request_redraw();
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
