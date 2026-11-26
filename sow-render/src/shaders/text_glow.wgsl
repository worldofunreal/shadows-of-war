struct TextGlobals {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
    shader_params: array<vec4<f32>, 512>,
}

struct TextInstance {
    screen_pos: vec2<f32>,
    size: vec2<f32>,
    uv_rect: vec4<f32>,
    content_rect: vec4<f32>,
    color: vec4<f32>,
    outline_color: vec4<f32>,
    face_dilate: f32,
    outline_thickness: f32,
    underlay_offset_y: f32,
    underlay_softness: f32,
    kind: f32,
}

var<uniform> globals: TextGlobals;
var font_atlas: texture_2d<f32>;
var font_sampler: sampler;
var avatar_atlas: texture_2d<f32>;
var avatar_sampler: sampler;
var building_atlas: texture_2d<f32>;
var building_sampler: sampler;
var status_atlas: texture_2d<f32>;
var status_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) outline_color: vec4<f32>,
    @location(3) face_dilate: f32,
    @location(4) outline_thickness: f32,
    @location(5) underlay_offset_y: f32,
    @location(6) underlay_softness: f32,
    @location(7) uv_rect: vec4<f32>,
    @location(8) kind: f32,
    @location(9) local_uv: vec2<f32>,
    @location(10) content_rect: vec4<f32>,
    @location(11) quad_size: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: TextInstance) -> VertexOutput {
    var out: VertexOutput;
    let corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0),
        vec2(1.0, 0.0),
        vec2(1.0, 1.0),
        vec2(0.0, 0.0),
        vec2(1.0, 1.0),
        vec2(0.0, 1.0),
    );
    let local = corners[vi];

    var instance_scale = 1.0;
    if inst.kind > 1.5 && inst.kind < 2.5 { instance_scale = globals.shader_params[__TEXT_DISC_SCALE__].x; }
    if inst.kind > 3.5 && inst.kind < 4.5 { instance_scale = globals.shader_params[__TEXT_AVATAR_SIZE__].x; }
    if inst.kind > 10.5 && inst.kind < 11.5 { instance_scale = globals.shader_params[__TEXT_BUILDING_SIZE__].x; }
    if inst.kind > 11.5 && inst.kind < 12.5 { instance_scale = globals.shader_params[__TEXT_STATUS_SIZE__].x; }
    if inst.kind > 12.5 && inst.kind < 13.5 { instance_scale = globals.shader_params[__TEXT_EMOJI_SIZE__].x; }
    if inst.kind > 14.5 && inst.kind < 15.5 { instance_scale = globals.shader_params[__TEXT_BUILDING_SIZE__].x; }
    let center = inst.screen_pos + inst.size * 0.5;
    let screen = center + (inst.screen_pos + local * inst.size - center) * instance_scale;
    let ndc = vec2<f32>(
        screen.x / globals.screen_size.x * 2.0 - 1.0,
        1.0 - screen.y / globals.screen_size.y * 2.0,
    );

    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = mix(inst.uv_rect.xy, inst.uv_rect.zw, local);
    out.color = inst.color;
    out.outline_color = inst.outline_color;
    out.face_dilate = inst.face_dilate;
    out.outline_thickness = inst.outline_thickness;
    out.underlay_offset_y = inst.underlay_offset_y;
    out.underlay_softness = inst.underlay_softness;
    out.uv_rect = inst.uv_rect;
    out.kind = inst.kind;
    out.local_uv = local;
    out.content_rect = inst.content_rect;
    out.quad_size = inst.size * instance_scale;
    return out;
}

