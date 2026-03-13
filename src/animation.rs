use shared::fade;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeDirection {
    In,
    Out,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FadeAnimation {
    pub duration_ms: f32,
    pub direction: FadeDirection,
    pub elapsed_ms: f32,
}

impl FadeAnimation {
    pub fn new(direction: FadeDirection, duration_ms: f32) -> Self {
        Self {
            duration_ms,
            direction,
            elapsed_ms: 0.0,
        }
    }

    /// Update animation state and return current alpha.
    pub fn update(&mut self, delta_ms: f32) -> f32 {
        self.elapsed_ms += delta_ms;
        let t = (self.elapsed_ms / self.duration_ms).min(1.0);
        let progress = fade(t);

        let alpha = match self.direction {
            FadeDirection::In => progress,
            FadeDirection::Out => 1.0 - progress,
        };

        alpha
    }

    pub fn is_finished(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }

    pub fn reset(&mut self) {
        self.elapsed_ms = 0.0;
    }
}
