use core::f32::consts::PI;

use alloc::boxed::Box;
use alloc::vec::Vec;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use glam::{Mat4, Vec2, Vec3};
use hashbrown::HashSet;
use shared::{LaserUniforms, PointCloudUniforms};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use super::controller::Controller;
use super::model::{Model, load_models};
use super::render::{Graphics, Rc, RenderLevel, create_graphics};

enum State {
    Ready {
        controller: Box<Controller>,
        graphics: Box<Graphics>,
    },
    Init(Option<EventLoopProxy<Graphics>>),
}

pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,   // Horizontal rotation (radians)
    pub pitch: f32, // Vertical rotation (radians)
    pub speed: f32, // Movement speed
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: Vec3::new(0.0, 300.0, -2000.0),
            yaw: PI,
            pitch: -0.15,
            speed: 800.0,
        }
    }

    pub fn update(&mut self, delta_ms: f32, keys: &HashSet<KeyCode>) {
        let forward = Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos());
        let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());

        for key in keys.iter() {
            match key {
                KeyCode::KeyW => {
                    self.position += forward * self.speed * (delta_ms / 1000.0);
                }
                KeyCode::KeyS => {
                    self.position -= forward * self.speed * (delta_ms / 1000.0);
                }
                KeyCode::KeyA => {
                    self.position -= right * self.speed * (delta_ms / 1000.0);
                }
                KeyCode::KeyD => {
                    self.position += right * self.speed * (delta_ms / 1000.0);
                }
                KeyCode::Space => {
                    self.position.y += self.speed * (delta_ms / 1000.0);
                }
                KeyCode::ControlLeft => {
                    self.position.y -= self.speed * (delta_ms / 1000.0);
                }
                _ => {}
            }
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        let forward = Vec3::new(
            -self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
        .normalize();

        let target = self.position + forward;
        let up = Vec3::Y;

        Mat4::look_at_rh(self.position, target, up)
    }

    pub fn rotate(&mut self, delta_x: f32, delta_y: f32, sensitivity: f32) {
        self.yaw -= delta_x * sensitivity;
        self.pitch = (self.pitch + delta_y * sensitivity).clamp(-1.5, 1.5); // Clamp pitch
    }
}

pub struct App {
    state: State,
    last_time: Instant,
    camera: Camera,
    is_right_dragging: bool, // Right mouse for camera look
    is_left_down: bool,      // Left mouse down for model rotation
    last_mouse: Vec2,
    keys_pressed: HashSet<KeyCode>,
}

