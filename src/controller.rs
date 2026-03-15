use alloc::vec::Vec;

use glam::Vec4;
use shared::{FadeState, LaserInstance};

use super::glitch::GlitchModule;
use super::laser::LaserModule;
use super::model::Model;
use super::render::RenderLevel;

fn ease_in_out_quad(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScanlineAnim {
    pub start_y: f32,
    pub end_y: f32,
    pub duration_ms: f32,
    pub delay_ms: f32,
    pub current_y: f32,
    pub done: bool,
}

impl ScanlineAnim {
    pub fn new(duration_ms: f32, delay_ms: f32) -> Self {
        Self {
            start_y: -1150.0,
            end_y: 1350.0,
            duration_ms,
            delay_ms,
            current_y: -1150.0,
            done: false,
        }
    }

    pub fn tick(&mut self, elapsed_ms: f32) -> f32 {
        let time_since_start = elapsed_ms - self.delay_ms;
        if time_since_start < 0.0 {
            self.current_y = self.start_y;
            return self.current_y;
        }
        let t = (time_since_start / self.duration_ms).clamp(0.0, 1.0);
        self.current_y = self.start_y + (self.end_y - self.start_y) * ease_in_out_quad(t);
        if time_since_start >= self.duration_ms {
            self.current_y = self.end_y;
            self.done = true;
        }
        self.current_y
    }
}

#[derive(Debug, Clone)]
pub struct FadeInState {
    pub scanlines: [ScanlineAnim; 3],
    pub elapsed_ms: f32,
    pub done: bool,
}

impl FadeInState {
    pub fn new() -> Self {
        Self {
            scanlines: [
                // SL0: duration=3000, delay=0
                ScanlineAnim::new(3000.0, 0.0),
                // SL1: duration=1500, delay=2000*ln(2)=1386
                ScanlineAnim::new(1500.0, 2000.0 * 2.0_f32.ln()),
                // SL2: duration=1000, delay=2000*ln(3)=2197
                ScanlineAnim::new(1000.0, 2000.0 * 3.0_f32.ln()),
            ],
            elapsed_ms: 0.0,
            done: false,
        }
    }

    pub fn tick(&mut self, delta_ms: f32) {
        self.elapsed_ms += delta_ms;
        for sl in &mut self.scanlines {
            sl.tick(self.elapsed_ms);
        }
        self.done = self.scanlines.iter().all(|s| s.done);
    }

    pub fn scanline_uniforms(&self) -> [f32; 3] {
        const DISPLAY_OFFSET: f32 = -200.0;
        [
            self.scanlines[0].current_y + DISPLAY_OFFSET,
            self.scanlines[1].current_y + DISPLAY_OFFSET,
            self.scanlines[2].current_y + DISPLAY_OFFSET,
        ]
    }

    pub fn scanline_raw(&self) -> [f32; 3] {
        [
            self.scanlines[0].current_y,
            self.scanlines[1].current_y,
            self.scanlines[2].current_y,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct FadeOutState {
    pub scanlines: [ScanlineAnim; 3],
    pub elapsed_ms: f32,
    pub done: bool,
}

impl FadeOutState {
    pub fn new() -> Self {
        // All 3 scanlines identical: -1150 -> 1150, 2000ms, no delay, no offset
        Self {
            scanlines: [
                ScanlineAnim {
                    start_y: -1150.0,
                    end_y: 1150.0,
                    duration_ms: 2000.0,
                    delay_ms: 0.0,
                    current_y: -1150.0,
                    done: false,
                },
                ScanlineAnim {
                    start_y: -1150.0,
                    end_y: 1150.0,
                    duration_ms: 2000.0,
                    delay_ms: 0.0,
                    current_y: -1150.0,
                    done: false,
                },
                ScanlineAnim {
                    start_y: -1150.0,
                    end_y: 1150.0,
                    duration_ms: 2000.0,
                    delay_ms: 0.0,
                    current_y: -1150.0,
                    done: false,
                },
            ],
            elapsed_ms: 0.0,
            done: false,
        }
    }

    pub fn tick(&mut self, delta_ms: f32) {
        self.elapsed_ms += delta_ms;
        for sl in &mut self.scanlines {
            sl.tick(self.elapsed_ms);
        }
        self.done = self.scanlines.iter().all(|s| s.done);
    }

    pub fn scanline_uniforms(&self) -> [f32; 3] {
        [
            self.scanlines[0].current_y,
            self.scanlines[1].current_y,
            self.scanlines[2].current_y,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct RotationBurst {
    pub base_speed: f32,
    pub current_speed: f32,
    pub elapsed_ms: f32,
    pub done: bool,
}

impl RotationBurst {
    pub fn new(base_speed: f32) -> Self {
        Self {
            base_speed,
            current_speed: base_speed,
            elapsed_ms: 0.0,
            done: false,
        }
    }

    pub fn tick(&mut self, delta_ms: f32) {
        self.elapsed_ms += delta_ms;
        let peak = self.base_speed * 20.0;

        if self.elapsed_ms <= 400.0 {
            // Phase 1: ramp up to 20x over 400ms
            let t = ease_in_out_quad(self.elapsed_ms / 400.0);
            self.current_speed = self.base_speed + (peak - self.base_speed) * t;
        } else if self.elapsed_ms <= 1200.0 {
            // Phase 2: hold peak during 800ms delay
            self.current_speed = peak;
        } else if self.elapsed_ms <= 2000.0 {
            // Phase 3: ramp down to base over 800ms
            let t = ease_in_out_quad((self.elapsed_ms - 1200.0) / 800.0);
            self.current_speed = peak + (self.base_speed - peak) * t;
        } else {
            self.current_speed = self.base_speed;
            self.done = true;
        }
    }
}

#[derive(Debug, Clone)]
pub struct Transition {
    pub fade_in: FadeInState,
    pub fade_out: FadeOutState,
    pub rotation_burst: RotationBurst,
    pub ray_frame_count: u32,
}

impl Transition {
    pub fn new(auto_rotation_speed: f32) -> Self {
        Self {
            fade_in: FadeInState::new(),
            fade_out: FadeOutState::new(),
            rotation_burst: RotationBurst::new(auto_rotation_speed),
            ray_frame_count: 0,
        }
    }

    pub fn is_done(&self) -> bool {
        self.fade_in.done && self.fade_out.done && self.rotation_burst.done
    }
}

pub struct Controller {
    pub models: Vec<Model>,
    pub current_index: usize,
    pub backup_index: Option<usize>,
    pub transition: Option<Transition>,
    pub glitch_module: GlitchModule,
    pub laser_module: LaserModule,
    pub glitch_effects: [Vec4; 4],
    pub auto_rotation: bool,
    pub auto_rotation_speed: f32,
    pub target_rotation_y: f32,
    pub current_rotation_y: f32,
}

impl Controller {
    pub fn new(models: Vec<Model>, render_level: RenderLevel) -> Self {
        Self {
            models,
            current_index: 0,
            backup_index: None,
            transition: None,
            glitch_module: GlitchModule::new(),
            laser_module: match render_level {
                RenderLevel::Low => LaserModule::new(500, 8),
                RenderLevel::Medium => LaserModule::new(1000, 14),
                RenderLevel::High => LaserModule::new(2000, 20),
            },
            glitch_effects: [Vec4::ZERO; 4],
            auto_rotation: true,
            auto_rotation_speed: 0.0005,
            target_rotation_y: 0.0,
            current_rotation_y: 0.0,
        }
    }

    pub fn tick(&mut self, delta_ms: f32) {
        if self.auto_rotation {
            let speed = if let Some(trans) = &self.transition {
                trans.rotation_burst.current_speed
            } else {
                self.auto_rotation_speed
            };
            self.target_rotation_y += speed;
        }
        self.current_rotation_y += (self.target_rotation_y - self.current_rotation_y) * 0.1;

        let mut transition_just_finished = false;
        if let Some(trans) = &mut self.transition {
            trans.fade_in.tick(delta_ms);
            trans.fade_out.tick(delta_ms);
            trans.rotation_burst.tick(delta_ms);

            trans.ray_frame_count += 1;
            if trans.ray_frame_count >= 2 {
                trans.ray_frame_count = 0;
                let raw = trans.fade_in.scanline_raw();
                let model = &self.models[self.current_index];
                let mut rng = rand::rng();
                self.laser_module.spawn_batch(&mut rng, model, raw[0], 1.0);
                self.laser_module.spawn_batch(&mut rng, model, raw[1], 0.4);
            }

            if trans.is_done() {
                transition_just_finished = true;
            }
        }

        if transition_just_finished {
            self.transition = None;
            self.backup_index = None;
            self.glitch_module = GlitchModule::new();
        }

        self.laser_module.tick(delta_ms);

        if self.transition.is_none() {
            self.glitch_effects = self.glitch_module.tick(delta_ms);
        } else {
            self.glitch_effects = [Vec4::ZERO; 4];
        }
    }

    pub fn switch_to(&mut self, index: usize) {
        if self.transition.is_some() {
            return;
        }

        let old_index = self.current_index;
        self.current_index = index;
        self.backup_index = Some(old_index);

        self.glitch_effects = [Vec4::ZERO; 4];

        self.laser_module.clear();

        // Start transition
        self.transition = Some(Transition::new(self.auto_rotation_speed));
    }

    pub fn is_transitioning(&self) -> bool {
        self.transition.is_some()
    }

    pub fn current_scanline_uniforms(&self) -> [f32; 3] {
        match &self.transition {
            Some(t) => t.fade_in.scanline_uniforms(),
            None => [1150.0; 3],
        }
    }

    pub fn current_fade_state(&self) -> FadeState {
        if self.transition.is_some() {
            FadeState::FadingIn
        } else {
            FadeState::None
        }
    }

    pub fn backup_scanline_uniforms(&self) -> [f32; 3] {
        match &self.transition {
            Some(t) => t.fade_out.scanline_uniforms(),
            None => [-1150.0; 3],
        }
    }

    pub fn backup_fade_state(&self) -> FadeState {
        FadeState::FadingOut
    }

    pub fn glitch_effects(&self) -> [Vec4; 4] {
        self.glitch_effects
    }

    pub fn laser_instances(&self) -> &[LaserInstance] {
        &self.laser_module.instances
    }

    pub fn model_rotation_y(&self) -> f32 {
        self.current_rotation_y
    }
}
