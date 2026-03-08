#![no_std]

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};

/// Point cloud rendering uniforms
#[derive(Copy, Clone, Pod, Zeroable)]
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

    /// Activation flag (0/1)
    pub is_active: u32,

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

/// Point vertex data with per-point attributes
#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
pub struct PointVertex {
    pub position: [f32; 3],
    pub active: f32,
    pub size: f32,
    pub layer: f32,
    pub delay: f32,
}

#[inline(always)]
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline(always)]
pub fn mix_f(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

#[inline(always)]
pub fn mix_v3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}
