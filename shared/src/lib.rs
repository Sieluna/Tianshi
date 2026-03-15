#![no_std]

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
#[allow(unused_imports)]
use spirv_std::num_traits::Float;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum FadeState {
    None = 0,
    FadingIn = 1,
    FadingOut = 2,
}

impl From<FadeState> for u32 {
    #[inline]
    fn from(state: FadeState) -> u32 {
        state as u32
    }
}

impl From<u32> for FadeState {
    #[inline]
    fn from(value: u32) -> Self {
        match value {
            1 => Self::FadingIn,
            2 => Self::FadingOut,
            _ => Self::None,
        }
    }
}

/// Point cloud rendering uniforms
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
pub struct PointCloudUniforms {
    /// Model-view matrix (4x4)
    pub model_view: Mat4,

    /// Projection matrix (4x4)
    pub projection: Mat4,

    /// Scan line Y coordinates (3 lines)
    pub scan_line_y1: f32,
    pub scan_line_y2: f32,
    pub scan_line_y3: f32,
    pub scan_line_width: f32,

    /// Camera fade parameters
    pub camera_fade_distance: f32,
    pub camera_fade_start: f32,

    /// Visual effect parameters
    pub feather_width: f32,
    pub core_radius: f32,
    pub inner_glow_strength: f32,
    pub compress_strength: f32,
    pub point_size_scale: f32,

    /// Fade parameters
    pub fade_state: u32,

    /// Screen resolution
    pub resolution_x: f32,
    pub resolution_y: f32,

    /// Glitch parameters
    pub glitch_y_range: f32,
    pub glitch_x_offset: f32,

    /// Glitch effects (4 bands, each vec4)
    pub glitch_effects_0: Vec4,
    pub glitch_effects_1: Vec4,
    pub glitch_effects_2: Vec4,
    pub glitch_effects_3: Vec4,
}

/// Laser rendering uniforms
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
pub struct LaserUniforms {
    /// Model-view matrix (4x4)
    pub model_view: Mat4,

    /// Projection matrix (4x4)
    pub projection: Mat4,

    /// Camera position (world space)
    pub camera_pos: Vec3,

    /// Camera fade distance
    pub camera_fade_distance: f32,
}

/// Point vertex data with per-point attributes
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
pub struct PointVertex {
    pub position: [f32; 3],
    pub active: f32,
    pub size: f32,
    pub layer: f32,
    pub delay: f32,
}

/// Per-laser instance data
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
pub struct LaserInstance {
    /// Start point (world space)
    pub src: [f32; 3],
    /// Animation progress [0, 1]
    pub progress: f32,

    /// End point (world space)
    pub target: [f32; 3],
    /// Base opacity [0, 1]
    pub base_opacity: f32,

    /// Random offset for noise [0, 1]
    pub random_offset: f32,

    /// Padding
    pub _padding0: f32,
    pub _padding1: f32,
    pub _padding2: f32,
}

#[inline(always)]
pub fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline(always)]
pub fn lerp(x: f32, y: f32, s: f32) -> f32 {
    x + s * (y - x)
}

#[inline(always)]
pub fn lerp_3d(a: Vec3, b: Vec3, s: f32) -> Vec3 {
    a + (b - a) * s
}

#[inline(always)]
fn perm(index: i32, seed: i32) -> i32 {
    let mut hash = index.wrapping_mul(seed);
    hash = hash ^ (hash >> 13);
    hash = hash.wrapping_mul(hash.wrapping_mul(15731).wrapping_add(74323));
    hash = hash ^ (hash >> 16);
    hash.abs() % 256
}

#[inline(always)]
pub fn fade(t: f32) -> f32 {
    t * t * t * (t * (6.0 * t - 15.0) + 10.0)
}

#[inline(always)]
fn grad(hash: i32, x: f32) -> f32 {
    let grad_idx = hash & 15;
    let gradient = if (grad_idx & 1) == 0 { 1.0 } else { -1.0 };
    gradient * x
}

#[inline(always)]
pub fn noise(x: f32, seed: i32) -> f32 {
    let xi = x.floor() as i32;
    let xf = x - x.floor();

    let u = fade(xf);

    let p0 = perm(xi, seed);
    let p1 = perm(xi + 1, seed);

    let g0 = grad(p0, xf);
    let g1 = grad(p1, xf - 1.0);

    (lerp(g0, g1, u) + 1.0) / 2.0
}
