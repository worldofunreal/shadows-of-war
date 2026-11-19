// GPU-instanced localized blast VFX pass (ground scorch, nuke fireball/shockwave/embers,
// SAM mid-air airbursts, and wave-synced building vaporization pops).
//
// Zero fullscreen cost: each active blast rasterizes only an inscribed octagon scaled tightly
// to its current visible radius, and the pass is skipped entirely when no blasts are active.

struct BlastGlobals {
    camera_pos: vec2<f32>,
    zoom: f32,
    time: f32,
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

// Per-instance attributes (instanced vertex buffer, divisor = 1).
// Field names MUST match `BlastInstanceGpu` in blast_renderer.rs.
struct BlastInstance {
    world_pos: vec2<f32>,
    bound_radius: f32,
    inner_radius: f32,
    outer_radius: f32,
    age_secs: f32,
    expand_dur: f32,
    lifetime_secs: f32,
    kind: f32,
    seed: f32,
}

var<uniform> globals: BlastGlobals;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    // (bound_radius, inner_radius, outer_radius, age_secs)
    @location(1) radii_age: vec4<f32>,
    // (expand_dur, lifetime_secs, kind, seed)
    @location(2) params: vec4<f32>,
}

fn world_to_clip(world: vec2<f32>) -> vec4<f32> {
    let screen = world * globals.zoom + globals.camera_pos;
    let ndc = vec2<f32>(
        screen.x / globals.screen_size.x * 2.0 - 1.0,
        1.0 - screen.y / globals.screen_size.y * 2.0,
    );
    return vec4<f32>(ndc, 0.0, 1.0);
}

const OCT_T: f32 = 0.416;

// Regular octagon circumscribing the unit circle (r = 1.0): 6 triangles = 18 vertices.
// Cuts ~17% corner fragment overdraw compared to a square quad.
@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: BlastInstance) -> VertexOutput {
    let oct = array<vec2<f32>, 18>(
        // Center strip (2 triangles)
        vec2<f32>(-OCT_T, -1.0),
        vec2<f32>( OCT_T, -1.0),
        vec2<f32>(-OCT_T,  1.0),
        vec2<f32>( OCT_T, -1.0),
        vec2<f32>( OCT_T,  1.0),
        vec2<f32>(-OCT_T,  1.0),
        // Left trapezoid (2 triangles)
        vec2<f32>(-1.0, -OCT_T),
        vec2<f32>(-OCT_T, -1.0),
        vec2<f32>(-OCT_T,  1.0),
        vec2<f32>(-1.0, -OCT_T),
        vec2<f32>(-OCT_T,  1.0),
        vec2<f32>(-1.0,  OCT_T),
        // Right trapezoid (2 triangles)
        vec2<f32>( OCT_T, -1.0),
        vec2<f32>(1.0, -OCT_T),
        vec2<f32>( OCT_T,  1.0),
        vec2<f32>(1.0, -OCT_T),
        vec2<f32>(1.0,  OCT_T),
        vec2<f32>( OCT_T,  1.0),
    );
    let local = oct[vi];
    let world = inst.world_pos + local * inst.bound_radius;

    var out: VertexOutput;
    out.clip_position = world_to_clip(world);
    out.local_pos = local;
    out.radii_age = vec4<f32>(
        inst.bound_radius,
        inst.inner_radius,
        inst.outer_radius,
        inst.age_secs,
    );
    out.params = vec4<f32>(
        inst.expand_dur,
        inst.lifetime_secs,
        inst.kind,
        inst.seed,
    );
    return out;
}

// ── Fast sin-free hashes & smooth value noise (mobile / WebGL2 safe) ────────

fn hash11(p_in: f32) -> f32 {
    var p = fract(p_in * 0.1031);
    p *= p + 33.33;
    p *= p + p;
    return fract(p);
}

fn hash21(p_in: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p_in.x, p_in.y, p_in.x) * 0.1031);
    p3 += dot(p3, p3.yzx + vec3<f32>(33.33));
    return fract((p3.x + p3.y) * p3.z);
}

fn vnoise1(x: f32) -> f32 {
    let i = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(hash11(i), hash11(i + 1.0), u);
}

fn vnoise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Identical ease-out curve to `crater_front_fraction` in `crater.rs`.
fn crater_front_fraction(u: f32) -> f32 {
    let c = clamp(u, 0.0, 1.0);
    return 1.0 - pow(max(1.0 - c, 0.0), 2.2);
}

