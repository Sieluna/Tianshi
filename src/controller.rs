use alloc::vec::Vec;

use glam::Vec4;
use rand::Rng;

use super::actor::PointCloudActor;
use super::glitch::GlitchEffect;
use super::model::Model;

/// Glitch controller for managing glitch effects
pub struct Controller {
    pub models: Vec<Model>,
    pub actor: PointCloudActor,
    pub glitch: GlitchEffect,
    pub glitch_effects: [Vec4; 4],
    pub glitch_loop_timer: f32,   // Timer for glitch effect intervals
    pub glitch_loop_active: bool, // Whether continuous glitch loop is running
}

impl Controller {
    pub fn new(models: Vec<Model>) -> Self {
        Self {
            models,
            actor: PointCloudActor::new(1.0),
            glitch: GlitchEffect::new(),
            glitch_effects: [Vec4::ZERO; 4],
            glitch_loop_timer: 0.0,
            glitch_loop_active: false,
        }
    }

    /// Update controller state and effects
    pub fn update(&mut self, delta_ms: f32) {
        // Update actor
        self.actor.update(delta_ms);

        // Update glitch effect
        self.glitch_effects = self.glitch.update(delta_ms);

        // Update glitch loop
        if self.glitch_loop_active {
            self.glitch_loop_timer += delta_ms;

            // Trigger glitch every 4-6 seconds
            let mut rng = rand::rng();
            let glitch_interval = 4000.0 + rng.random_range(0.0..2000.0);
            if self.glitch_loop_timer >= glitch_interval {
                self.glitch_loop_timer = 0.0;
                self.glitch.activate(&mut rng);
            }
        }
    }
}
