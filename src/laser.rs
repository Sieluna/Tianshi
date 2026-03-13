use alloc::vec::Vec;

use glam::Vec3;
use rand::Rng;
use rand::prelude::IndexedRandom;
use shared::LaserInstance;

use super::model::{LaserMode, Model};

#[derive(Debug, Clone, Copy)]
pub struct LaserRay {
    pub src_pos: Vec3,
    pub target_pos: Vec3,
    pub progress: f32,     // [0, 1]
    pub base_opacity: f32, // [0, 1]
    pub age_ms: f32,
    pub lifetime_ms: f32,
    pub delay_ms: f32, // [0, 200)
    pub active: bool,
    pub random_offset: f32, // [0, 1]
}

impl LaserRay {
    pub fn new(
        src_pos: Vec3,
        target_pos: Vec3,
        base_opacity: f32,
        delay_ms: f32,
        random_offset: f32,
        lifetime_ms: f32,
    ) -> Self {
        Self {
            src_pos,
            target_pos,
            progress: 0.0,
            base_opacity,
            age_ms: 0.0,
            lifetime_ms,
            delay_ms,
            active: false, // Will become active after delay
            random_offset,
        }
    }

    /// Update ray state, returns true if still active.
    pub fn update(&mut self, delta_ms: f32) -> bool {
        self.age_ms += delta_ms;

        // Check if delay has passed
        if self.age_ms >= self.delay_ms {
            self.active = true;
            let age_since_active = self.age_ms - self.delay_ms;
            self.progress = (age_since_active / self.lifetime_ms).min(1.0);

            // Deactivate when lifetime expires
            if age_since_active >= self.lifetime_ms {
                self.active = false;
            }
        }

        self.active
    }
}

#[derive(Debug, Clone)]
pub struct LaserRayPool {
    pub rays: Vec<LaserRay>,
    pub instances: Vec<LaserInstance>,
    pub max_rays: usize,
    pub ray_per_batch: usize,
}

impl LaserRayPool {
    pub fn new(max_rays: usize, ray_per_batch: usize) -> Self {
        Self {
            rays: Vec::with_capacity(max_rays),
            instances: Vec::with_capacity(max_rays),
            max_rays,
            ray_per_batch,
        }
    }

    /// Spawn a batch of rays targeting point cloud.
    pub fn spawn_batch<R: Rng>(&mut self, rng: &mut R, model: &Model, scan_line_y: f32, n: f32) {
        let count_to_spawn = self.ray_per_batch.min(self.max_rays - self.rays.len());
        let mut spawned = 0;

        for _ in 0..500 {
            if spawned >= count_to_spawn {
                break;
            }

            // Get a random point from the point cloud as target
            let point_count = model.data.point_count();
            if point_count == 0 {
                continue;
            }

            let random_point_idx = rng.random_range(0..point_count);
            let target_pos = model.data.point(random_point_idx).unwrap_or(Vec3::ZERO);

            match model.laser_mode {
                LaserMode::Ceiling => {
                    // For ceiling mode (during scanline animation), only spawn near the scanline
                    if (target_pos.y - scan_line_y).abs() > 40.0 {
                        continue;
                    }
                }
                LaserMode::Random => {
                    // For random mode, skip points above the scanline
                    if target_pos.y > scan_line_y {
                        continue;
                    }
                }
            }

            // Source position depends on laser mode
            let src_pos = match model.laser_mode {
                LaserMode::Ceiling => {
                    // Source from ceiling at fixed Y=2000, with X,Z matching target
                    Vec3::new(target_pos.x, 2000.0, target_pos.z)
                }
                LaserMode::Random => {
                    // Source from random discrete X position, fixed Y=2000, Z=0
                    let x_options = [-3000.0_f32, 400.0_f32, 3000.0_f32];
                    let x = *x_options.choose(rng).unwrap();
                    Vec3::new(x, 2000.0, 0.0)
                }
            };

            // Base opacity: 0.8 * random * n
            let base_opacity = 0.8 * rng.random::<f32>() * n;

            // Delay: (0, 200)ms
            let delay_ms = rng.random_range(0.0..200.0);

            // Random offset for noise: [0, 1)
            let random_offset = rng.random::<f32>();

            // Lifetime: (0.5 + random) * 400 * n
            let lifetime_ms = (0.5 + rng.random::<f32>()) * 400.0 * n;

            self.rays.push(LaserRay::new(
                src_pos,
                target_pos,
                base_opacity,
                delay_ms,
                random_offset,
                lifetime_ms,
            ));
            spawned += 1;
        }
    }

    /// Update all rays and refresh cached instances.
    pub fn update(&mut self, delta_ms: f32) {
        self.rays.retain_mut(|ray| ray.update(delta_ms));

        self.instances.clear();
        self.instances.reserve(self.rays.len());
        for ray in &self.rays {
            self.instances.push(LaserInstance {
                src: ray.src_pos.to_array(),
                progress: ray.progress,
                target: ray.target_pos.to_array(),
                base_opacity: ray.base_opacity,
                random_offset: ray.random_offset,
                _padding0: 0.0,
                _padding1: 0.0,
                _padding2: 0.0,
            });
        }
    }

    pub fn clear(&mut self) {
        self.rays.clear();
        self.instances.clear();
    }

    pub fn is_full(&self) -> bool {
        self.rays.len() >= self.max_rays
    }
}