// ── Kind 0: GroundScorch (drawn UNDER movers & buildings) ───────────────────

fn shade_ground_scorch(
    local_pos: vec2<f32>,
    bound_r: f32,
    inner_r: f32,
    age: f32,
    expand_dur: f32,
    lifetime: f32,
    seed: f32,
) -> vec4<f32> {
    let world_off = local_pos * bound_r;
    let dist = length(world_off);
    if dist > bound_r {
        discard;
    }

    let life_t = clamp(age / max(lifetime, 0.001), 0.0, 1.0);
    let hole_frac = select(
        1.0,
        crater_front_fraction(age / max(expand_dur, 0.001)),
        expand_dur > 0.001,
    );
    let hole_r = inner_r * hole_frac;
    if hole_r <= 0.1 {
        discard;
    }

    let ang = atan2(local_pos.y, local_pos.x);
    let seed_phase = seed * 6.2831853;
    let wobble = (sin(ang * 5.0 + seed_phase) * 0.55 + cos(ang * 11.0 - seed_phase * 1.7) * 0.35);
    let edge_r = hole_r + wobble * 0.85;
    let soft = max(1.1, inner_r * 0.08);
    let mask = 1.0 - smoothstep(edge_r - soft, edge_r + soft * 0.65, dist);
    if mask < 0.005 {
        discard;
    }

    let fade_out = 1.0 - smoothstep(0.45, 1.0, life_t);
    let r_norm = clamp(dist / max(inner_r, 1.0), 0.0, 1.0);

    // Charred charcoal core + crisp dark soot ring hugging the crater rim.
    let rim_d = (dist - hole_r * 0.92) / max(1.3, inner_r * 0.10);
    let soot_rim = exp(-rim_d * rim_d);
    let char_alpha = clamp((0.56 * (1.0 - r_norm * 0.42) + soot_rim * 0.28) * mask * fade_out, 0.0, 0.78);
    var col = vec3<f32>(0.045, 0.038, 0.036);

    // Cooling incandescent magma/ember fissures inside the crater floor.
    let cool = 1.0 - smoothstep(0.06, 0.68, life_t);
    if cool > 0.005 {
        let rot = mat2x2<f32>(0.8253, -0.5646, 0.5646, 0.8253);
        let p = rot * world_off * 0.36 + vec2<f32>(seed * 31.7, seed * 17.3);
        let n1 = vnoise2(p);
        let n2 = vnoise2(p * 2.15 + vec2<f32>(5.2, 1.3));
        let ridge = 1.0 - abs(2.0 * mix(n1, n2, 0.35) - 1.0);
        let r2 = ridge * ridge;
        let cracks = r2 * r2 * r2; // pow(ridge, 6)
        let heat_falloff = (1.0 - r_norm * 0.72) * mask;
        let ember_glow = cracks * heat_falloff * cool * 1.45;

        // Smooth cooling gradient: molten gold -> hot orange -> deep ember crimson.
        let ember_col = mix(
            vec3<f32>(0.72, 0.11, 0.02),
            vec3<f32>(1.65, 0.62, 0.10),
            clamp(cool * 1.15 - r_norm * 0.35, 0.0, 1.0),
        );
        col += ember_col * ember_glow;
    }

    let final_a = clamp(char_alpha, 0.0, 0.85);
    if final_a < 0.004 {
        discard;
    }
    return vec4<f32>(col, final_a);
}

// ── Kind 1: NukeAirBlast (flash + fireball + Mach ring + embers + smoke) ────

