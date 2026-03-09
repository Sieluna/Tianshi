#![no_std]

use shared::{PointCloudUniforms, mix_f, mix_v3, smoothstep};
use spirv_std::glam::{Vec2, Vec3, Vec4, vec2, vec3, vec4};
use spirv_std::num_traits::Float;
use spirv_std::spirv;

#[spirv(vertex)]
pub fn point_cloud_vs(
    #[spirv(vertex_index)] vertex_index: u32,
    #[spirv(location = 0)] in_position: Vec3,
    #[spirv(location = 1)] in_point_data: Vec4,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] uniforms: &PointCloudUniforms,
    #[spirv(position)] out_pos: &mut Vec4,
    #[spirv(location = 0)] out_coord: &mut Vec2,
    #[spirv(location = 1)] out_alpha: &mut f32,
    #[spirv(location = 2)] out_color: &mut Vec3,
    #[spirv(location = 3)] out_dist_alpha: &mut f32,
    #[spirv(location = 4)] out_screen_uv: &mut Vec2,
) {
    let vert_in_quad = vertex_index % 6;
    let point_active = in_point_data.x;
    let size = in_point_data.y;
    let layer = in_point_data.z;
    let delay = in_point_data.w;

    // Scan line selection
    let mut scan_line_y = uniforms.scan_line_y1;
    if (layer - 2.0).abs() < 0.1 {
        scan_line_y = uniforms.scan_line_y2;
    } else if (layer - 3.0).abs() < 0.1 {
        scan_line_y = uniforms.scan_line_y3;
    }
    let adjusted_scan_line_y = scan_line_y - delay;

    // Scan line distance fade
    let mut pos = in_position;
    let y_pos = pos.y;
    let scan_line_dist = (adjusted_scan_line_y - y_pos).abs();

    let mut alpha = point_active;
    let mut color = vec3(0.8, 0.8, 0.8);
    if scan_line_dist > 0.0 && scan_line_dist < uniforms.scan_line_width {
        color = vec3(1.0, 1.0, 0.2);
    }

    // Cosine fade
    const PI: f32 = core::f32::consts::PI;
    let range = 100.0_f32;
    if uniforms.is_active == 1u32 {
        if y_pos > adjusted_scan_line_y {
            alpha = if scan_line_dist >= range {
                0.0
            } else {
                (scan_line_dist * PI / (range * 2.0)).cos().clamp(0.0, 1.0)
            };
        }
    } else {
        if y_pos < adjusted_scan_line_y {
            pos.y -= 0.05 * scan_line_dist * scan_line_dist;
            alpha = if scan_line_dist >= range {
                0.0
            } else {
                (scan_line_dist * PI / (range * 2.0)).cos().clamp(0.0, 1.0)
            };
        }
    }

    // Camera distance fade
    let mv_pos = uniforms.model_view * vec4(pos.x, pos.y, pos.z, 1.0);
    let view_z = -mv_pos.z;
    let dist_alpha = 1.0
        - ((view_z - uniforms.camera_fade_start)
            / (uniforms.camera_fade_distance - uniforms.camera_fade_start))
            .clamp(0.0, 1.0);

    // Layer weighting + final alpha
    let layer_weight = -0.25 * layer + 1.25;
    let final_alpha = 0.6 * alpha * dist_alpha * layer_weight * point_active;

    // Glitch effects (4 bands)
    let mut glitched = mv_pos;
    let glitches = [
        uniforms.glitch_effects_0,
        uniforms.glitch_effects_1,
        uniforms.glitch_effects_2,
        uniforms.glitch_effects_3
    ];
    for i in 0..4 {
        let glitch = glitches[i];
        let gy0 = glitch.x;
        let gx0 = glitch.y;
        let gy1 = glitch.z;
        let gx1 = glitch.w;

        let screen_y = glitched.y;
        if (screen_y - gy0).abs() < uniforms.glitch_y_range {
            glitched.x += uniforms.glitch_x_offset * gx0;
        }
        if (screen_y - gy1).abs() < uniforms.glitch_y_range {
            glitched.x += uniforms.glitch_x_offset * gx1;
        }
    }

    // Projection + billboard
    let proj = uniforms.projection * glitched;
    let screen_pos = vec2(proj.x / proj.w, proj.y / proj.w);
    let screen_uv = vec2((proj.x / proj.w + 1.0) * 0.5, (proj.y / proj.w + 1.0) * 0.5);
    let ps = (size * uniforms.point_size_scale * dist_alpha + 0.5) / uniforms.resolution_y;

    let (offset, coord): (Vec2, Vec2) = match vert_in_quad {
        0 => (vec2(-ps, ps), vec2(0.0, 1.0)),
        1 => (vec2(-ps, -ps), vec2(0.0, 0.0)),
        2 => (vec2(ps, ps), vec2(1.0, 1.0)),
        3 => (vec2(-ps, -ps), vec2(0.0, 0.0)),
        4 => (vec2(ps, -ps), vec2(1.0, 0.0)),
        _ => (vec2(ps, ps), vec2(1.0, 1.0)),
    };

    let final_xy = screen_pos + offset;

    *out_pos = vec4(final_xy.x, final_xy.y, proj.z / proj.w, 1.0);
    *out_coord = coord;
    *out_alpha = final_alpha;
    *out_color = color;
    *out_dist_alpha = dist_alpha;
    *out_screen_uv = screen_uv;
}

