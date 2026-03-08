struct PointCloudUniforms {
    model_view: mat4x4<f32>,
    projection: mat4x4<f32>,
    scan_line_y1: f32,
    scan_line_y2: f32,
    scan_line_y3: f32,
    scan_line_width: f32,
    camera_fade_distance: f32,
    camera_fade_start: f32,
    feather_width: f32,
    core_radius: f32,
    inner_glow_strength: f32,
    compress_strength: f32,
    point_size_scale: f32,
    is_active: u32,
    resolution_x: f32,
    resolution_y: f32,
    glitch_y_range: f32,
    glitch_x_offset: f32,
    glitch_effects_0: vec4<f32>,
    glitch_effects_1: vec4<f32>,
    glitch_effects_2: vec4<f32>,
    glitch_effects_3: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: PointCloudUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) coord: vec2<f32>,
    @location(1) alpha: f32,
    @location(2) color: vec3<f32>,
    @location(3) dist_alpha: f32,
    @location(4) screen_uv: vec2<f32>,
}

@vertex
fn point_cloud_vs(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) point_data: vec4<f32>
) -> VertexOutput {
    // Each point rendered as quad: 6 vertices per point
    let vert_in_quad = vertex_index % 6u;
    let point_index = vertex_index / 6u;

    // Parse point attributes from location(1)
    let point_active = point_data.x;      // 0/1: is active
    let size = point_data.y;              // 4-8: point size
    let layer = point_data.z;             // 1-3: layer
    let delay = point_data.w;             // -100 to 100: delay offset

    var pos = position;

    // Scan Line Selection
    var scan_line_y = uniforms.scan_line_y1;
    if abs(layer - 2.0) < 0.1 {
        scan_line_y = uniforms.scan_line_y2;
    } else if abs(layer - 3.0) < 0.1 {
        scan_line_y = uniforms.scan_line_y3;
    }

    // Apply delay offset
    let adjusted_scan_line_y = scan_line_y - delay;

    // Scan Line Distance Fade
    let y_pos = pos.y;
    let scan_line_delta = adjusted_scan_line_y - y_pos;
    let scan_line_dist = abs(scan_line_delta);

    var alpha = point_active;
    var color = vec3<f32>(0.8, 0.8, 0.8); // Gray default

    if scan_line_dist > 0.0 && scan_line_dist < uniforms.scan_line_width {
        color = vec3<f32>(1.0, 1.0, 0.2); // Yellow near scan line
    }

    // Cosine fade-in/out based on scan line
    let range = 100.0;
    if uniforms.is_active == 1u {
        // Fade-in mode: points above scan line fade out
        if y_pos > adjusted_scan_line_y {
            if scan_line_dist >= range {
                alpha = 0.0;
            } else {
                alpha = clamp(cos(scan_line_dist * 3.14159265 / (range * 2.0)), 0.0, 1.0);
            }
        }
    } else {
        // Fade-out mode: points below scan line fade out + move down
        if y_pos < adjusted_scan_line_y {
            pos.y -= 0.05 * scan_line_dist * scan_line_dist;  // Position displacement
            if scan_line_dist >= range {
                alpha = 0.0;
            } else {
                alpha = clamp(cos(scan_line_dist * 3.14159265 / (range * 2.0)), 0.0, 1.0);
            }
        }
    }

    // Camera Distance Fade
    let mv_pos = uniforms.model_view * vec4<f32>(pos, 1.0);
    let view_z = -mv_pos.z;
    let fade_start = uniforms.camera_fade_start;
    let fade_end = uniforms.camera_fade_distance;
    let dist_alpha = 1.0 - clamp((view_z - fade_start) / (fade_end - fade_start), 0.0, 1.0);

    // Layer Weighting
    let layer_weight = -0.25 * layer + 1.25;

    // Final alpha combines all factors
    let final_alpha = 0.6 * alpha * dist_alpha * layer_weight * point_active;

    // Glitch Effects (4 distortion bands)
    var glitched_mv_pos = mv_pos;
    let glitches = array(
        uniforms.glitch_effects_0,
        uniforms.glitch_effects_1,
        uniforms.glitch_effects_2,
        uniforms.glitch_effects_3
    );

    for (var i = 0u; i < 4u; i = i + 1u) {
        let glitch = glitches[i];
        let gy0 = glitch.x;    // First band y (view space)
        let gx0 = glitch.y;    // First band x offset multiplier
        let gy1 = glitch.z;    // Second band y (view space)
        let gx1 = glitch.w;    // Second band x offset multiplier

        let screen_y = glitched_mv_pos.y;  // Apply in view space
        if abs(screen_y - gy0) < uniforms.glitch_y_range {
            glitched_mv_pos.x += uniforms.glitch_x_offset * gx0;
        }
        if abs(screen_y - gy1) < uniforms.glitch_y_range {
            glitched_mv_pos.x += uniforms.glitch_x_offset * gx1;
        }
    }

    // Now project the glitched position
    let proj_pos = uniforms.projection * glitched_mv_pos;
    var glitched_pos = proj_pos;

    // Billboard Quad Positioning
    let screen_pos = glitched_pos.xy / glitched_pos.w;
    // Dynamic point size based on uniform and distance alpha (matches GLSL: size * pointSizeScale * distanceAlpha + 4.0)
    // WebGL gl_PointSize is in pixels; we need to convert to NDC coordinates for manual quad rendering
    // NDC coordinate range is [-1, 1] which maps to the viewport
    // For a quad offset: 1 pixel ≈ 2.0 / resolution in NDC (full range is 2.0)
    // IMPORTANT: This differs from WebGL which uses gl_PointSize directly in pixels
    let base_point_size = size * uniforms.point_size_scale * dist_alpha + 0.5;
    let point_size = base_point_size / uniforms.resolution_y;  // Pixel to NDC conversion

    var offset: vec2<f32> = vec2<f32>(0.0);
    var coord: vec2<f32> = vec2<f32>(0.5);

    // Calculate screen UV for vignette effect (based on glitched position before billboard offset)
    // Map from clip space [-1, 1] to UV [0, 1]
    let screen_uv_x = (glitched_pos.x / glitched_pos.w + 1.0) * 0.5;
    let screen_uv_y = (glitched_pos.y / glitched_pos.w + 1.0) * 0.5;
    let screen_uv = vec2<f32>(screen_uv_x, screen_uv_y);

    switch (vert_in_quad) {
        case 0u: { // TL
            offset = vec2<f32>(-point_size, point_size);
            coord = vec2<f32>(0.0, 1.0);
        }
        case 1u: { // BL
            offset = vec2<f32>(-point_size, -point_size);
            coord = vec2<f32>(0.0, 0.0);
        }
        case 2u: { // TR
            offset = vec2<f32>(point_size, point_size);
            coord = vec2<f32>(1.0, 1.0);
        }
        case 3u: { // BL (duplicate for triangle)
            offset = vec2<f32>(-point_size, -point_size);
            coord = vec2<f32>(0.0, 0.0);
        }
        case 4u: { // BR
            offset = vec2<f32>(point_size, -point_size);
            coord = vec2<f32>(1.0, 0.0);
        }
        default: { // TR (duplicate for triangle)
            offset = vec2<f32>(point_size, point_size);
            coord = vec2<f32>(1.0, 1.0);
        }
    }

    let final_pos = screen_pos + offset;
    let final_ndc = vec4<f32>(final_pos, glitched_pos.z / glitched_pos.w, 1.0);

    return VertexOutput(
        final_ndc,
        coord,
        final_alpha,
        color,
        dist_alpha,
        screen_uv
    );
}

