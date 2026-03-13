use alloc::vec::Vec;

use glam::Vec4;
use rand::Rng;

use super::actor::{ActorState, PointCloudActor};
use super::glitch::GlitchEffect;
use super::model::Model;

/// Cycle timing constants (in milliseconds)
const CYCLE_DURATION: f32 = 20000.0;
const FADE_IN_END: f32 = 2000.0;
const ACTIVE_END: f32 = 12000.0;
const FADE_OUT_START: f32 = 12000.0;
const FADE_OUT_END: f32 = 14000.0;

pub struct Controller {
    pub models: Vec<Model>,
    pub actor: PointCloudActor,
    pub glitch: GlitchEffect,
    pub glitch_effects: [Vec4; 4],
    pub glitch_loop_timer: f32,
    pub glitch_loop_active: bool,
    pub cycle_timer: f32,
}

impl Controller {
    pub fn new(models: Vec<Model>) -> Self {
        Self {
            models,
            actor: PointCloudActor::new(),
            glitch: GlitchEffect::new(),
            glitch_effects: [Vec4::ZERO; 4],
            glitch_loop_timer: 0.0,
            glitch_loop_active: false,
            cycle_timer: 0.0,
        }
    }

    pub fn update(&mut self, delta_ms: f32) {
        // Advance cycle timer
        self.cycle_timer = (self.cycle_timer + delta_ms) % CYCLE_DURATION;

        // Drive actor state transitions based on cycle phase
        self.drive_actor_state();

        // Update actor (handles fade animation and laser generation)
        let model = &self.models[2];
        self.actor.update(delta_ms, model);

        // Update glitch effects
        self.update_glitch(delta_ms);
    }

    fn drive_actor_state(&mut self) {
        match self.cycle_timer {
            t if t < FADE_IN_END => {
                if !matches!(self.actor.state, ActorState::Transition(_)) {
                    self.actor.fade_in(FADE_IN_END);
                }
            }
            t if t < ACTIVE_END => {
                if !matches!(
                    self.actor.state,
                    ActorState::Active | ActorState::Transition(_)
                ) {
                    self.actor.state = ActorState::Active;
                    self.actor.fade_alpha = 1.0;
                }
            }
            t if t < FADE_OUT_END => {
                if !matches!(self.actor.state, ActorState::Transition(_)) {
                    self.actor.fade_out(FADE_OUT_END - FADE_OUT_START);
                }
            }
            _ => {
                if !matches!(self.actor.state, ActorState::Inactive) {
                    self.actor.state = ActorState::Inactive;
                    self.actor.fade_alpha = 0.0;
                }
            }
        }
    }

    fn update_glitch(&mut self, delta_ms: f32) {
        self.glitch_effects = self.glitch.update(delta_ms);

        if !self.glitch_loop_active {
            return;
        }

        self.glitch_loop_timer += delta_ms;
        let mut rng = rand::rng();
        let interval = 4000.0 + rng.random_range(0.0..2000.0);

        if self.glitch_loop_timer >= interval {
            self.glitch_loop_timer = 0.0;
            self.glitch.activate(&mut rng);
        }
    }
}