fn median(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

fn rounded_rect_distance(local: vec2<f32>, corner_radius_ratio: f32) -> f32 {
    let pixel_step = max(fwidth(local), vec2<f32>(1e-5));
    let pixels_per_local = 1.0 / pixel_step;
    let half_size = pixels_per_local * 0.5;
    let radius = clamp(corner_radius_ratio, 0.0, 0.5) * min(pixels_per_local.x, pixels_per_local.y);
    let p = abs(local - vec2<f32>(0.5, 0.5)) * pixels_per_local;
    let q = p - (half_size - vec2<f32>(radius, radius));
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

fn sample_atlas(uv: vec2<f32>, uv_rect: vec4<f32>) -> vec3<f32> {
    let clamped_uv = clamp(uv, uv_rect.xy, uv_rect.zw);
    let sampled = textureSample(font_atlas, font_sampler, clamped_uv).rgb;

    let eps = 1e-5;
    let inside_x = step(uv_rect.x - eps, uv.x) * step(uv.x, uv_rect.z + eps);
    let inside_y = step(uv_rect.y - eps, uv.y) * step(uv.y, uv_rect.w + eps);

    return sampled * inside_x * inside_y;
}

fn sample_icon(uv: vec2<f32>, lo: vec2<f32>, hi: vec2<f32>) -> vec4<f32> {
    let eps = 1e-5;
    let inside_x = step(lo.x - eps, uv.x) * step(uv.x, hi.x + eps);
    let inside_y = step(lo.y - eps, uv.y) * step(uv.y, hi.y + eps);
    return textureSample(status_atlas, status_sampler, clamp(uv, lo, hi)) * inside_x * inside_y;
}

// Procedural shapes (KIND_DISC / KIND_RING). For shapes uv_rect is [0,0,1,1], so `in.uv`
// is the local 0..1 quad coordinate. `outline_thickness` carries the ring width in pixels
// (converted to uv via fwidth). `color` is the fill/stroke color.
fn shade_shape(in: VertexOutput) -> vec4<f32> {
    let shape_span = max(in.content_rect.zw - in.content_rect.xy, vec2<f32>(1e-5));
    let shape_uv = (in.local_uv - in.content_rect.xy) / shape_span;
    let p = shape_uv - vec2<f32>(0.5, 0.5);
    let dist = length(p);
    let aa = max(fwidth(dist) * globals.shader_params[__TEXT_SHAPE_AA__].x, 1e-5);

    if (in.kind > 2.5) {
        // Ring: outer edge at uv radius 0.5, drawn inward by `outline_thickness` px.
        let tw = max(in.outline_thickness * globals.shader_params[__TEXT_RING_WIDTH__].x * globals.shader_params[__TEXT_SHAPE_STROKE__].x * fwidth(shape_uv).x, aa);
        let outer = 1.0 - smoothstep(0.5 - aa, 0.5, dist);
        let inner = 1.0 - smoothstep(0.5 - tw - aa, 0.5 - tw, dist);
        let a = clamp(outer - inner, 0.0, 1.0) * in.color.a * globals.shader_params[__TEXT_SHAPE_FILL_OPACITY__].x * globals.shader_params[__TEXT_OPACITY__].x;
        return vec4<f32>(in.color.rgb * globals.shader_params[__TEXT_SHAPE_STROKE_TINT__].rgb, a);
    }

    // Disc: filled circle.
    let a = (1.0 - smoothstep(0.5 - aa, 0.5, dist)) * in.color.a * globals.shader_params[__TEXT_SHAPE_FILL_OPACITY__].x * globals.shader_params[__TEXT_OPACITY__].x;
    return vec4<f32>(in.color.rgb * globals.shader_params[__TEXT_SHAPE_FILL_TINT__].rgb, a);
}

// Progress arc (KIND_ARC). The CPU packs rotation phase in uv_rect.y and sweep in uv_rect.z.
// Arcs do not sample a texture. The arc starts at 12 o'clock and advances clockwise.
fn shade_arc(in: VertexOutput) -> vec4<f32> {
    let shape_span = max(in.content_rect.zw - in.content_rect.xy, vec2<f32>(1e-5));
    let shape_uv = (in.local_uv - in.content_rect.xy) / shape_span;
    let p = shape_uv - vec2<f32>(0.5, 0.5);
    let dist = length(p);
    let aa = max(fwidth(dist) * globals.shader_params[__TEXT_SHAPE_AA__].x, 1e-5);
    let tw = max(in.outline_thickness * globals.shader_params[__TEXT_ARC_WIDTH__].x * globals.shader_params[__TEXT_SHAPE_STROKE__].x * fwidth(shape_uv).x, aa);
    let outer = 1.0 - smoothstep(0.5 - aa, 0.5, dist);
    let inner = 1.0 - smoothstep(0.5 - tw - aa, 0.5 - tw, dist);
    let ring_alpha = clamp(outer - inner, 0.0, 1.0);

    let tau = 6.28318530718;
    var angle = atan2(p.y, p.x) + 1.57079632679 + tau;
    if (angle >= tau) {
        angle -= tau;
    }
    let normalized_angle = angle / tau;
    let rotated_angle = fract(normalized_angle - fract(in.uv_rect.y) + 1.0);
    let progress = clamp(in.uv_rect.z, 0.0, 1.0);
    var arc_alpha = 1.0;
    if (progress < 1.0) {
        let angular_aa = max(fwidth(normalized_angle) * globals.shader_params[__TEXT_ARC_PROGRESS_AA__].x, 1e-5);
        arc_alpha = 1.0 - smoothstep(progress, progress + angular_aa, rotated_angle);
    }
    return vec4<f32>(in.color.rgb * globals.shader_params[__TEXT_SHAPE_STROKE_TINT__].rgb, ring_alpha * arc_alpha * in.color.a * globals.shader_params[__TEXT_OPACITY__].x);
}

// Circular portrait from the avatar atlas (heroes, campaigns and bots).
fn shade_sprite(in: VertexOutput) -> vec4<f32> {
    let span = max(in.uv_rect.zw - in.uv_rect.xy, vec2<f32>(1e-6));
    let local = (in.uv - in.uv_rect.xy) / span;
    let d = rounded_rect_distance(local, 0.5);
    let aa = max(fwidth(d), 1e-5);
    let mask = 1.0 - smoothstep(-aa, aa, d);
    let uv = clamp(in.uv, in.uv_rect.xy, in.uv_rect.zw);
    let tex = textureSample(avatar_atlas, avatar_sampler, uv);
    return vec4<f32>(tex.rgb * in.color.rgb * globals.shader_params[__TEXT_AVATAR_TINT__].rgb, tex.a * mask * in.color.a * globals.shader_params[__TEXT_AVATAR_OPACITY__].x * globals.shader_params[__TEXT_OPACITY__].x);
}

fn shade_building_sprite(in: VertexOutput) -> vec4<f32> {
    let tex = textureSample(
        building_atlas,
        building_sampler,
        clamp(in.uv, in.uv_rect.xy, in.uv_rect.zw),
    );
    return vec4<f32>(tex.rgb * in.color.rgb * globals.shader_params[__TEXT_BUILDING_TINT__].rgb, tex.a * in.color.a * globals.shader_params[__TEXT_BUILDING_OPACITY__].x * globals.shader_params[__TEXT_OPACITY__].x);
}

fn shade_status_sprite(in: VertexOutput) -> vec4<f32> {
    let tex = textureSample(
        status_atlas,
        status_sampler,
        clamp(in.uv, in.uv_rect.xy, in.uv_rect.zw),
    );
    let tinted = tex.rgb * in.color.rgb * globals.shader_params[__TEXT_STATUS_TINT__].rgb;
    let luminance = dot(tinted, vec3<f32>(0.2126, 0.7152, 0.0722));
    let rgb = mix(tinted, vec3<f32>(luminance), clamp(in.face_dilate, 0.0, 1.0));
    return vec4<f32>(rgb, tex.a * in.color.a * globals.shader_params[__TEXT_STATUS_OPACITY__].x * globals.shader_params[__TEXT_OPACITY__].x);
}

// Anti-aliased filled rectangle (KIND_RECT). Its atlas UVs are constant; local_uv is the
// 0..1 quad coordinate used to calculate edge coverage and keep the fill visible.
fn shade_rect(in: VertexOutput) -> vec4<f32> {
    let aa = max(fwidth(in.local_uv), vec2<f32>(1e-5));
    let x_alpha = smoothstep(0.0, aa.x, in.local_uv.x) * (1.0 - smoothstep(1.0 - aa.x, 1.0, in.local_uv.x));
    let y_alpha = smoothstep(0.0, aa.y, in.local_uv.y) * (1.0 - smoothstep(1.0 - aa.y, 1.0, in.local_uv.y));
    let alpha = x_alpha * y_alpha * in.color.a * globals.shader_params[__TEXT_RECT_OPACITY__].x * globals.shader_params[__TEXT_OPACITY__].x;
    return vec4<f32>(in.color.rgb * globals.shader_params[__TEXT_SHAPE_FILL_TINT__].rgb, alpha);
}

fn shade_rounded_rect(in: VertexOutput) -> vec4<f32> {
    let dist = rounded_rect_distance(in.local_uv, in.face_dilate * globals.shader_params[__TEXT_ROUNDED_RADIUS__].x);
    let aa = max(fwidth(dist), 1e-5);
    let outer = 1.0 - smoothstep(-aa, aa, dist);
    let border_width = max(in.outline_thickness * globals.shader_params[__TEXT_ROUNDED_STROKE__].x * globals.shader_params[__TEXT_SHAPE_STROKE__].x, 0.0);
    let inner = 1.0 - smoothstep(-aa, aa, dist + border_width);
    let edge = max(outer - inner, 0.0);
    let alpha = in.color.a * inner + in.outline_color.a * edge * globals.shader_params[__TEXT_OPACITY__].x;
    if (alpha <= 1e-5) {
        return vec4<f32>(0.0);
    }
    let rgb = (in.color.rgb * globals.shader_params[__TEXT_SHAPE_FILL_TINT__].rgb * in.color.a * inner + in.outline_color.rgb * globals.shader_params[__TEXT_SHAPE_STROKE_TINT__].rgb * in.outline_color.a * edge) / alpha;
    return vec4<f32>(rgb, alpha);
}

// Anti-aliased filled downward triangle (KIND_TRIANGLE). `local_uv` is the full quad; the
// triangle occupies its bounding box with a flat top and a point at the bottom.
fn shade_triangle(in: VertexOutput) -> vec4<f32> {
    let p = vec2<f32>(in.local_uv.x, select(in.local_uv.y, 1.0 - in.local_uv.y, in.face_dilate > 0.5));
    let half_width = 0.5 * (1.0 - p.y);
    let horizontal = half_width - abs(p.x - 0.5);
    let vertical = min(p.y, 1.0 - p.y);
    let signed_distance = min(horizontal, vertical);
    let aa = max(fwidth(signed_distance), 1e-5);
    let alpha = 1.0 - smoothstep(-aa, aa, -signed_distance);
    return vec4<f32>(in.color.rgb * globals.shader_params[__TEXT_SHAPE_FILL_TINT__].rgb, alpha * in.color.a * globals.shader_params[__TEXT_TRIANGLE_OPACITY__].x * globals.shader_params[__TEXT_OPACITY__].x);
}

// Anti-aliased diagonal cross (KIND_CROSS). The content rectangle excludes the
// outer fringe reserved by the CPU geometry helper.
fn shade_cross(in: VertexOutput) -> vec4<f32> {
    let shape_span = max(in.content_rect.zw - in.content_rect.xy, vec2<f32>(1e-5));
    let shape_uv = (in.local_uv - in.content_rect.xy) / shape_span;
    let p = shape_uv - vec2<f32>(0.5, 0.5);
    let distance_to_line = min(abs(p.y - p.x), abs(p.y + p.x));
    let aa = max(fwidth(distance_to_line), 1e-5);
    let pixel = max(fwidth(shape_uv).x, fwidth(shape_uv).y);
    let width = max(in.outline_thickness * globals.shader_params[__TEXT_CROSS_WIDTH__].x * globals.shader_params[__TEXT_SHAPE_STROKE__].x * pixel * 0.7, aa);
    let alpha = 1.0 - smoothstep(width, width + aa, distance_to_line);
    return vec4<f32>(in.color.rgb * globals.shader_params[__TEXT_SHAPE_STROKE_TINT__].rgb, alpha * in.color.a * globals.shader_params[__TEXT_OPACITY__].x);
}

// Icons-atlas art with an alpha-dilated outline and drop shadow (KIND_ICON).
fn shade_icon(in: VertexOutput) -> vec4<f32> {
    let lo = in.uv_rect.xy;
    let hi = in.uv_rect.zw;
    let span = hi - lo;
    let content_span = max(in.content_rect.zw - in.content_rect.xy, vec2<f32>(1e-6));
    let content_local = (in.local_uv - in.content_rect.xy) / content_span;
    let icon_uv = mix(lo, hi, content_local);
    let content_diameter = min(in.quad_size.x * content_span.x, in.quad_size.y * content_span.y);
    let reference_diameter = max(in.face_dilate, 1.0);
    let outline_scale = clamp(max(content_diameter / reference_diameter, globals.shader_params[__TEXT_EMOJI_MIN_OUTLINE__].x), 0.0, 1.0);

    let fw = fwidth(icon_uv);
    var ow = vec2<f32>(0.0);
    let outline_control = globals.shader_params[__TEXT_EMOJI_OUTLINE_SCALE__].x;
    if (in.outline_thickness > 0.0 && outline_scale > 0.0 && outline_control > 0.0) {
        ow = max(fw * in.outline_thickness * outline_scale * outline_control, span * (0.01 * outline_control));
    }
    var so = 0.0;
    let shadow_control = globals.shader_params[__TEXT_EMOJI_SHADOW_SCALE__].x;
    if (in.underlay_offset_y > 0.0 && outline_scale > 0.0 && shadow_control > 0.0) {
        so = max(fw.y * in.underlay_offset_y * outline_scale * shadow_control, span.y * (0.01 * shadow_control));
    }

    let center = sample_icon(icon_uv, lo, hi);

    var ring = sample_icon(icon_uv + vec2<f32>( ow.x, 0.0), lo, hi).a;
    ring = max(ring, sample_icon(icon_uv + vec2<f32>(-ow.x, 0.0), lo, hi).a);
    ring = max(ring, sample_icon(icon_uv + vec2<f32>(0.0,  ow.y), lo, hi).a);
    ring = max(ring, sample_icon(icon_uv + vec2<f32>(0.0, -ow.y), lo, hi).a);
    ring = max(ring, sample_icon(icon_uv + vec2<f32>( ow.x,  ow.y), lo, hi).a);
    ring = max(ring, sample_icon(icon_uv + vec2<f32>( ow.x, -ow.y), lo, hi).a);
    ring = max(ring, sample_icon(icon_uv + vec2<f32>(-ow.x,  ow.y), lo, hi).a);
    ring = max(ring, sample_icon(icon_uv + vec2<f32>(-ow.x, -ow.y), lo, hi).a);

    let shadow_a = sample_icon(icon_uv - vec2<f32>(0.0, so), lo, hi).a;

    let fill_a = center.a * in.color.a * globals.shader_params[__TEXT_EMOJI_OPACITY__].x * globals.shader_params[__TEXT_OPACITY__].x;
    let dark_a = max(ring, shadow_a) * in.outline_color.a * globals.shader_params[__TEXT_EMOJI_OPACITY__].x * globals.shader_params[__TEXT_OPACITY__].x;
    let out_a = max(fill_a, dark_a);
    if (out_a <= 0.001) {
        // Transparent (not `discard`): keeps this a plain helper so the
        // Naga GLSL/WebGL path stays happy; equivalent under alpha blending.
        return vec4<f32>(0.0);
    }
    let fill_ratio = fill_a / max(out_a, 0.0001);
    let outline_tint = globals.shader_params[__TEXT_EMOJI_TINT__].rgb * globals.shader_params[__TEXT_EMOJI_OUTLINE_TINT__].rgb;
    let rgb = mix(in.outline_color.rgb * outline_tint, center.rgb * in.color.rgb * globals.shader_params[__TEXT_EMOJI_TINT__].rgb, fill_ratio);
    return vec4<f32>(rgb, out_a);
}

// Landing site sonar. Keep the center quiet for the arrival countdown.
fn shade_landing_beacon(in: VertexOutput) -> vec4<f32> {
    let p = 2.0 * (in.local_uv - in.content_rect.xy) / (in.content_rect.zw-in.content_rect.xy) - 1.0;
    let d = length(p);
    let aa = max(fwidth(d),0.012);
    let rim = 1.0 - smoothstep(aa, 2.0*aa, abs(d-0.89));
    let bearing = atan2(p.y,p.x)/6.2831853 + 0.5;
    let arc = fract(4.0*(bearing-in.uv_rect.y));
    let brackets = smoothstep(0.015,0.08,arc)*(1.0-smoothstep(0.57,0.65,arc));
    let sweep = fract(in.uv_rect.y-bearing);
    let dial = smoothstep(0.42,0.52,d)*(1.0-smoothstep(0.85,0.89,d));
    let echo = 1.0-smoothstep(aa,2.0*aa,abs(d-(0.43+0.43*fract(in.uv_rect.y*2.0))));
    let marks = 1.0-smoothstep(0.023,0.060,min(abs(p.x),abs(p.y)));
    let signal = max(rim*brackets,max(dial*(0.43*echo+0.66*exp(-12.0*sweep)), marks*smoothstep(0.76,0.84,d)*(1.0-smoothstep(0.94,1.0,d))));
    let alpha = max(0.32*(1.0-smoothstep(0.87,0.94,d)),signal);
    let tint = mix(in.color.rgb,vec3<f32>(0.63,0.98,1.0),0.55);
    return vec4<f32>(mix(vec3<f32>(0.02,0.06,0.08),tint,clamp(signal/max(alpha,0.001),0.0,1.0)),alpha*in.color.a);
}

// Tactical ground selection / hover ghost ring (KIND_SELECTION_RING = 14.0).
// Single-quad procedural SDF: incandescent inner ring, 6 rotating outer segmented
// arcs, diagonal lock-on ticks, soft inward ground pool and radial neon bloom.
fn shade_selection_ring(in: VertexOutput) -> vec4<f32> {
    let p = in.local_uv * 2.0 - vec2<f32>(1.0, 1.0);
    let d = length(p);
    // Compute derivatives in uniform control flow before any early return.
    let aa = max(fwidth(d) * 1.15, 0.004);
    if (d >= 1.0) {
        return vec4<f32>(0.0);
    }

    let mode = clamp(in.uv_rect.x, 0.0, 1.0);
    let phase = in.uv_rect.y;
    let pulse = clamp(in.uv_rect.z, 0.0, 1.0);
    let acquire = clamp(in.uv_rect.w, 0.0, 1.0);

    let r_main = 0.70;
    let r_outer = 0.84;
    let dist_main = abs(d - r_main);
    let dist_outer = abs(d - r_outer);

    // Primary luminous ring with bright incandescent core.
    let w_main = mix(0.017, 0.024, mode) + aa * 0.45;
    let ring_main = 1.0 - smoothstep(w_main - aa, w_main + aa, dist_main);
    let ring_core = 1.0 - smoothstep(w_main * 0.36 - aa * 0.5, w_main * 0.36 + aa * 0.7, dist_main);

    // Outer concentric 6-segment tactical arcs (analytical angular AA avoids atan2 seam).
    let tau = 6.28318530718;
    let norm_ang = atan2(p.y, p.x) / tau + 0.5;
    let seg_count = 6.0;
    let seg_pos = fract((norm_ang - phase) * seg_count);
    let seg_edge = abs(seg_pos - 0.5);
    let ang_aa = max(aa / (max(d, 0.25) * tau) * seg_count * 1.4, 0.012);
    let seg_half = mix(0.31, 0.37, mode);
    let seg_mask = 1.0 - smoothstep(seg_half - ang_aa, seg_half + ang_aa, seg_edge);
    let w_outer = mix(0.014, 0.022, mode) + aa * 0.35;
    let outer_ring = (1.0 - smoothstep(w_outer - aa, w_outer + aa, dist_outer)) * seg_mask;
    let outer_core = (1.0 - smoothstep(w_outer * 0.38 - aa * 0.5, w_outer * 0.38 + aa * 0.6, dist_outer)) * seg_mask;

    // Four 45-degree inner tactical lock-on ticks (Cartesian symmetry, active when selected).
    let diag = min(abs(p.x - p.y), abs(p.x + p.y)) * 0.70710678;
    let tick_w = 0.017 + aa * 0.45;
    let tick_line = 1.0 - smoothstep(tick_w - aa, tick_w + aa, diag);
    let tick_rad = smoothstep(0.57 - aa, 0.57 + aa, d) * (1.0 - smoothstep(0.68 - aa, 0.68 + aa, d));
    let ticks = tick_line * tick_rad * mode;

    // Volumetric neon glow + inward ground aura + lock-on acquisition ripple.
    let edge_fade = 1.0 - smoothstep(0.92, 1.0, d);
    let glow_tight = exp(-14.0 * dist_main);
    let glow_soft = exp(-5.8 * dist_main) * edge_fade;
    let inner_pool = smoothstep(0.16, r_main, d) * (1.0 - smoothstep(r_main, r_main + 0.06, d));
    let inner_aura = inner_pool * inner_pool * mix(0.16, 0.32, mode);
    let outer_glow = exp(-18.0 * dist_outer) * seg_mask * mix(0.28, 0.46, mode) * edge_fade;
    let sweep_r = mix(0.46, 0.93, 1.0 - acquire);
    let sweep_ring = exp(-22.0 * abs(d - sweep_r)) * acquire * 0.85 * edge_fade;

    let breath = 1.0 + (0.10 * pulse + 0.35 * acquire) * mode;
    let glow_strength = max(in.outline_color.a, 0.0);
    let lit = clamp(ring_main * 1.05 + outer_ring * 0.90 + ticks * 0.92, 0.0, 1.0);
    let core = clamp(ring_core * 0.85 + outer_core * 0.48 + ticks * 0.45 + sweep_ring * 0.50, 0.0, 1.0);
    let bloom = (glow_tight * 0.50 + glow_soft * 0.24 + inner_aura + outer_glow + sweep_ring) * glow_strength * breath;

    // Subtle dark contrast rim behind the main ring keeps bright highlights crisp over light terrain.
    let rim_shadow = (1.0 - smoothstep(w_main + 0.026, w_main + 0.052, dist_main)) * (1.0 - ring_main) * 0.28 * mode;
    let energy_a = clamp(max(lit, bloom), 0.0, 1.0);
    let combined_a = clamp(max(energy_a, rim_shadow), 0.0, 1.0) * edge_fade;
    let out_a = combined_a * in.color.a * globals.shader_params[__TEXT_OPACITY__].x;
    if (out_a <= 0.001) {
        return vec4<f32>(0.0);
    }

    let energy_rgb = mix(in.color.rgb, in.outline_color.rgb, clamp(core + acquire * 0.35, 0.0, 1.0))
        * (1.0 + 0.25 * core * mode + 0.45 * acquire);
    let shadow_weight = clamp((rim_shadow * (1.0 - energy_a)) / max(combined_a, 0.001), 0.0, 0.65);
    let final_rgb = mix(energy_rgb, vec3<f32>(0.02, 0.04, 0.05), shadow_weight);
    return vec4<f32>(final_rgb, out_a);
}

fn rgb2hsv(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = mix(vec4<f32>(c.bg, k.wz), vec4<f32>(c.gb, k.xy), vec4<f32>(step(c.b, c.g)));
    let q = mix(vec4<f32>(p.xyw, c.r), vec4<f32>(c.r, p.yzx), vec4<f32>(step(p.x, c.r)));
    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;
    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

fn hsv2rgb(c: vec3<f32>) -> vec3<f32> {
    let p = abs(fract(c.xxx + vec3<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - vec3<f32>(3.0));
    return c.z * mix(vec3<f32>(1.0), clamp(p - vec3<f32>(1.0), vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(c.y));
}

fn scale_rgb_value(rgb: vec3<f32>, v_scale: f32) -> vec3<f32> {
    var hsv = rgb2hsv(max(rgb, vec3<f32>(0.0)));
    hsv.z = clamp(hsv.z * v_scale, 0.0, 1.0);
    return hsv2rgb(hsv);
}

// Signed distance to a regular n-gon in Y-down local quad space (negative inside).
fn sd_polygon(p: vec2<f32>, radius: f32, sides: f32, rot: f32) -> f32 {
    let an = 3.14159265 / sides;
    let raw_a = atan2(p.y, p.x) - rot + an;
    let period = 2.0 * an;
    let a = (raw_a - period * floor(raw_a / period)) - an;
    return length(p) * cos(a) - radius * cos(an);
}

// Per-BuildingKind SDF shape (0=City, 1=MissileSilo, 2=SAM, 3=Port, 4=TradingPost, 5=Tower, 6=MilitaryCamp).
fn building_shape_sdf(p: vec2<f32>, radius: f32, shape_id: f32) -> f32 {
    if (shape_id < 0.5) {
        return length(p) - radius;                                  // City -> circle
    }
    if (shape_id < 1.5) {
        return sd_polygon(p, radius * 1.12, 3.0, 1.5707963);        // MissileSilo -> triangle (vertex up)
    }
    if (shape_id < 2.5) {
        return sd_polygon(p, radius * 1.08, 4.0, 0.0);              // SAM -> square (flat sides)
    }
    if (shape_id < 3.5) {
        return sd_polygon(p, radius * 1.04, 5.0, 1.5707963);        // Port -> pentagon (vertex up)
    }
    if (shape_id < 4.5) {
        return sd_polygon(p, radius * 1.04, 6.0, 1.5707963);        // TradingPost -> hexagon (flat top)
    }
    if (shape_id < 5.5) {
        return sd_polygon(p, radius * 1.02, 8.0, 0.0);              // Tower -> octagon (flat sides)
    }
    return sd_polygon(p, radius * 1.12, 4.0, 0.7853982);            // MilitaryCamp -> diamond
}

// Procedural SDF building LOD marker (KIND_BUILDING_MARKER = 15.0).
// Draws an OpenFront-style player-colored polygon with crisp contrast border, optional
// silhouette glyph extracted from building_atlas.a, density stack rim, hover/selection
// highlight, and smooth cross-fade into the full building sprite.
fn shade_building_marker(in: VertexOutput) -> vec4<f32> {
    let p = in.local_uv - vec2<f32>(0.5, 0.5);
    let base_radius = 0.34;
    let shape_id = in.content_rect.x;
    let glyph_alpha = clamp(in.content_rect.y, 0.0, 1.0);
    let sprite_blend = clamp(in.content_rect.z, 0.0, 1.0);
    let stack_hint = clamp(in.content_rect.w, 0.0, 1.0);
    let icon_fill = clamp(in.face_dilate, 0.35, 1.0);
    let highlight_w = max(in.outline_thickness, 0.0);
    let under_construction = in.outline_color.a > 0.5;

    // Uniform control flow: compute derivatives and sample atlas before any early return.
    let sdf = building_shape_sdf(p, base_radius, shape_id);
    let fw = max(fwidth(length(p)), 1e-4);

    let uv_span_ratio = mix(0.68 * icon_fill, 0.68, sprite_blend);
    let art_local = p / uv_span_ratio + vec2<f32>(0.5, 0.5);
    let art_in_bounds = step(0.0, art_local.x) * step(art_local.x, 1.0)
        * step(0.0, art_local.y) * step(art_local.y, 1.0);
    let art_uv = mix(
        in.uv_rect.xy,
        in.uv_rect.zw,
        clamp(art_local, vec2<f32>(0.0), vec2<f32>(1.0)),
    );
    let tex = textureSample(building_atlas, building_sampler, art_uv);

    let icon_local = p / (0.68 * icon_fill) + vec2<f32>(0.5, 0.5);
    let icon_in_bounds = step(0.0, icon_local.x) * step(icon_local.x, 1.0)
        * step(0.0, icon_local.y) * step(icon_local.y, 1.0);
    let icon_uv = vec2<f32>(
        mix(in.underlay_offset_y, in.underlay_softness, clamp(icon_local.x, 0.0, 1.0)),
        clamp(icon_local.y, 0.0, 1.0),
    );
    let icon_tex = textureSample(status_atlas, status_sampler, icon_uv);

    let fill_darken = globals.shader_params[__TEXT_LOD_BUILDING_FILL_DARKEN__].x;
    let border_darken = globals.shader_params[__TEXT_LOD_BUILDING_BORDER_DARKEN__].x;
    let glyph_darken = globals.shader_params[__TEXT_LOD_BUILDING_GLYPH_DARKEN__].x;

    let fill_rgb = select(scale_rgb_value(in.color.rgb, fill_darken), in.color.rgb, under_construction);
    let border_rgb = select(scale_rgb_value(in.outline_color.rgb, border_darken), in.outline_color.rgb, under_construction);

    let border_w = 0.068;
    let shape_alpha = 1.0 - smoothstep(-fw, fw, sdf);
    let inner_mask = 1.0 - smoothstep(-fw, fw, sdf + border_w);
    var badge_rgb = mix(border_rgb, fill_rgb, inner_mask);

    // Dedicated LOD icon glyph sampled from status_atlas in mid-zoom tactical badge mode.
    let fill_lum = dot(fill_rgb, vec3<f32>(0.299, 0.587, 0.114));
    let glyph_rgb = select(
        scale_rgb_value(fill_rgb, glyph_darken),
        vec3<f32>(0.96, 0.97, 0.99),
        fill_lum < 0.25,
    );
    let glyph_mask = icon_tex.a * icon_in_bounds * inner_mask * glyph_alpha;
    badge_rgb = mix(badge_rgb, glyph_rgb, glyph_mask);

    // Subtle concentric outer rim indicating dense same-owner neighbors were thinned here.
    let stack_outer = 1.0 - smoothstep(-fw, fw, sdf - 0.064);
    let stack_inner = 1.0 - smoothstep(-fw, fw, sdf - 0.028);
    let stack_ring = clamp(stack_outer - stack_inner, 0.0, 1.0) * stack_hint * (1.0 - sprite_blend);

    // Hover / selection highlight outline band.
    let hl_outer = 1.0 - smoothstep(-fw, fw, sdf - highlight_w);
    let hl_band = clamp(hl_outer - shape_alpha, 0.0, 1.0) * step(0.001, highlight_w);

    let badge_a = clamp(max(shape_alpha, max(stack_ring * 0.85, hl_band)), 0.0, 1.0);
    var marker_rgb = badge_rgb;
    let stack_rgb = mix(fill_rgb, vec3<f32>(1.0), 0.55);
    marker_rgb = mix(marker_rgb, stack_rgb, stack_ring * (1.0 - shape_alpha));
    marker_rgb = mix(marker_rgb, vec3<f32>(0.96, 0.99, 1.0), hl_band);
    let marker_out_a = badge_a * (1.0 - sprite_blend) * in.color.a;

    // Cross-fade full-color building art over the fading SDF badge in the transition band.
    let sprite_rgb = tex.rgb * globals.shader_params[__TEXT_BUILDING_TINT__].rgb;
    let sprite_out_a = tex.a * art_in_bounds * sprite_blend * in.color.a;

    let combined_a = sprite_out_a + marker_out_a * (1.0 - sprite_out_a);
    let final_a = combined_a
        * globals.shader_params[__TEXT_BUILDING_OPACITY__].x
        * globals.shader_params[__TEXT_OPACITY__].x;
    if (final_a <= 0.001) {
        return vec4<f32>(0.0);
    }
    let combined_rgb = (sprite_rgb * sprite_out_a + marker_rgb * marker_out_a * (1.0 - sprite_out_a))
        / max(combined_a, 0.0001);
    return vec4<f32>(combined_rgb, final_a);
}

fn shade_text(in: VertexOutput) -> vec4<f32> {
    if (in.kind > 14.5) {
        return shade_building_marker(in);
    }
    if (in.kind > 13.5) {
        return shade_selection_ring(in);
    }
    if (in.kind > 12.5) {
        return shade_icon(in);
    }
    if (in.kind > 11.5) {
        return shade_status_sprite(in);
    }
    if (in.kind > 10.5) {
        return shade_building_sprite(in);
    }
    if (in.kind > 9.5) {
        return shade_rounded_rect(in);
    }
    if (in.kind > 8.5) { return shade_landing_beacon(in); }
    if (in.kind > 7.5) {
        return shade_arc(in);
    }
    if (in.kind > 6.5) {
        return shade_cross(in);
    }
    if (in.kind > 5.5) {
        return shade_triangle(in);
    }
    if (in.kind > 4.5) {
        return shade_rect(in);
    }
    if (in.kind > 3.5) {
        return shade_sprite(in);
    }
    if (in.kind > 1.5) {
        return shade_shape(in);
    }
    // MSDF text path
    let uv_dy = dpdy(in.uv);

    let unitRange = vec2<f32>(16.0, 16.0) / vec2<f32>(textureDimensions(font_atlas, 0));
    let screenTexSize = 1.0 / fwidth(in.uv);
    let screenPxRange = max(0.5 * dot(unitRange, screenTexSize), 1.0);

    let msd_c = sample_atlas(in.uv, in.uv_rect);
    let sd_c = median(msd_c.r, msd_c.g, msd_c.b);
    let centerDist = screenPxRange * (sd_c - 0.5) + in.face_dilate * globals.shader_params[__TEXT_SDF_FILL_GAIN__].x;

    let fillAlpha = clamp(centerDist + 0.5, 0.0, 1.0);
    let outlineAlpha = clamp(in.outline_thickness * globals.shader_params[__TEXT_SDF_OUTLINE_GAIN__].x + centerDist + 0.5, 0.0, 1.0) * (1.0 - fillAlpha);

    let shadowUv = in.uv - uv_dy * in.underlay_offset_y * globals.shader_params[__TEXT_SHADOW_OFFSET_SCALE__].x;
    let msd_s = sample_atlas(shadowUv, in.uv_rect);
    let sd_s = median(msd_s.r, msd_s.g, msd_s.b);
    let shadowDist = screenPxRange * (sd_s - 0.5);
    let softness = max(in.underlay_softness * globals.shader_params[__TEXT_SHADOW_SOFTNESS_SCALE__].x * screenPxRange, 0.001);
    let shadowAlpha = clamp((shadowDist + 0.5) / (1.0 + softness), 0.0, 1.0) * globals.shader_params[__TEXT_SHADOW_GAIN__].x;

    let fill_a = fillAlpha * in.color.a * globals.shader_params[__TEXT_OPACITY__].x;
    let black_a = max(outlineAlpha, shadowAlpha) * in.outline_color.a * globals.shader_params[__TEXT_OPACITY__].x;
    let final_a = max(fill_a, black_a);
    let fill_ratio = fill_a / max(final_a, 0.0001);
    let final_rgb = mix(in.outline_color.rgb * globals.shader_params[__TEXT_OUTLINE_TINT__].rgb, in.color.rgb * globals.shader_params[__TEXT_FILL_TINT__].rgb, fill_ratio);

    return vec4<f32>(final_rgb, final_a);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return shade_text(in);
}

// WebGL canvas is a plain UNORM surface (no hardware linear->sRGB encode on
// write), so we encode here to match the sRGB swapchain used natively. Picked
// by surface format at pipeline creation — same contract as map.wgsl.
@fragment
fn fs_main_srgb(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = shade_text(in);
    return vec4<f32>(pow(max(c.rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2)), c.a);
}
