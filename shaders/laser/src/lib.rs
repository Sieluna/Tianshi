#![no_std]

use shared::{LaserInstance, LaserUniforms, lerp_3d, noise};
use spirv_std::glam::{Vec3, Vec4, Vec4Swizzles};
#[allow(unused_imports)]
use spirv_std::num_traits::Float;
use spirv_std::spirv;

#[spirv(vertex)]
pub fn laser_vs(
    #[spirv(vertex_index)] vertex_index: u32,
    #[spirv(instance_index)] instance_index: u32,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] uniforms: &LaserUniforms,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] laser_data: &[LaserInstance],
    #[spirv(position)] out_pos: &mut Vec4,
    #[spirv(location = 0)] out_world_pos: &mut Vec3,
    #[spirv(location = 1)] out_t: &mut f32,
    #[spirv(location = 2)] out_base_opacity: &mut f32,
    #[spirv(location = 3)] out_random_offset: &mut f32,
) {
    let laser = laser_data[instance_index as usize];

    let progress = laser.progress.clamp(0.0, 1.0);
    let segment_param = ((vertex_index) & 1u32) as f32;
    let t = segment_param * progress;

    let src = Vec3::new(laser.src[0], laser.src[1], laser.src[2]);
    let target = Vec3::new(laser.target[0], laser.target[1], laser.target[2]);
    let pos = lerp_3d(src, target, t);

    *out_t = t;
    *out_base_opacity = laser.base_opacity;
    *out_random_offset = laser.random_offset;

    // Transform to clip space
    let world_pos = Vec4::new(pos.x, pos.y, pos.z, 1.0);
    let view_pos = uniforms.model_view * world_pos;
    let clip_pos = uniforms.projection * view_pos;

    *out_pos = clip_pos;
    *out_world_pos = view_pos.xyz();
}

#[spirv(fragment)]
pub fn laser_fs(
    #[spirv(uniform, descriptor_set = 0, binding = 0)] uniforms: &LaserUniforms,
    #[spirv(location = 0)] world_pos: Vec3,
    #[spirv(location = 1)] t: f32,
    #[spirv(location = 2)] base_opacity: f32,
    #[spirv(location = 3)] random_offset: f32,
    output: &mut Vec4,
) {
    // Camera distance fade
    let dist = world_pos.distance(uniforms.camera_pos);
    let distance_alpha = 1.0 - (dist / uniforms.camera_fade_distance).clamp(0.0, 1.0);

    // Multi-layer opacity decay
    let n = noise(t + random_offset, 3621235);
    let alpha = base_opacity * distance_alpha * n * t;

    if alpha < 0.01 {
        spirv_std::arch::kill();
    }

    *output = Vec4::new(1.0, 1.0, 1.0, alpha);
}