fn shade_nuke_air(
    local_pos: vec2<f32>,
    bound_r: f32,
    inner_r: f32,
    outer_r: f32,
    age: f32,
    expand_dur: f32,
    lifetime: f32,
    seed: f32,
) -> vec4<f32> {
    let world_off = local_pos * bound_r;
    let dist = length(world_off);
    if dist > bound_r {
        discard;
    }

    let life_t = clamp(age / max(lifetime, 0.001), 0.0, 1.0);
    let ang = atan2(local_pos.y, local_pos.x);
    let seed_phase = seed * 6.2831853;

    var rgb = vec3<f32>(0.0);
    var alpha = 0.0;

    // 1. Billowing dissipating smoke torus (composited underneath fire & embers).
    let smoke_start = 0.18;
    if age > smoke_start {
        let smoke_t = clamp((age - smoke_start) / max(lifetime - smoke_start, 0.001), 0.0, 1.0);
        let smoke_ease = 1.0 - (1.0 - smoke_t) * (1.0 - smoke_t);
        let smoke_r = inner_r * (0.58 + 0.56 * smoke_ease);
        let smoke_w = max(2.8, inner_r * (0.34 + 0.18 * smoke_t));
        let ring_d = abs(dist - smoke_r);
        if ring_d < smoke_w * 1.35 {
            let p_smoke = world_off * 0.22 + vec2<f32>(seed * 19.3, -age * 1.15);
            let puff = 0.55 + 0.45 * vnoise2(p_smoke);
            let band = 1.0 - smoothstep(0.0, smoke_w, ring_d);
            let fade_in = smoothstep(0.0, 0.16, smoke_t);
            let fade_out = 1.0 - smoothstep(0.28, 1.0, smoke_t);
            let sa = band * puff * fade_in * fade_out * 0.44;
            if sa > 0.003 {
                // Warm backlit underside early, cooling to dark ash smoke.
                let warm_lit = (1.0 - smoothstep(0.0, 0.45, smoke_t)) * 0.35;
                let s_col = mix(vec3<f32>(0.10, 0.095, 0.095), vec3<f32>(0.42, 0.18, 0.08), warm_lit);
                rgb = s_col;
                alpha = sa;
            }
        }
    }

    // 2. Boiling expanding fireball front synced 1:1 with the crater tile reveal.
    let eff_expand = max(expand_dur, 0.28);
    let fire_end = eff_expand + 0.46;
    if age < fire_end {
        let exp_t = clamp(age / eff_expand, 0.0, 1.0);
        let front_r = inner_r * crater_front_fraction(exp_t);
        let n_ang = vnoise1(ang * 7.0 + seed_phase + age * 4.5)
            + 0.5 * vnoise1(ang * 17.0 - seed_phase - age * 8.5);
        let perturbed_r = front_r + (n_ang - 0.75) * min(inner_r * 0.16, 2.3);
        let behind = perturbed_r - dist;
        let band_w = max(2.2, inner_r * 0.26);

        if behind > -band_w * 0.35 && behind < band_w * 1.15 {
            let fire_fade = 1.0 - smoothstep(eff_expand * 0.78, fire_end, age);
            let turb = vnoise2(world_off * 0.45 - vec2<f32>(age * 2.4, seed * 23.1));

            // Sharp incandescent leading plasma lip riding the exact crater tear line.
            let lip = (1.0 - smoothstep(0.0, band_w * 0.32, abs(behind))) * fire_fade;
            // Rolling fiery orange-red wake just behind the lip.
            let wake_pos = clamp(behind / band_w, 0.0, 1.0);
            let wake = smoothstep(-0.08, 0.12, wake_pos)
                * (1.0 - smoothstep(0.25, 1.0, wake_pos))
                * (0.65 + 0.55 * turb)
                * fire_fade;

            let fire_intensity = clamp(lip * 1.15 + wake * 0.85, 0.0, 1.0);
            if fire_intensity > 0.004 {
                let fire_col = mix(
                    mix(vec3<f32>(0.88, 0.14, 0.03), vec3<f32>(1.55, 0.56, 0.08), clamp(1.0 - wake_pos + turb * 0.25, 0.0, 1.0)),
                    vec3<f32>(2.1, 1.72, 0.85),
                    clamp(lip * 1.25, 0.0, 1.0),
                );
                let out_a = fire_intensity + alpha * (1.0 - fire_intensity);
                rgb = (fire_col * fire_intensity + rgb * alpha * (1.0 - fire_intensity)) / max(out_a, 0.0001);
                alpha = out_a;
            }
        }
    }

    // 3. Fast outer supersonic Mach shockwave (reaches outer_radius troop-damage envelope).
    let wave_dur = max(eff_expand + 0.45, 0.72);
    if age < wave_dur {
        let wt = clamp(age / wave_dur, 0.0, 1.0);
        let wave_r = outer_r * (1.0 - pow(max(1.0 - wt, 0.0), 2.6));
        let wave_w = max(1.35, outer_r * 0.055);
        let wd = abs(dist - wave_r);
        if wd < wave_w * 2.2 {
            let core_ring = 1.0 - smoothstep(0.0, wave_w * 0.65, wd);
            let halo = (1.0 - smoothstep(0.0, wave_w * 2.2, wd)) * 0.30;
            let wave_fade = (1.0 - wt) * (1.0 - wt * 0.45);
            let wa = clamp((core_ring * 0.82 + halo) * wave_fade, 0.0, 0.85);
            if wa > 0.004 {
                let w_col = mix(vec3<f32>(1.0, 0.86, 0.68), vec3<f32>(1.35, 1.32, 1.25), core_ring);
                let out_a = wa + alpha * (1.0 - wa);
                rgb = (w_col * wa + rgb * alpha * (1.0 - wa)) / max(out_a, 0.0001);
                alpha = out_a;
            }
        }
    }

    // 4. Outward-thrown chunky cooling embers (rotated world-space grid, OpenFront style).
    let ember_dur = 1.48;
    if age > 0.03 && age < ember_dur {
        let ember_max_r = inner_r * 1.42;
        if dist <= ember_max_r {
            let et = clamp(age / ember_dur, 0.0, 1.0);
            let ease = 1.0 - (1.0 - et) * (1.0 - et);
            let block = clamp(inner_r * 0.085, 0.75, 2.0);
            let rot = mat2x2<f32>(0.8253, -0.5646, 0.5646, 0.8253);
            // Slight outward advection makes sparks feel kinetically flung from ground zero.
            let radial_dir = world_off / max(dist, 0.001);
            let advected = world_off - radial_dir * (ease * inner_r * 0.22);
            let cid = floor((rot * advected) / block);
            let h1 = hash11(dot(cid, vec2<f32>(157.0, 113.0)) + seed * 97.3 + 41.7);
            if h1 < 0.20 {
                let h2 = hash11(h1 * 251.0 + 7.3);
                let cell_center = (cid + vec2<f32>(0.5)) * block;
                let dist_n = length(cell_center) / max(ember_max_r, 1.0);
                let reach = (0.14 + 0.86 * h2) * ease;
                if dist_n <= 1.0 && dist_n <= reach {
                    let ea = (1.0 - smoothstep(0.22, 1.0, et)) * (0.7 + 0.3 * hash11(h2 * 199.0));
                    if ea > 0.01 {
                        let heat = clamp(1.0 - et * 1.15 + h2 * 0.25, 0.0, 1.0);
                        let e_col = mix(
                            vec3<f32>(0.92, 0.16, 0.03),
                            mix(vec3<f32>(1.55, 0.65, 0.10), vec3<f32>(2.0, 1.85, 1.1), clamp(heat * 1.4 - 0.4, 0.0, 1.0)),
                            heat,
                        );
                        let out_a = ea + alpha * (1.0 - ea);
                        rgb = (e_col * ea + rgb * alpha * (1.0 - ea)) / max(out_a, 0.0001);
                        alpha = out_a;
                    }
                }
            }
        }
    }

    // 5. Blinding impact thermal flash at ground zero (fast punch, quick collapse).
    let flash_dur = 0.24;
    if age < flash_dur {
        let ft = clamp(age / flash_dur, 0.0, 1.0);
        let flash_r = inner_r * (0.35 + 0.92 * (1.0 - (1.0 - ft) * (1.0 - ft)));
        if dist < flash_r {
            let norm = dist / max(flash_r, 0.001);
            let core = exp(-norm * norm * 4.5);
            let flash_a = clamp(core * (1.0 - ft) * (1.0 - ft * 0.35) * 1.15, 0.0, 0.96);
            if flash_a > 0.004 {
                let f_col = mix(vec3<f32>(1.85, 1.25, 0.55), vec3<f32>(2.2, 2.1, 1.85), core);
                let out_a = flash_a + alpha * (1.0 - flash_a);
                rgb = (f_col * flash_a + rgb * alpha * (1.0 - flash_a)) / max(out_a, 0.0001);
                alpha = out_a;
            }
        }
    }

    alpha *= (1.0 - smoothstep(0.92, 1.0, life_t));
    if alpha < 0.004 {
        discard;
    }
    return vec4<f32>(rgb, clamp(alpha, 0.0, 1.0));
}