impl App {
    pub fn new(event_loop: &EventLoop<Graphics>) -> Self {
        Self {
            state: State::Init(Some(event_loop.create_proxy())),
            last_time: Instant::now(),
            camera: Camera::new(),
            is_right_dragging: false,
            is_left_down: false,
            last_mouse: Vec2::ZERO,
            keys_pressed: HashSet::new(),
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let delta_ms = now.duration_since(self.last_time).as_secs_f32() * 1000.0;
        self.last_time = now;

        self.camera.update(delta_ms, &self.keys_pressed);

        if let State::Ready { controller, .. } = &mut self.state {
            controller.tick(delta_ms);
        }
    }

    fn view_matrix(&self) -> Mat4 {
        self.camera.view_matrix()
    }

    fn projection_matrix(aspect_ratio: f32) -> Mat4 {
        let fov = 75.0_f32.to_radians();
        let near = 0.1;
        let far = 10000.0;
        Mat4::perspective_rh(fov, aspect_ratio, near, far)
    }

    fn model_matrix(model: &Model, rotation_y: f32) -> Mat4 {
        let offset = model.offset;
        let pivot = model.pivot;
        let scale = model.scale;

        let mesh_pos = -pivot;
        let mesh_translate = Mat4::from_translation(mesh_pos);

        let group_scale = Mat4::from_scale(Vec3::splat(scale));

        let group_rotation = Mat4::from_rotation_y(rotation_y);

        let group_pos = offset + pivot;
        let group_translate = Mat4::from_translation(group_pos);

        group_translate * group_rotation * group_scale * mesh_translate
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
        let view_mat = self.view_matrix();
        let projection_mat = Self::projection_matrix(aspect_ratio);

        if let State::Ready {
            graphics,
            controller,
        } = &mut self.state
        {
            let rotation_y = controller.model_rotation_y();
            let glitch = controller.glitch_effects();

            let current_model = &controller.models[controller.current_index];
            let current_model_mat = Self::model_matrix(current_model, rotation_y);
            let current_model_view = view_mat * current_model_mat;
            let [sl1, sl2, sl3] = controller.current_scanline_uniforms();

            graphics.update_current_uniforms(&PointCloudUniforms {
                model_view: current_model_view,
                projection: projection_mat,
                scan_line_y1: sl1,
                scan_line_y2: sl2,
                scan_line_y3: sl3,
                scan_line_width: 50.0,
                camera_fade_distance: current_model.camera_fade_distance as f32,
                camera_fade_start: current_model.camera_fade_start as f32,
                feather_width: 0.5,
                core_radius: 0.3,
                inner_glow_strength: 0.8,
                compress_strength: 0.6,
                point_size_scale: current_model.point_size_scale * 0.1,
                fade_state: controller.current_fade_state().into(),
                resolution_x: width,
                resolution_y: height,
                glitch_y_range: 10.0,
                glitch_x_offset: 20.0,
                glitch_effects_0: glitch[0],
                glitch_effects_1: glitch[1],
                glitch_effects_2: glitch[2],
                glitch_effects_3: glitch[3],
            });

            if let Some(backup_idx) = controller.backup_index {
                let backup_model = &controller.models[backup_idx];
                let backup_model_mat = Self::model_matrix(backup_model, rotation_y);
                let backup_model_view = view_mat * backup_model_mat;
                let [bsl1, bsl2, bsl3] = controller.backup_scanline_uniforms();

                graphics.update_backup_uniforms(&PointCloudUniforms {
                    model_view: backup_model_view,
                    projection: projection_mat,
                    scan_line_y1: bsl1,
                    scan_line_y2: bsl2,
                    scan_line_y3: bsl3,
                    scan_line_width: 20.0,
                    camera_fade_distance: backup_model.camera_fade_distance as f32,
                    camera_fade_start: backup_model.camera_fade_start as f32,
                    feather_width: 0.1,
                    core_radius: 0.1,
                    inner_glow_strength: 0.6,
                    compress_strength: 0.5,
                    point_size_scale: backup_model.point_size_scale,
                    fade_state: controller.backup_fade_state().into(),
                    resolution_x: width,
                    resolution_y: height,
                    glitch_y_range: 10.0,
                    glitch_x_offset: 20.0,
                    glitch_effects_0: Default::default(),
                    glitch_effects_1: Default::default(),
                    glitch_effects_2: Default::default(),
                    glitch_effects_3: Default::default(),
                });
            } else {
                // Clear backup actor when transition is done
                graphics.clear_backup_point_cloud();
            }

            graphics.update_laser_uniforms(&LaserUniforms {
                model_view: view_mat,
                projection: projection_mat,
                camera_pos: self.camera.position,
                camera_fade_distance: 2000.0,
            });

            let instances = controller
                .laser_instances()
                .iter()
                .map(|instance| {
                    let mut instance = *instance;
                    instance.src = current_model_mat
                        .transform_point3(Vec3::from_array(instance.src))
                        .to_array();
                    instance.target = current_model_mat
                        .transform_point3(Vec3::from_array(instance.target))
                        .to_array();
                    instance
                })
                .collect::<Vec<_>>();
            graphics.update_laser_instances(&instances);

            graphics.draw();
        }
    }
}

impl ApplicationHandler<Graphics> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let State::Init(proxy) = &mut self.state
            && let Some(proxy) = proxy.take()
        {
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

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, mut graphics: Graphics) {
        // Load models and create controller
        let models = load_models();
        let controller = Controller::new(models, RenderLevel::High);

        let model = &controller.models[controller.current_index];
        graphics.load_current_point_cloud(&model.data.points, &model.data.attributes);
        graphics.request_redraw();

        self.state = State::Ready {
            graphics: Box::new(graphics),
            controller: Box::new(controller),
        };
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => {
                if let State::Ready { graphics, .. } = &mut self.state {
                    graphics.resize(size);
                }
            }
            WindowEvent::RedrawRequested => {
                self.update();
                self.draw();
                if let State::Ready { graphics, .. } = &self.state {
                    graphics.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            // Q key: switch model
                            if keycode == KeyCode::KeyQ
                                && let State::Ready {
                                    graphics,
                                    controller,
                                } = &mut self.state
                                && !controller.is_transitioning()
                            {
                                let next_index =
                                    (controller.current_index + 1) % controller.models.len();

                                graphics.swap_actors();

                                let new_model = &controller.models[next_index];
                                graphics.load_current_point_cloud(
                                    &new_model.data.points,
                                    &new_model.data.attributes,
                                );

                                controller.switch_to(next_index);
                            }

                            self.keys_pressed.insert(keycode);
                        }
                        ElementState::Released => {
                            self.keys_pressed.remove(&keycode);
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                match button {
                    MouseButton::Right => {
                        // Right mouse: toggle camera look mode
                        match state {
                            ElementState::Pressed => {
                                self.is_right_dragging = true;
                                if let State::Ready { controller, .. } = &mut self.state {
                                    controller.auto_rotation = false;
                                }
                            }
                            ElementState::Released => {
                                self.is_right_dragging = false;
                                if let State::Ready { controller, .. } = &mut self.state {
                                    controller.auto_rotation = true;
                                }
                            }
                        }
                    }
                    MouseButton::Left => {
                        // Left mouse: manual model rotation (only while held)
                        match state {
                            ElementState::Pressed => {
                                self.is_left_down = true;
                                if let State::Ready { controller, .. } = &mut self.state {
                                    controller.auto_rotation = false;
                                }
                            }
                            ElementState::Released => {
                                self.is_left_down = false;
                                if let State::Ready { controller, .. } = &mut self.state {
                                    controller.auto_rotation = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x as f32;
                let y = position.y as f32;

                if self.is_right_dragging {
                    // Right mouse dragging: rotate camera view
                    let delta_x = x - self.last_mouse.x;
                    let delta_y = y - self.last_mouse.y;
                    self.camera.rotate(delta_x, delta_y, 0.003);
                }

                if self.is_left_down {
                    // Left mouse held: rotate model
                    let delta_x = x - self.last_mouse.x;
                    if let State::Ready { controller, .. } = &mut self.state {
                        controller.target_rotation_y += delta_x * 0.01;
                    }
                }

                self.last_mouse = Vec2::new(x, y);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // Mouse wheel: switch model
                let y_delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };

                if y_delta != 0.0
                    && let State::Ready {
                        graphics,
                        controller,
                    } = &mut self.state
                    && !controller.is_transitioning()
                {
                    let next_index = if y_delta > 0.0 {
                        // Scroll up: next model
                        (controller.current_index + 1) % controller.models.len()
                    } else {
                        // Scroll down: previous model
                        (controller.current_index + controller.models.len() - 1)
                            % controller.models.len()
                    };

                    graphics.swap_actors();

                    let new_model = &controller.models[next_index];
                    graphics.load_current_point_cloud(
                        &new_model.data.points,
                        &new_model.data.attributes,
                    );

                    controller.switch_to(next_index);
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }
}
