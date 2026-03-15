use alloc::vec::Vec;

use glam::Vec4;
use rand::Rng;

#[derive(Debug, Clone, Copy)]
pub struct GlitchLine {
    pub y: f32,        // [-2000, 2000]
    pub x_offset: f32, // [-5, 5]
}

#[derive(Debug, Default, Clone)]
pub struct GlitchEffect {
    pub lines: Vec<GlitchLine>, // 6-8 lines
    pub pulse_count: usize,     // 3-6 pulses
    pub pulse_index: usize,
    pub pulse_elapsed: f32, // time since last pulse (ms)
    pub is_active: bool,
}

impl GlitchEffect {
    pub fn new() -> Self {
        Self::default()
    }

    /// Activate glitch effect with random parameters.
    pub fn activate<R: Rng>(&mut self, rng: &mut R) {
        self.lines.clear();

        // Generate 6-8 random glitch lines
        let line_count = rng.random_range(6..=8);
        for _ in 0..line_count {
            let y = rng.random_range(-2000.0..2000.0);
            let x_offset = rng.random_range(-5.0..5.0);
            self.lines.push(GlitchLine { y, x_offset });
        }

        // Set random pulse count (3-6)
        self.pulse_count = rng.random_range(3..=6);
        self.pulse_index = 0;
        self.pulse_elapsed = 0.0;
        self.is_active = true;
    }

    /// Update glitch animation.
    pub fn tick(&mut self, delta_ms: f32) -> [Vec4; 4] {
        const PULSE_INTERVAL_MS: f32 = 80.0;

        let mut effects = [Vec4::ZERO; 4];

        if !self.is_active {
            return effects;
        }

        self.pulse_elapsed += delta_ms;

        // Check if we need to pulse
        if self.pulse_elapsed >= PULSE_INTERVAL_MS && self.pulse_index < self.pulse_count {
            self.pulse_elapsed = 0.0;
            self.pulse_index += 1;
        }

        // Deactivate after all pulses complete
        if self.pulse_index >= self.pulse_count {
            self.is_active = false;
            return effects;
        }

        // Encode glitch lines into Vec4 uniforms
        // Each Vec4 holds 2 glitch lines: (y0, x_offset0, y1, x_offset1)
        for i in 0..self.lines.len().min(8) {
            let vec_idx = i / 2;
            let line = self.lines[i];

            if i % 2 == 0 {
                effects[vec_idx].x = line.y;
                effects[vec_idx].y = line.x_offset;
            } else {
                effects[vec_idx].z = line.y;
                effects[vec_idx].w = line.x_offset;
            }
        }

        effects
    }
}

pub struct GlitchModule {
    pub effect: GlitchEffect,
    pub timer: f32,
    pub interval: f32,
}

impl GlitchModule {
    pub fn new() -> Self {
        let mut module = Self {
            effect: GlitchEffect::default(),
            timer: 0.0,
            interval: 5000.0,
        };

        let mut rng = rand::rng();
        module.effect.activate(&mut rng);
        module.interval = 4000.0 + rng.random_range(0.0..2000.0);

        module
    }

    pub fn tick(&mut self, delta_ms: f32) -> [Vec4; 4] {
        let effects = self.effect.tick(delta_ms);

        self.timer += delta_ms;
        if self.timer >= self.interval {
            self.timer = 0.0;
            let mut rng = rand::rng();
            self.effect.activate(&mut rng);
            self.interval = 4000.0 + rng.random_range(0.0..2000.0);
        }

        effects
    }
}