// ── Kind 2: SAM mid-air interception airburst ───────────────────────────────

fn shade_sam_airburst(
    local_pos: vec2<f32>,
    bound_r: f32,
    age: f32,
    lifetime: f32,
    seed: f32,
) -> vec4<f32> {
    let dist_n = length(local_pos);
    if dist_n > 1.0 {
        discard;
    }
    let t = clamp(age / max(lifetime, 0.001), 0.0, 1.0);
    let ease = 1.0 - (1.0 - t) * (1.0 - t);
    let ring_r = 0.14 + 0.80 * ease;
    let ring_w = 0.16 * (1.0 - t * 0.45);
    let ring = 1.0 - smoothstep(0.0, ring_w, abs(dist_n - ring_r));

    // Radial electric-cyan spark rays.
    let ang = atan2(local_pos.y, local_pos.x);
    let ray_base = 0.5 + 0.5 * sin(ang * 8.0 + seed * 6.2831853);
    let ray_2 = ray_base * ray_base;
    let ray_wave = ray_2 * ray_2 * ray_2;
    let sparks = ray_wave * (1.0 - smoothstep(0.0, ring_r + 0.15, dist_n)) * smoothstep(0.05, 0.35, dist_n) * (1.0 - t);

    // Bright intercept core flash.
    let core = exp(-dist_n * dist_n * 18.0) * (1.0 - smoothstep(0.0, 0.48, t));

    let fade = 1.0 - t;
    let intensity = clamp(ring * fade * 0.92 + sparks * 0.55 + core * 1.1, 0.0, 1.0);
    if intensity < 0.004 {
        discard;
    }
    let col = mix(
        vec3<f32>(0.55, 0.92, 1.50),
        vec3<f32>(1.85, 1.95, 2.0),
        clamp(core + ring * 0.45, 0.0, 1.0),
    );
    let _unused = bound_r;
    return vec4<f32>(col, intensity);
}

