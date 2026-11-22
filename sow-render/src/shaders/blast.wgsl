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
    if inst.kind == 5.0 && inst.seed < 1.5 {
        let angle = inst.seed * 6.2831853 + globals.time * 0.4;
        let axis = vec2<f32>(cos(angle),sin(angle));
        out.local_pos = vec2<f32>(dot(local,axis),dot(local,vec2<f32>(-axis.y,axis.x)));
    }
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
    seed_in: f32,
) -> vec4<f32> {
    let world_off = local_pos * bound_r;
    let dist = length(world_off);
    if dist > bound_r {
        discard;
    }

    let high_density = seed_in >= 1.5;
    let seed = fract(seed_in);
    let screen_px = bound_r * globals.zoom;
    let fast_lod = high_density || screen_px < 26.0;

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
        var ember_glow = 0.0;
        if fast_lod {
            let rim_heat = exp(-rim_d * rim_d * 1.6);
            ember_glow = ((1.0 - r_norm * 0.82) * 0.32 + rim_heat * 0.48) * mask * cool;
        } else {
            let rot = mat2x2<f32>(0.8253, -0.5646, 0.5646, 0.8253);
            let p = rot * world_off * 0.36 + vec2<f32>(seed * 31.7, seed * 17.3);
            let n1 = vnoise2(p);
            let n2 = vnoise2(p * 2.15 + vec2<f32>(5.2, 1.3));
            let ridge = 1.0 - abs(2.0 * mix(n1, n2, 0.35) - 1.0);
            let r2 = ridge * ridge;
            let cracks = r2 * r2 * r2; // pow(ridge, 6)
            let heat_falloff = (1.0 - r_norm * 0.72) * mask;
            ember_glow = cracks * heat_falloff * cool * 1.45;
        }

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
    seed_in: f32,
) -> vec4<f32> {
    let world_off = local_pos * bound_r;
    let dist = length(world_off);
    if dist > bound_r {
        discard;
    }

    let high_density = seed_in >= 1.5;
    let seed = fract(seed_in);
    let screen_px = bound_r * globals.zoom;
    let fast_lod = high_density || screen_px < 28.0;
    let is_thermonuclear = inner_r >= 50.0;

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
            var puff = 0.78;
            if !fast_lod {
                let p_smoke = world_off * 0.22 + vec2<f32>(seed * 19.3, -age * 1.15);
                puff = 0.55 + 0.45 * vnoise2(p_smoke);
            }
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
        var n_ang = 0.75 + 0.35 * sin(ang * 7.0 + seed_phase + age * 4.5);
        if !fast_lod {
            n_ang = vnoise1(ang * 7.0 + seed_phase + age * 4.5)
                + 0.5 * vnoise1(ang * 17.0 - seed_phase - age * 8.5);
        }
        let perturbed_r = front_r + (n_ang - 0.75) * min(inner_r * 0.16, 2.3);
        let behind = perturbed_r - dist;
        let band_w = max(2.2, inner_r * 0.26);

        if behind > -band_w * 0.35 && behind < band_w * 1.15 {
            let fire_fade = 1.0 - smoothstep(eff_expand * 0.78, fire_end, age);
            var turb = 0.55;
            if !fast_lod {
                turb = vnoise2(world_off * 0.45 - vec2<f32>(age * 2.4, seed * 23.1));
            }

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
                let hot_lip_col = select(
                    vec3<f32>(2.1, 1.72, 0.85),
                    vec3<f32>(2.25, 1.92, 1.25),
                    is_thermonuclear,
                );
                let fire_col = mix(
                    mix(vec3<f32>(0.88, 0.14, 0.03), vec3<f32>(1.55, 0.56, 0.08), clamp(1.0 - wake_pos + turb * 0.25, 0.0, 1.0)),
                    hot_lip_col,
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

        // Secondary Wilson vapor condensation cloud ring for Hydrogen Bombs.
        if is_thermonuclear && wt < 0.68 {
            let wilson_t = wt / 0.68;
            let wilson_r = inner_r * (0.42 + 0.74 * wilson_t);
            let wilson_w = max(3.2, inner_r * 0.14);
            let wilson_d = abs(dist - wilson_r);
            if wilson_d < wilson_w {
                let w_band = 1.0 - smoothstep(0.0, wilson_w, wilson_d);
                let w_env = sin(wilson_t * 3.14159265);
                let wilson_a = w_band * w_env * 0.38;
                if wilson_a > 0.004 {
                    let vapor_col = vec3<f32>(1.18, 1.22, 1.30);
                    let out_a = wilson_a + alpha * (1.0 - wilson_a);
                    rgb = (vapor_col * wilson_a + rgb * alpha * (1.0 - wilson_a)) / max(out_a, 0.0001);
                    alpha = out_a;
                }
            }
        }
    }

    // 4. Outward-thrown chunky cooling embers (skipped under fast_lod to save ALU on dense salvos).
    let ember_dur = 1.48;
    if !fast_lod && age > 0.03 && age < ember_dur {
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
    let flash_dur = select(0.24, 0.34, is_thermonuclear);
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
    seed_in: f32,
) -> vec4<f32> {
    let dist_n = length(local_pos);
    if dist_n > 1.0 {
        discard;
    }
    let seed = fract(seed_in);
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

// ── Kind 3: Wave-synced building demolition & structural shatter pop ────────
//
// `expand_dur` = structure tier (0.0 = standard, 1.0 = industrial/military, 2.0 = capital/nuclear)
// `seed_in`    = fract: phase seed, >= 1.5: high-density / reduced-motion fast LOD

fn shade_building_pop(
    local_pos: vec2<f32>,
    bound_r: f32,
    tier: f32,
    age: f32,
    lifetime: f32,
    seed_in: f32,
) -> vec4<f32> {
    let dist_n = length(local_pos);
    if dist_n > 1.0 {
        discard;
    }
    let fast_lod = (seed_in >= 1.5) || (bound_r * globals.zoom < 18.0);
    let seed = fract(seed_in);
    let t = clamp(age / max(lifetime, 0.001), 0.0, 1.0);
    let ease = 1.0 - (1.0 - t) * (1.0 - t);
    let ease3 = 1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t);
    let fade = 1.0 - t;

    // 1. Billowing concrete/ash demolition dust ring (provides high-contrast dark halo on bright land).
    let dust_r = 0.16 + 0.64 * ease;
    let dust_w = 0.25 * (1.0 + 0.30 * t);
    let dust_band = 1.0 - smoothstep(0.0, dust_w, abs(dist_n - dust_r));
    let dust_a = dust_band * smoothstep(0.0, 0.14, t) * (1.0 - smoothstep(0.32, 1.0, t)) * 0.54;
    let dust_warm = (1.0 - smoothstep(0.0, 0.48, t)) * 0.45;
    let dust_col = mix(vec3<f32>(0.085, 0.08, 0.078), vec3<f32>(0.52, 0.20, 0.06), dust_warm);

    // 2. Primary outward demolition shockwave ring + strategic secondary collapse ring.
    let ring_r = 0.14 + 0.82 * ease3;
    let ring_w = 0.16 * (1.0 - t * 0.42);
    let ring = 1.0 - smoothstep(0.0, ring_w, abs(dist_n - ring_r));
    var inner_ring = 0.0;
    if tier > 0.5 {
        let inner_r = 0.10 + 0.54 * ease;
        inner_ring = (1.0 - smoothstep(0.0, ring_w * 0.9, abs(dist_n - inner_r))) * 0.72;
    }

    // 3. Radial ballistic structural shards & incandescent sparks.
    let ang = atan2(local_pos.y, local_pos.x);
    let s1 = 0.5 + 0.5 * sin(ang * 8.0 + seed * 6.2831853);
    let s2 = 0.5 + 0.5 * cos(ang * 13.0 - seed * 9.424778);
    let s1_2 = s1 * s1;
    var shards = s1_2 * s1_2 * (0.45 + 0.55 * s2 * s2)
        * smoothstep(0.06, 0.22, dist_n)
        * (1.0 - smoothstep(0.15, ring_r + 0.08, dist_n))
        * (1.0 - smoothstep(0.0, 0.84, t));
    if !fast_lod && shards > 0.01 {
        let chunk = vnoise1(dist_n * 9.5 - ease3 * 4.8 + seed * 19.7);
        shards *= smoothstep(0.24, 0.68, chunk) * 1.35;
    }

    // 4. Demolition core flash transitioning to a cooling smoldering ember bed.
    let flash_core = exp(-dist_n * dist_n * 14.0) * (1.0 - smoothstep(0.0, 0.44, t));
    let ember_bed = exp(-dist_n * dist_n * 6.5) * (1.0 - smoothstep(0.20, 0.95, t)) * 0.48;
    let core = flash_core + ember_bed;

    let hot_a = clamp(
        (ring * 0.86 + inner_ring + shards * 0.78) * fade + core * 1.12,
        0.0,
        1.0,
    );
    let total_a = clamp(hot_a + dust_a * (1.0 - hot_a), 0.0, 0.98);
    if total_a < 0.004 {
        discard;
    }
    let hot_base = select(
        vec3<f32>(1.52, 0.44, 0.07),
        vec3<f32>(1.72, 0.58, 0.12),
        tier > 1.5,
    );
    let hot_peak = select(
        vec3<f32>(2.05, 1.82, 0.96),
        vec3<f32>(2.20, 2.02, 1.32),
        tier > 1.5,
    );
    let hot_col = mix(
        hot_base,
        hot_peak,
        clamp(flash_core + ring * 0.42 + inner_ring * 0.35, 0.0, 1.0),
    );
    let rgb = (hot_col * hot_a + dust_col * dust_a * (1.0 - hot_a)) / max(total_a, 0.0001);
    return vec4<f32>(rgb, total_a);
}

// ── Kind 4: In-Flight Nuke Destination Telegraph (drawn in ground pass) ─────
//
// `age`        = flight progress (0.0 .. 1.0)
// `expand_dur` = diplomatic relation (0.0 = Self, 1.0 = Ally/Teammate, 2.0 = Enemy)
// `lifetime`   = floor: nuke level (1 = Atom, 2 = H-Bomb, 3 = MIRV Carrier, 4 = MIRV Warhead)
//                fract * 10.0: coalesced salvo stack boost (0.0 .. 0.8)
// `seed_in`    = fract: phase seed, >= 1.5: reduced_motion

fn shade_nuke_telegraph(
    local_pos: vec2<f32>,
    bound_r: f32,
    inner_r: f32,
    outer_r: f32,
    progress_in: f32,
    relation: f32,
    level_encoded: f32,
    seed_in: f32,
) -> vec4<f32> {
    let world_off = local_pos * bound_r;
    let dist = length(world_off);
    let px_world = 1.0 / max(globals.zoom, 0.01);
    let aa = max(0.35, 0.85 * px_world);
    let stroke_w = max(1.25 * px_world, min(outer_r * 0.022, 2.35));
    let halo_w = stroke_w + 1.15 * px_world;

    if dist > outer_r + halo_w + aa {
        discard;
    }

    let progress = clamp(progress_in, 0.0, 1.0);
    let level = floor(level_encoded + 0.01);
    let stack_boost = clamp(fract(level_encoded) * 10.0, 0.0, 0.8);
    let reduced_motion = seed_in >= 1.5;
    let seed = fract(seed_in);

    // Converging time-to-impact (ETA) ring sweeps from outer_r inward toward ground zero.
    let eta_r = mix(outer_r, inner_r * 0.22, progress);
    let eta_half_w = stroke_w * 0.72;
    let in_inner = dist <= inner_r + halo_w + aa;
    let in_outer = abs(dist - outer_r) <= halo_w + aa;
    let in_eta = !reduced_motion && abs(dist - eta_r) <= eta_half_w + aa;
    let mid_r = mix(inner_r, outer_r, 0.52);
    let in_mirv_mid = abs(level - 3.0) < 0.5 && abs(dist - mid_r) <= stroke_w + aa;

    // Fast early discard in the empty annulus (>75% of octagon fragments exit here in 0 trig ops).
    if !in_inner && !in_outer && !in_eta && !in_mirv_mid {
        discard;
    }

    // Diplomatic palette: Self = Emerald, Ally = Amber, Enemy = Crimson.
    var base_col = vec3<f32>(1.0, 0.16, 0.14);
    var accent_col = vec3<f32>(1.0, 0.62, 0.45);
    if relation < 0.5 {
        base_col = vec3<f32>(0.08, 0.94, 0.42);
        accent_col = vec3<f32>(0.55, 1.0, 0.76);
    } else if relation < 1.5 {
        base_col = vec3<f32>(1.0, 0.78, 0.10);
        accent_col = vec3<f32>(1.0, 0.95, 0.56);
    }

    let urgency = clamp((progress - 0.55) / 0.45, 0.0, 1.0);
    let pulse_freq = 3.2 + urgency * 7.5;
    let pulse = select(
        0.88 + 0.12 * sin(globals.time * pulse_freq + seed * 6.2831853),
        0.90,
        reduced_motion,
    );
    let base_alpha = clamp((0.82 + stack_boost * 0.18) * pulse, 0.0, 0.98);

    var rgb = base_col;
    var alpha = 0.0;
    var halo_alpha = 0.0;

    // 1. Inner lethal wipe zone: radial threat fill + solid high-contrast rim + bullseye pip.
    if in_inner {
        let inner_fill_mask = 1.0 - smoothstep(inner_r - aa, inner_r + aa, dist);
        let r_norm = clamp(dist / max(inner_r, 0.5), 0.0, 1.0);
        let fill_ramp = 0.10 + 0.16 * (r_norm * r_norm) + stack_boost * 0.08 + urgency * 0.05;
        alpha = max(alpha, inner_fill_mask * fill_ramp * pulse);

        let inner_d = abs(dist - inner_r);
        let inner_stroke = 1.0 - smoothstep(stroke_w * 0.75 - aa, stroke_w * 0.75 + aa, inner_d);
        let inner_halo = 1.0 - smoothstep(halo_w * 0.85 - aa, halo_w * 0.85 + aa, inner_d);
        halo_alpha = max(halo_alpha, inner_halo * 0.48 * pulse);
        if inner_stroke > 0.005 {
            rgb = mix(base_col, accent_col, inner_stroke * (0.35 + urgency * 0.35));
            alpha = max(alpha, inner_stroke * base_alpha);
        }

        // Ground-zero bullseye crosshair when zoomed in enough to read cleanly.
        if inner_r * globals.zoom > 16.0 {
            let pip_r = max(1.3, 2.8 * px_world);
            if dist <= pip_r * 2.4 {
                let center_dot = 1.0 - smoothstep(pip_r * 0.75 - aa, pip_r * 0.75 + aa, dist);
                let cross_arm = min(abs(world_off.x), abs(world_off.y));
                let cross_mask = (1.0 - smoothstep(0.65 * px_world, 1.35 * px_world, cross_arm))
                    * smoothstep(pip_r * 1.05, pip_r * 1.35, dist)
                    * (1.0 - smoothstep(pip_r * 2.1, pip_r * 2.4, dist));
                let reticle_core = max(center_dot, cross_mask * 0.85);
                if reticle_core > 0.01 {
                    rgb = mix(rgb, accent_col, reticle_core * 0.65);
                    alpha = max(alpha, reticle_core * base_alpha);
                }
            }
        }
    }

    // 2. Converging ETA pulse ring (gives instant visual reading of remaining flight time).
    if in_eta {
        let eta_d = abs(dist - eta_r);
        let eta_ring = 1.0 - smoothstep(eta_half_w - aa, eta_half_w + aa, eta_d);
        let eta_strength = (0.28 + 0.42 * progress) * eta_ring;
        if eta_strength > 0.005 {
            rgb = mix(rgb, accent_col, eta_ring * 0.55);
            alpha = max(alpha, eta_strength);
        }
    }

    // 3. MIRV Carrier intermediate cluster saturation ring (level == 3).
    if in_mirv_mid {
        let ang_mid = atan2(local_pos.y, local_pos.x);
        let rot_mid = select(globals.time * -0.35, 0.0, reduced_motion);
        let phase_mid = fract(ang_mid / 6.2831853 * 12.0 + rot_mid);
        let dash_mid = 1.0 - smoothstep(0.48, 0.56, phase_mid);
        let mid_stroke = 1.0 - smoothstep(stroke_w * 0.65 - aa, stroke_w * 0.65 + aa, abs(dist - mid_r));
        let mid_a = mid_stroke * dash_mid * base_alpha * 0.78;
        if mid_a > 0.005 {
            rgb = mix(rgb, accent_col, 0.4);
            alpha = max(alpha, mid_a);
        }
    }

    // 4. Outer fallout / troop-damage dashed perimeter ring (atan2 strictly guarded here).
    if in_outer {
        let ang = atan2(local_pos.y, local_pos.x);
        // Seamless integer dash count around 2*PI so there is never a seam at +-PI.
        var num_dashes = 24.0;
        if level > 3.5 {
            num_dashes = 16.0;
        } else if level > 2.5 {
            num_dashes = 32.0;
        } else if level > 1.5 {
            num_dashes = 48.0;
        }
        let rot_speed = select(0.45, 0.22, level > 1.5 && level < 3.5);
        let rot = select(globals.time * rot_speed, 0.0, reduced_motion);
        let cyc = fract(ang / 6.2831853 * num_dashes + rot);
        let dash_aa = clamp(0.04, 0.015, 0.12);
        let dash_mask = smoothstep(0.0, dash_aa, cyc) * (1.0 - smoothstep(0.62 - dash_aa, 0.62 + dash_aa, cyc));

        let outer_d = abs(dist - outer_r);
        let outer_stroke = 1.0 - smoothstep(stroke_w - aa, stroke_w + aa, outer_d);
        let outer_halo = 1.0 - smoothstep(halo_w - aa, halo_w + aa, outer_d);
        halo_alpha = max(halo_alpha, outer_halo * dash_mask * 0.52 * pulse);

        let outer_a = outer_stroke * dash_mask * base_alpha;
        if outer_a > 0.005 {
            // Super-weapons (H-Bomb & MIRV Carrier) get hot accent tips on major cardinal sectors.
            let is_super = level > 1.5 && level < 3.5;
            let cardinal = select(0.0, pow(0.5 + 0.5 * cos(ang * 4.0), 6.0), is_super);
            rgb = mix(base_col, accent_col, cardinal * 0.65 + outer_stroke * 0.20);
            alpha = max(alpha, outer_a);
        }
    }

    // Composite dark contrast halo behind bright strokes so indicators stay crisp on any territory color.
    let total_a = clamp(max(alpha, halo_alpha), 0.0, 0.96);
    if total_a < 0.006 {
        discard;
    }
    let dark_halo_col = vec3<f32>(0.025, 0.03, 0.045);
    let stroke_blend = clamp(alpha / max(total_a, 0.0001), 0.0, 1.0);
    let final_rgb = mix(dark_halo_col, rgb, stroke_blend);
    return vec4<f32>(final_rgb, total_a);
}

// ── Kind 5: Live Nuclear Aiming Reticle & Source Silo Lock (ground pass) ────
//
// `expand_dur` = mode (0.0 = Valid Target Lock, 1.0 = Standby/Invalid Preview, 2.0 = Source Silo Lock)
// `lifetime`   = nuke level (1.0 = Atom, 2.0 = H-Bomb, 3.0 = MIRV)
// `seed_in`    = >= 1.5: reduced_motion

fn shade_nuke_reticle(
    local_pos: vec2<f32>,
    bound_r: f32,
    inner_r: f32,
    outer_r: f32,
    mode: f32,
    level: f32,
    seed_in: f32,
) -> vec4<f32> {
    let reduced_motion = seed_in >= 1.5;
    let world_off = local_pos * bound_r;
    let dist = length(world_off);
    let px_world = 1.0 / max(globals.zoom, 0.01);
    let aa = max(0.35, 0.85 * px_world);
    // Mode 2.0: Source Silo Launch Lock Beacon.
    if mode > 1.5 {
        let stroke_w = max(1.4 * px_world, outer_r * 0.14);
        if dist > outer_r + stroke_w + aa {
            discard;
        }
        let ang = atan2(local_pos.y, local_pos.x);
        let rot = select(globals.time * 0.65, 0.125, reduced_motion);
        let bracket = fract(ang / 6.2831853 * 4.0 + rot);
        let arc_mask = smoothstep(0.04, 0.09, bracket) * (1.0 - smoothstep(0.66, 0.71, bracket));
        let ring = 1.0 - smoothstep(stroke_w - aa, stroke_w + aa, abs(dist - outer_r));
        let inner_pip = 1.0 - smoothstep(stroke_w * 1.1 - aa, stroke_w * 1.1 + aa, dist);
        let a = max(ring * arc_mask * 0.92, inner_pip * 0.75);
        if a < 0.006 {
            discard;
        }
        let col = mix(vec3<f32>(1.0, 0.74, 0.16), vec3<f32>(1.0, 0.95, 0.58), ring);
        return vec4<f32>(select(col,vec3<f32>(1.0,0.17,0.08),mode>2.5), a);
    }

    let stroke_w = max(1.35 * px_world, min(outer_r * 0.024, 2.5));
    let halo_w = stroke_w + 1.2 * px_world;
    if dist > outer_r + halo_w + aa {
        discard;
    }

    let valid = mode < 0.5;
    let is_mirv = level > 2.5;
    let mid_r = mix(inner_r, outer_r, 0.52);

    let in_inner = dist <= inner_r + halo_w + aa;
    let in_outer = abs(dist - outer_r) <= halo_w + aa;
    let in_mid = is_mirv && abs(dist - mid_r) <= stroke_w + aa;
    let on_axis = min(abs(world_off.x), abs(world_off.y)) <= stroke_w * 0.9 + aa;

    if !in_inner && !in_outer && !in_mid && !on_axis {
        discard;
    }

    let primary_col = select(
        vec3<f32>(0.72, 0.78, 0.86),
        vec3<f32>(1.0, 0.34, 0.14),
        valid,
    );
    let accent_col = select(
        vec3<f32>(0.90, 0.94, 1.0),
        vec3<f32>(1.0, 0.84, 0.28),
        valid,
    );
    let alpha_scale = select(0.52, 0.92, valid);
    let breath = select(0.92 + 0.08 * sin(globals.time * 4.5), 0.95, reduced_motion);

    var rgb = primary_col;
    var alpha = 0.0;
    var halo_alpha = 0.0;

    // 1. Inner destruction zone + solid rim.
    if in_inner {
        let fill_mask = 1.0 - smoothstep(inner_r - aa, inner_r + aa, dist);
        let r_norm = clamp(dist / max(inner_r, 0.5), 0.0, 1.0);
        let fill_a = fill_mask * (0.07 + 0.13 * (r_norm * r_norm)) * alpha_scale * breath;
        alpha = max(alpha, fill_a);

        let inner_d = abs(dist - inner_r);
        let inner_ring = 1.0 - smoothstep(stroke_w * 0.8 - aa, stroke_w * 0.8 + aa, inner_d);
        let inner_halo = 1.0 - smoothstep(halo_w * 0.85 - aa, halo_w * 0.85 + aa, inner_d);
        halo_alpha = max(halo_alpha, inner_halo * 0.45 * alpha_scale);
        if inner_ring > 0.005 {
            rgb = mix(primary_col, accent_col, inner_ring * 0.5);
            alpha = max(alpha, inner_ring * alpha_scale * breath);
        }
    }

    // 2. Tactical crosshair ticks spanning the inner rim and center bullseye.
    if on_axis {
        let axis_d = min(abs(world_off.x), abs(world_off.y));
        let line_mask = 1.0 - smoothstep(stroke_w * 0.65 - aa, stroke_w * 0.65 + aa, axis_d);
        let center_tick = smoothstep(1.2 * px_world, 2.2 * px_world, dist)
            * (1.0 - smoothstep(max(3.5, 8.0 * px_world), max(4.2, 9.5 * px_world), dist));
        let rim_tick = smoothstep(inner_r - stroke_w * 2.6, inner_r - stroke_w * 1.8, dist)
            * (1.0 - smoothstep(inner_r + stroke_w * 1.8, inner_r + stroke_w * 2.6, dist));
        let tick_a = line_mask * max(center_tick, rim_tick) * alpha_scale * breath;
        if tick_a > 0.005 {
            rgb = accent_col;
            alpha = max(alpha, tick_a);
        }
    }

    // 3. MIRV intermediate saturation ring.
    if in_mid {
        let ang_m = atan2(local_pos.y, local_pos.x);
        let rot_m = select(globals.time * -0.35, 0.0, reduced_motion);
        let dash_m = 1.0 - smoothstep(0.45, 0.55, fract(ang_m / 6.2831853 * 16.0 + rot_m));
        let ring_m = 1.0 - smoothstep(stroke_w * 0.65 - aa, stroke_w * 0.65 + aa, abs(dist - mid_r));
        let mid_a = ring_m * dash_m * alpha_scale * 0.75;
        if mid_a > 0.005 {
            rgb = mix(primary_col, accent_col, 0.45);
            alpha = max(alpha, mid_a);
        }
    }

    // 4. Outer damage perimeter dashed ring / MIRV cardinal brackets.
    if in_outer {
        let ang = atan2(local_pos.y, local_pos.x);
        let num_dashes = select(select(24.0, 48.0, level > 1.5), 12.0, is_mirv);
        let rot = select(globals.time * 0.35, 0.0, reduced_motion);
        let cyc = fract(ang / 6.2831853 * num_dashes + rot);
        let dash_mask = smoothstep(0.0, 0.04, cyc) * (1.0 - smoothstep(0.62, 0.66, cyc));
        let outer_d = abs(dist - outer_r);
        let outer_ring = 1.0 - smoothstep(stroke_w - aa, stroke_w + aa, outer_d);
        let outer_halo = 1.0 - smoothstep(halo_w - aa, halo_w + aa, outer_d);
        halo_alpha = max(halo_alpha, outer_halo * dash_mask * 0.5 * alpha_scale);
        let outer_a = outer_ring * dash_mask * alpha_scale * breath;
        if outer_a > 0.005 {
            rgb = mix(primary_col, accent_col, 0.35);
            alpha = max(alpha, outer_a);
        }
    }

    let total_a = clamp(max(alpha, halo_alpha), 0.0, 0.95);
    if total_a < 0.006 {
        discard;
    }
    let dark_halo_col = vec3<f32>(0.025, 0.03, 0.045);
    let stroke_blend = clamp(alpha / max(total_a, 0.0001), 0.0, 1.0);
    return vec4<f32>(mix(dark_halo_col, rgb, stroke_blend), total_a);
}

// ── Kind 6: Silo Launch Ignition & Exhaust Plume (air pass) ─────────────────

fn shade_nuke_launch(
    local_pos: vec2<f32>,
    age: f32,
    lifetime: f32,
    seed_in: f32,
) -> vec4<f32> {
    let dist_n = length(local_pos);
    if dist_n > 1.0 {
        discard;
    }
    let seed = fract(seed_in);
    let t = clamp(age / max(lifetime, 0.001), 0.0, 1.0);
    let ease = 1.0 - (1.0 - t) * (1.0 - t);

    // Expanding ground exhaust dust ring.
    let dust_r = 0.18 + 0.76 * ease;
    let dust_w = 0.24 * (1.0 - t * 0.3);
    let dust_band = 1.0 - smoothstep(0.0, dust_w, abs(dist_n - dust_r));
    let dust_a = dust_band * (1.0 - t) * 0.55;

    // Radial ignition exhaust streaks.
    let ang = atan2(local_pos.y, local_pos.x);
    let spoke_wave = pow(0.5 + 0.5 * sin(ang * 8.0 + seed * 6.2831853), 5.0);
    let sparks = spoke_wave
        * (1.0 - smoothstep(0.08, dust_r + 0.12, dist_n))
        * smoothstep(0.04, 0.24, dist_n)
        * (1.0 - smoothstep(0.0, 0.72, t));

    // White-gold rocket motor ignition core flash.
    let core = exp(-dist_n * dist_n * 14.0) * (1.0 - smoothstep(0.0, 0.52, t));

    let hot_a = clamp(sparks * 0.78 + core * 1.15, 0.0, 1.0);
    let total_a = clamp(hot_a + dust_a * (1.0 - hot_a), 0.0, 0.96);
    if total_a < 0.004 {
        discard;
    }
    let dust_col = mix(vec3<f32>(0.22, 0.20, 0.18), vec3<f32>(0.75, 0.36, 0.12), 1.0 - t);
    let hot_col = mix(vec3<f32>(1.65, 0.58, 0.10), vec3<f32>(2.15, 1.95, 1.25), clamp(core, 0.0, 1.0));
    let rgb = (hot_col * hot_a + dust_col * dust_a * (1.0 - hot_a)) / max(total_a, 0.0001);
    return vec4<f32>(rgb, total_a);
}

// ── Kind 7: Mid-Air MIRV Carrier Separation Burst (air pass) ────────────────

fn shade_mirv_split(
    local_pos: vec2<f32>,
    age: f32,
    lifetime: f32,
    seed_in: f32,
) -> vec4<f32> {
    let dist_n = length(local_pos);
    if dist_n > 1.0 {
        discard;
    }
    let seed = fract(seed_in);
    let t = clamp(age / max(lifetime, 0.001), 0.0, 1.0);
    let ease = 1.0 - (1.0 - t) * (1.0 - t);

    // Outer high-altitude staging shock ring + inner bus separation ring.
    let outer_ring_r = 0.16 + 0.80 * ease;
    let inner_ring_r = 0.10 + 0.52 * ease;
    let ring_w = 0.13 * (1.0 - t * 0.45);
    let outer_ring = 1.0 - smoothstep(0.0, ring_w, abs(dist_n - outer_ring_r));
    let inner_ring = 1.0 - smoothstep(0.0, ring_w * 1.15, abs(dist_n - inner_ring_r));

    // 12-spoke cluster warhead dispersion flare.
    let ang = atan2(local_pos.y, local_pos.x);
    let spoke = pow(0.5 + 0.5 * cos(ang * 12.0 + seed * 6.2831853), 6.0);
    let rays = spoke
        * smoothstep(0.06, 0.25, dist_n)
        * (1.0 - smoothstep(0.2, outer_ring_r + 0.08, dist_n))
        * (1.0 - t);

    let core = exp(-dist_n * dist_n * 16.0) * (1.0 - smoothstep(0.0, 0.45, t));
    let fade = 1.0 - t;
    let alpha = clamp((outer_ring * 0.88 + inner_ring * 0.65) * fade + rays * 0.65 + core * 1.15, 0.0, 0.98);
    if alpha < 0.004 {
        discard;
    }

    let col = mix(
        vec3<f32>(1.55, 0.68, 0.14),
        vec3<f32>(2.1, 1.98, 1.45),
        clamp(core + outer_ring * 0.55, 0.0, 1.0),
    );
    return vec4<f32>(col, alpha);
}

// ── Kind 10: Sovereign Entity Elimination & Naval Sinking Burst (air pass) ──
//
// `inner_r`    = packed 18-bit entity RGB color ((r6 << 12) | (g6 << 6) | b6)
// `expand_dur` = mode (0.0 = Tribe/Bot, 1.0 = Sovereign Player/Empire, 2.0 = Nuclear Kill, 3.0 = Naval Sunk)
// `seed_in`    = fract: phase seed, >= 1.5: high-density / reduced-motion fast LOD

fn unpack_rgb_f32(packed_in: f32) -> vec3<f32> {
    let p = clamp(floor(packed_in + 0.5), 0.0, 262143.0);
    let r = floor(p / 4096.0);
    let rem = p - r * 4096.0;
    let g = floor(rem / 64.0);
    let b = rem - g * 64.0;
    return clamp(vec3<f32>(r, g, b) / 63.0, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn shade_entity_death(
    local_pos: vec2<f32>,
    bound_r: f32,
    packed_rgb: f32,
    mode: f32,
    age: f32,
    lifetime: f32,
    seed_in: f32,
) -> vec4<f32> {
    let dist_n = length(local_pos);
    if dist_n > 1.0 {
        discard;
    }
    let fast_lod = (seed_in >= 1.5) || (bound_r * globals.zoom < 20.0);
    let seed = fract(seed_in);
    let t = clamp(age / max(lifetime, 0.001), 0.0, 1.0);
    let ease = 1.0 - (1.0 - t) * (1.0 - t);
    let ease3 = 1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t);
    let fade = 1.0 - t;

    let raw_faction = unpack_rgb_f32(packed_rgb);
    let faction_col = max(raw_faction, vec3<f32>(0.22)) * 1.25;
    let is_naval = mode > 2.5;
    let is_nuke = mode > 1.5 && mode < 2.5;
    let is_sovereign = mode > 0.5;

    var accent_col = vec3<f32>(1.88, 1.32, 0.42);
    if is_naval {
        accent_col = vec3<f32>(1.40, 1.85, 2.15);
    } else if is_nuke {
        accent_col = vec3<f32>(2.15, 1.72, 0.36);
    } else if is_sovereign {
        accent_col = vec3<f32>(2.10, 1.58, 0.48);
    }

    // 1. Primary expanding sovereign shockwave ring + dark contrast halo.
    let ring_r = 0.14 + 0.72 * ease3;
    let ring_w = 0.13 * (1.0 - t * 0.42);
    let ring_d = abs(dist_n - ring_r);
    let outer_ring = 1.0 - smoothstep(0.0, ring_w, ring_d);
    let halo_ring = (1.0 - smoothstep(0.0, ring_w * 2.0, ring_d)) * 0.52 * fade;

    // 2. Segmented sovereign crown / naval ripple inner fracture ring.
    let crown_r = 0.08 + 0.52 * ease;
    let crown_d = abs(dist_n - crown_r);
    var crown_ring = 0.0;
    var rays = 0.0;
    if crown_d < ring_w * 1.35 || (!fast_lod && dist_n < ring_r + 0.08) {
        let ang = atan2(local_pos.y, local_pos.x);
        let segs = select(8.0, 12.0, is_sovereign && !is_naval);
        let cyc = fract(ang / 6.2831853 * segs + seed);
        let dash = smoothstep(0.05, 0.15, cyc) * (1.0 - smoothstep(0.62, 0.72, cyc));
        crown_ring = (1.0 - smoothstep(0.0, ring_w * 1.1, crown_d)) * dash * 0.82;

        // 3. Radial emblem shatter rays & flying crystalline/ember shards.
        let spoke_base = 0.5 + 0.5 * cos(ang * segs + seed * 6.2831853);
        let spoke_2 = spoke_base * spoke_base;
        rays = spoke_2 * spoke_2 * spoke_base
            * smoothstep(0.06, 0.22, dist_n)
            * (1.0 - smoothstep(0.18, ring_r + 0.06, dist_n))
            * (1.0 - smoothstep(0.0, 0.78, t));
        if !fast_lod && rays > 0.01 {
            let shard_band = vnoise1(dist_n * 10.5 - ease3 * 5.2 + seed * 23.1);
            rays *= smoothstep(0.22, 0.65, shard_band) * 1.4;
        }
    }

    // 4. Fast implosion-to-explosion core flash + tactical elimination cross scar.
    let core_flash = exp(-dist_n * dist_n * 15.0) * (1.0 - smoothstep(0.0, 0.46, t));
    let cross_arm = min(abs(local_pos.x), abs(local_pos.y));
    let cross_flare = (1.0 - smoothstep(0.0, 0.045, cross_arm))
        * (1.0 - smoothstep(0.05, 0.58 * (1.0 - 0.35 * t), dist_n))
        * (1.0 - smoothstep(0.0, 0.54, t))
        * 0.85;

    let hot_a = clamp(
        (outer_ring * 0.90 + crown_ring + rays * 0.72) * fade + core_flash * 1.15 + cross_flare,
        0.0,
        1.0,
    );
    let total_a = clamp(max(hot_a, halo_ring), 0.0, 0.96);
    if total_a < 0.004 {
        discard;
    }

    let hot_mix = clamp(core_flash + cross_flare * 0.75 + outer_ring * 0.38, 0.0, 1.0);
    let burst_col = mix(faction_col, accent_col, hot_mix);
    let dark_halo = vec3<f32>(0.025, 0.03, 0.045);
    let stroke_blend = clamp(hot_a / max(total_a, 0.0001), 0.0, 1.0);
    return vec4<f32>(mix(dark_halo, burst_col, stroke_blend), total_a);
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
    } else if kind < 3.5 {
        return shade_building_pop(in.local_pos, bound_r, expand_dur, age, lifetime, seed);
    } else if kind < 4.5 {
        return shade_nuke_telegraph(in.local_pos, bound_r, inner_r, outer_r, age, expand_dur, lifetime, seed);
    } else if kind < 5.5 {
        return shade_nuke_reticle(in.local_pos, bound_r, inner_r, outer_r, expand_dur, lifetime, seed);
    } else if kind < 6.5 {
        return shade_nuke_launch(in.local_pos, age, lifetime, seed);
    } else if kind < 7.5 {
        return shade_mirv_split(in.local_pos, age, lifetime, seed);
    }
    if kind > 9.5 {
        return shade_entity_death(in.local_pos, bound_r, inner_r, expand_dur, age, lifetime, seed);
    }
    if kind > 8.5 {
        let t = clamp(age / max(lifetime,0.001),0.0,1.0);
        let r = length(in.local_pos);
        let fade = 1.0 - t;
        let core = max(0.0,1.0-r) * fade;
        let ring = 1.0-smoothstep(0.035,0.18,abs(r-(0.18+0.69*(1.0-fade*fade))));
        let hot = vec3<f32>(1.0,0.33+0.64*core,0.09+0.62*core*core);
        return vec4<f32>(mix(vec3<f32>(0.14,0.045,0.018),hot,expand_dur),fade*(core*core+ring*0.6));
    }
    // Compact diamond with a dark keyline when full-size warning zones saturate the viewport.
    let d = abs(in.local_pos.x) + abs(in.local_pos.y);
    let edge = 1.0 - smoothstep(0.68,0.85,d);
    let rim = 1.0 - smoothstep(0.08,0.20,abs(d-0.48));
    let core = 1.0 - smoothstep(0.12,0.28,d);
    let color = select(select(vec3<f32>(0.12,0.94,0.46),vec3<f32>(1.0,0.78,0.13),expand_dur>0.5),vec3<f32>(1.0,0.25,0.17),expand_dur>1.5);
    return vec4<f32>(mix(vec3<f32>(0.035),color,max(rim,core)),edge*0.92);

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