#[spirv(fragment)]
pub fn point_cloud_fs(
    #[spirv(uniform, descriptor_set = 0, binding = 0)] uniforms: &PointCloudUniforms,
    #[spirv(location = 0)] coord: Vec2,
    #[spirv(location = 1)] v_alpha: f32,
    #[spirv(location = 2)] v_color: Vec3,
    #[spirv(location = 3)] v_dist_alpha: f32,
    #[spirv(location = 4)] v_screen_uv: Vec2,
    output: &mut Vec4,
) {
    let center = coord - vec2(0.5, 0.5);
    let dist = center.length();
    let radius = 0.5_f32;

    if dist > radius {
        spirv_std::arch::kill();
    }

    let mut alpha = v_alpha;

    if dist > uniforms.core_radius * v_dist_alpha {
        let feather_start = uniforms.core_radius;
        let feather_end = radius;
        let fade1 = smoothstep(feather_end, feather_start, dist);
        let fade2 = smoothstep(feather_end * 0.8, feather_start, dist);
        let mix_ratio = uniforms.feather_width.clamp(0.1, 1.0);
        let feather_alpha = mix_f(fade1, fade2, mix_ratio);
        let dist_fade = 1.0 - (dist / radius).powf(2.0);
        let extra_feather = 1.0 - (dist / radius).powf(1.0 + uniforms.feather_width * 2.0);
        alpha = v_alpha * feather_alpha * dist_fade * extra_feather;
    }

    let inner_glow = 1.0 - smoothstep(0.0, uniforms.core_radius * 3.0, dist);
    let mut final_color = v_color + v_color * inner_glow * uniforms.inner_glow_strength;

    let compressed = final_color / (final_color + vec3(1.0, 1.0, 1.0));
    final_color = mix_v3(final_color, compressed, uniforms.compress_strength);

    let brightness = v_color.dot(vec3(0.299, 0.587, 0.114));
    if brightness > 0.6 {
        alpha *= 1.0 + (brightness - 0.6) * 0.5;
    }

    let uv = v_screen_uv;
    let left_x = (uv.x - 0.4) * 0.8 + 0.4;
    let right_x = (uv.x - 0.4) * 0.6 + 0.4;
    let mapped_x = if uv.x < 0.4 { left_x } else { right_x };
    let vignette_dist = vec2(mapped_x, uv.y).distance(vec2(0.4, 0.5));
    let mut vignette = 1.0_f32;
    if vignette_dist > 0.3 {
        vignette = 1.0 - smoothstep(0.4, 0.5, vignette_dist);
    }
    alpha *= vignette;

    *output = vec4(final_color.x, final_color.y, final_color.z, alpha);
}