// ── Kind 3: Wave-synced building vaporization pop ───────────────────────────

fn shade_building_pop(
    local_pos: vec2<f32>,
    age: f32,
    lifetime: f32,
    seed: f32,
) -> vec4<f32> {
    let dist_n = length(local_pos);
    if dist_n > 1.0 {
        discard;
    }
    let t = clamp(age / max(lifetime, 0.001), 0.0, 1.0);
    let ease = 1.0 - (1.0 - t) * (1.0 - t);
    let ring_r = 0.12 + 0.82 * ease;
    let ring = 1.0 - smoothstep(0.0, 0.22, abs(dist_n - ring_r));
    let core = exp(-dist_n * dist_n * 12.0) * (1.0 - smoothstep(0.0, 0.55, t));

    let ang = atan2(local_pos.y, local_pos.x);
    let fleck_base = 0.5 + 0.5 * sin(ang * 6.0 + seed * 12.566);
    let fleck_2 = fleck_base * fleck_base;
    let flecks = fleck_2 * fleck_2 * fleck_base
        * (1.0 - smoothstep(0.1, 0.95, dist_n))
        * (1.0 - t);

    let alpha = clamp((ring * 0.78 + flecks * 0.45) * (1.0 - t) + core * 1.05, 0.0, 1.0);
    if alpha < 0.004 {
        discard;
    }
    let col = mix(
        vec3<f32>(1.45, 0.42, 0.06),
        vec3<f32>(2.0, 1.80, 0.95),
        clamp(core + ring * 0.35, 0.0, 1.0),
    );
    return vec4<f32>(col, alpha);
}

fn shade_blast(in: VertexOutput) -> vec4<f32> {
    let bound_r = in.radii_age.x;
    let inner_r = in.radii_age.y;
    let outer_r = in.radii_age.z;
    let age = in.radii_age.w;
    let expand_dur = in.params.x;
    let lifetime = in.params.y;
    let kind = in.params.z;
    let seed = in.params.w;

    if kind < 0.5 {
        return shade_ground_scorch(in.local_pos, bound_r, inner_r, age, expand_dur, lifetime, seed);
    } else if kind < 1.5 {
        return shade_nuke_air(in.local_pos, bound_r, inner_r, outer_r, age, expand_dur, lifetime, seed);
    } else if kind < 2.5 {
        return shade_sam_airburst(in.local_pos, bound_r, age, lifetime, seed);
    } else {
        return shade_building_pop(in.local_pos, age, lifetime, seed);
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = shade_blast(in);
    return vec4<f32>(clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), c.a);
}

// WebGL canvas is a plain UNORM surface (no hardware linear->sRGB encode), so
// encode here to match the native sRGB swapchain. Alpha stays linear.
@fragment
fn fs_main_srgb(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = shade_blast(in);
    let clamped = clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(pow(clamped, vec3<f32>(1.0 / 2.2)), c.a);
}
