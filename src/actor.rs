use shared::fade;

use super::animation::{FadeAnimation, FadeDirection};
use super::laser::LaserRayPool;
use super::model::Model;

/// Scanline durations for 3 cascading lines
const SCANLINE_DURATIONS: [f32; 3] = [3000.0, 1500.0, 1000.0];

/// Scanline delays (cascade timing)
/// Calculated as: ln(i+1) * 2000
const SCANLINE_DELAYS: [f32; 3] = [0.0, 1386.0, 2197.0];

/// Y coordinate range for scanline animation
const SCANLINE_START_Y: f32 = -1150.0;
const SCANLINE_END_Y: f32 = 1350.0;
const SCANLINE_DISPLAY_OFFSET: f32 = -200.0;

/// Compute scanline Y position for a given line index and elapsed time.
pub fn compute_scanline_y(line_index: usize, elapsed_ms: f32) -> f32 {
    if line_index >= 3 {
        return SCANLINE_START_Y + SCANLINE_DISPLAY_OFFSET;
    }

    let delay = SCANLINE_DELAYS[line_index];
    let time_since_start = elapsed_ms - delay;

    // Not yet started
    if time_since_start < 0.0 {
        return SCANLINE_START_Y + SCANLINE_DISPLAY_OFFSET;
    }

    let duration = SCANLINE_DURATIONS[line_index];
    let t = (time_since_start / duration).min(1.0);
    let eased_t = fade(t);

    let y = SCANLINE_START_Y + (SCANLINE_END_Y - SCANLINE_START_Y) * eased_t;
    y + SCANLINE_DISPLAY_OFFSET
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActorState {
    Inactive,
    Transition(FadeAnimation),
    Active,
}

#[derive(Debug, Clone)]
pub struct PointCloudActor {
    pub scan_elapsed_ms: f32, // Elapsed time for scanline animation
    pub fade_alpha: f32,      // Current alpha value
    pub state: ActorState,
    pub is_active_uniform: u32,   // 1=normal, 0=hide above scanline
    pub laser_pool: LaserRayPool, // Laser rays are managed by the actor's state timeline
}

impl PointCloudActor {
    pub fn new() -> Self {
        Self {
            scan_elapsed_ms: 0.0,
            fade_alpha: 0.0,
            state: ActorState::Inactive,
            is_active_uniform: 0,
            laser_pool: LaserRayPool::new(2000, 20),
        }
    }

    /// Get all three scanline Y positions
    pub fn get_scanline_ys(&self) -> [f32; 3] {
        [
            compute_scanline_y(0, self.scan_elapsed_ms),
            compute_scanline_y(1, self.scan_elapsed_ms),
            compute_scanline_y(2, self.scan_elapsed_ms),
        ]
    }

    /// Start fade-in animation.
    pub fn fade_in(&mut self, duration_ms: f32) {
        self.state = ActorState::Transition(FadeAnimation::new(FadeDirection::In, duration_ms));
        self.scan_elapsed_ms = 0.0;
        self.is_active_uniform = 1;
    }

    /// Start fade-out animation.
    pub fn fade_out(&mut self, duration_ms: f32) {
        self.state = ActorState::Transition(FadeAnimation::new(FadeDirection::Out, duration_ms));
        self.is_active_uniform = 0;
    }

    /// Update actor state and return animation result.
    pub fn update(&mut self, delta_ms: f32, model: &Model) {
        // Update state machine
        match &mut self.state {
            ActorState::Inactive => {
                // Do nothing
            }
            ActorState::Transition(fade_anim) => {
                self.fade_alpha = fade_anim.update(delta_ms);

                if fade_anim.is_finished() {
                    self.state = match fade_anim.direction {
                        FadeDirection::In => ActorState::Active,
                        FadeDirection::Out => ActorState::Inactive,
                    };
                }

                // Continue scanline animation during fade
                self.scan_elapsed_ms += delta_ms;

                // Spawn laser rays during transitions for scanline 1 and 2
                let mut rng = rand::rng();
                let scan_y1 = compute_scanline_y(0, self.scan_elapsed_ms);
                let scan_y2 = compute_scanline_y(1, self.scan_elapsed_ms);
                self.laser_pool.spawn_batch(&mut rng, model, scan_y1, 1.0);
                self.laser_pool.spawn_batch(&mut rng, model, scan_y2, 0.4);
            }
            ActorState::Active => {
                // Continue scanline animation
                self.scan_elapsed_ms += delta_ms;
            }
        }

        // Update laser pool
        self.laser_pool.update(delta_ms);
    }
}
