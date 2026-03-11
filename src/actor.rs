use shared::fade;

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
    FadingIn { elapsed_ms: f32, duration_ms: f32 },
    Active,
    FadingOut { elapsed_ms: f32, duration_ms: f32 },
}

/// Result of updating an actor
pub struct ActorUpdateResult {
    pub scan_line_ys: [f32; 3],
    pub alpha: f32,
    pub is_done_fading_out: bool,
}

/// Point cloud actor for rendering a single model
#[derive(Debug, Clone, Copy)]
pub struct PointCloudActor {
    pub scan_elapsed_ms: f32, // Elapsed time for scanline animation
    pub fade_alpha: f32,      // Current alpha value
    pub state: ActorState,
    pub point_size_scale: f32,
    pub is_active_uniform: u32, // 1=normal, 0=hide above scanline
}

impl PointCloudActor {
    pub fn new(point_size_scale: f32) -> Self {
        Self {
            scan_elapsed_ms: 0.0,
            fade_alpha: 0.0,
            state: ActorState::Inactive,
            point_size_scale,
            is_active_uniform: 0,
        }
    }

    /// Start fade-in animation.
    pub fn fade_in(&mut self, duration_ms: f32) {
        self.state = ActorState::FadingIn {
            elapsed_ms: 0.0,
            duration_ms,
        };
        self.scan_elapsed_ms = 0.0;
        self.is_active_uniform = 1;
    }

    /// Start fade-out animation.
    pub fn fade_out(&mut self, duration_ms: f32) {
        self.state = ActorState::FadingOut {
            elapsed_ms: 0.0,
            duration_ms,
        };
        self.is_active_uniform = 0;
    }

    /// Update actor state and return animation result.
    pub fn update(&mut self, delta_ms: f32) -> ActorUpdateResult {
        // Update state machine
        match self.state {
            ActorState::Inactive => {
                // Do nothing
            }
            ActorState::FadingIn {
                elapsed_ms,
                duration_ms,
            } => {
                let new_elapsed = elapsed_ms + delta_ms;
                let t = (new_elapsed / duration_ms).min(1.0);
                self.fade_alpha = t;

                if new_elapsed >= duration_ms {
                    self.state = ActorState::Active;
                    self.fade_alpha = 1.0;
                } else {
                    self.state = ActorState::FadingIn {
                        elapsed_ms: new_elapsed,
                        duration_ms,
                    };
                }
            }
            ActorState::Active => {
                // Continue scanline animation
            }
            ActorState::FadingOut {
                elapsed_ms,
                duration_ms,
            } => {
                let new_elapsed = elapsed_ms + delta_ms;
                let t = (new_elapsed / duration_ms).min(1.0);
                self.fade_alpha = 1.0 - t;

                if new_elapsed >= duration_ms {
                    self.state = ActorState::Inactive;
                    self.fade_alpha = 0.0;
                } else {
                    self.state = ActorState::FadingOut {
                        elapsed_ms: new_elapsed,
                        duration_ms,
                    };
                }
            }
        }

        // Update scanline animation for Active and FadingIn states
        let is_animating = matches!(self.state, ActorState::FadingIn { .. } | ActorState::Active);
        if is_animating {
            self.scan_elapsed_ms += delta_ms;
        }

        // Compute scanline positions
        let scan_line_ys = [
            compute_scanline_y(0, self.scan_elapsed_ms),
            compute_scanline_y(1, self.scan_elapsed_ms),
            compute_scanline_y(2, self.scan_elapsed_ms),
        ];

        ActorUpdateResult {
            scan_line_ys,
            alpha: self.fade_alpha,
            is_done_fading_out: matches!(self.state, ActorState::Inactive),
        }
    }
}