@fragment
fn point_cloud_fs(
    @location(0) coord: vec2<f32>,
    @location(1) v_alpha: f32,
    @location(2) v_color: vec3<f32>,
    @location(3) v_dist_alpha: f32,
    @location(4) v_screen_uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    // Circle Clipping
    let center = coord - vec2<f32>(0.5);
    let dist = length(center);
    let radius = 0.5;

    if dist > radius {
        discard;
    }

    var alpha = v_alpha;

    // Core + Feathering Region
    if dist <= uniforms.core_radius * v_dist_alpha {
        // Inner core region: preserve original alpha
        alpha = v_alpha;
    } else {
        // Feathering region
        let feather_start = uniforms.core_radius;
        let feather_end = radius;

        // Two-layer feathering
        let fade_out1 = smoothstep(feather_end, feather_start, dist);
        let fade_out2 = smoothstep(feather_end * 0.8, feather_start, dist);

        let mix_ratio = clamp(uniforms.feather_width, 0.1, 1.0);
        let feather_alpha = mix(fade_out1, fade_out2, mix_ratio);

        // Distance decay (squared)
        let distance_fade = 1.0 - pow(dist / radius, 2.0);

        // Additional feathering (adjustable exponent)
        let additional_feather = 1.0 - pow(dist / radius, 1.0 + uniforms.feather_width * 2.0);

        // Combine multiple decay factors
        alpha = v_alpha * feather_alpha * distance_fade * additional_feather;
    }

    // Inner Glow Effect
    let inner_glow = 1.0 - smoothstep(0.0, uniforms.core_radius * 3.0, dist);
    var final_color = v_color + v_color * inner_glow * uniforms.inner_glow_strength;

    // Brightness Compression (Tone Mapping)
    let compressed = final_color / (final_color + vec3<f32>(1.0));
    final_color = mix(final_color, compressed, uniforms.compress_strength);

    // Highlight Enhancement
    let color_brightness = dot(v_color, vec3<f32>(0.299, 0.587, 0.114));
    if color_brightness > 0.6 {
        alpha *= 1.0 + (color_brightness - 0.6) * 0.5;
    }

    // Vignette Effect (Asymmetric Lens Darkening)
    // Use screen UV passed from vertex shader
    let uv = v_screen_uv;

    // Asymmetric vignette: left side compressed (0.8x), right side relaxed (0.6x)
    // Fixed: correct select syntax - select(false_val, true_val, condition)
    let left_x = (uv.x - 0.4) * 0.8 + 0.4;
    let right_x = (uv.x - 0.4) * 0.6 + 0.4;
    let mapped_x = select(right_x, left_x, uv.x < 0.4);
    let vignette_pos = vec2<f32>(mapped_x, uv.y);
    let vignette_dist = distance(vignette_pos, vec2<f32>(0.4, 0.5));

    var vignette = 1.0;
    if vignette_dist > 0.3 {
        vignette = 1.0 - smoothstep(0.4, 0.5, vignette_dist);
    }
    alpha *= vignette;

    // Final Output
    return vec4<f32>(final_color, alpha);
}
