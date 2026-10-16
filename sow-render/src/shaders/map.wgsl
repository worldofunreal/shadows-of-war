struct Globals {
    camera_pos: vec2<f32>,
    zoom: f32,
    time: f32,
    screen_size: vec2<f32>,
    map_size: vec2<f32>,
    border_thickness: f32,
    border_darkness: f32,
    shore_thickness: f32,
    shore_darkness: f32,
    threat_slots: array<vec4<f32>, 9>,
    alliance_celebration_slots: array<vec4<f32>, 18>,
    effect_shockwave: f32,
    effect_breathe: f32,
    effect_energy_flow: f32,
    my_player_id: f32,
    placement_active: f32,
    placement_pad_x: f32,
    placement_pad_y: f32,
    territory_opacity: f32,
    fallout_slots: array<vec4<f32>, 9>,
    blend_mode: f32,
    effect_heartbeat: f32,
    effect_war_fog: f32,
    effect_fallout: f32,
    effect_golden_hour: f32,
    effect_holo_grid: f32,
    attack_flash_target: f32,
    attack_flash_t: f32,
    alert_intensity: f32,
    fog_of_war: f32,
    tutorial_target_player: f32,
    _pad2: f32,
    alert_color: vec4<f32>,
    // Entity requesting terms in the open tutorial dialog: [player_id, age_secs, 0, 0].
    dialog_border: vec4<f32>,
    effect_vignette: f32,
    preview_effect: f32,
    preview_phase: f32,
    preview_time: f32,
    shader_params: array<vec4<f32>, 512>,
}

struct PlayerColors {
    colors: array<vec4<f32>, 256>,
}

struct PlayerSkinStyles {
    styles: array<vec4<f32>, 256>,
}

var<uniform> globals: Globals;
var<uniform> player_colors: PlayerColors;
var<uniform> player_skin_styles: PlayerSkinStyles;
var terrain_texture: texture_2d<f32>;
var owner_texture: texture_2d<u32>;
var occupancy_texture: texture_2d<u32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32((in_vertex_index & 1u) << 2u);
    let y = f32((in_vertex_index & 2u) << 1u);
    out.clip_position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5, y * 0.5);
    return out;
}

fn get_cell_owner(hex: vec2<i32>) -> u32 {
    if (hex.x < 0 || hex.y < 0 || hex.x >= i32(globals.map_size.x) || hex.y >= i32(globals.map_size.y)) {
        return 0u;
    }
    return textureLoad(owner_texture, vec2<i32>(hex.x, hex.y), 0).x & 0xFFFFu;
}

fn get_cell_terrain(hex: vec2<i32>) -> u32 {
    if (hex.x < 0 || hex.y < 0 || hex.x >= i32(globals.map_size.x) || hex.y >= i32(globals.map_size.y)) {
        return 0u;
    }
    return u32(textureLoad(terrain_texture, vec2<i32>(hex.x, hex.y), 0).x * 255.0 + 0.5);
}

fn world_to_hex(world_pos: vec2<f32>) -> vec2<i32> {
    return vec2<i32>(floor(world_pos));
}

fn hex_distance(a: vec2<i32>, b: vec2<i32>) -> i32 {
    let r1 = a.y;
    let q1 = a.x - (a.y - (a.y & 1)) / 2;
    let s1 = -q1 - r1;
    let r2 = b.y;
    let q2 = b.x - (b.y - (b.y & 1)) / 2;
    let s2 = -q2 - r2;
    return (abs(q1 - q2) + abs(r1 - r2) + abs(s1 - s2)) / 2;
}

fn hex_to_world(hex: vec2<i32>) -> vec2<f32> {
    return vec2<f32>(f32(hex.x) + 0.5, f32(hex.y) + 0.5);
}

fn get_hex_neighbor(hex: vec2<i32>, direction: i32) -> vec2<i32> {
    var offset = vec2<i32>(0, 0);
    if (direction == 0) {
        offset = vec2<i32>(1, 0); // East
    } else if (direction == 1) {
        offset = vec2<i32>(-1, 0); // West
    } else if (direction == 2) {
        offset = vec2<i32>(0, -1); // North
    } else if (direction == 3) {
        offset = vec2<i32>(0, 1); // South
    } else if (direction == 4) {
        offset = vec2<i32>(1, -1); // Northeast
    } else if (direction == 5) {
        offset = vec2<i32>(-1, -1); // Northwest
    } else if (direction == 6) {
        offset = vec2<i32>(1, 1); // Southeast
    } else if (direction == 7) {
        offset = vec2<i32>(-1, 1); // Southwest
    }
    return hex + offset;
}

fn ocean_shore_edge(cell_hex: vec2<i32>, local_pos: vec2<f32>) -> f32 {
    let width = globals.shader_params[__MAP_WATER_SHORE_WIDTH__].x;
    if width <= 0.0 {
        return 0.0;
    }
    const neighbor_dirs = array<vec2<f32>, 4>(
        vec2<f32>(1.0, 0.0),
        vec2<f32>(-1.0, 0.0),
        vec2<f32>(0.0, -1.0),
        vec2<f32>(0.0, 1.0)
    );
    var nearest_edge = 99.0;
    for (var i = 0; i < 4; i = i + 1) {
        let neighbor = get_cell_terrain(get_hex_neighbor(cell_hex, i));
        if (neighbor & 0x80u) != 0u {
            nearest_edge = min(nearest_edge, 0.5 - dot(local_pos, neighbor_dirs[i]));
        }
    }
    if nearest_edge >= 99.0 {
        return 0.0;
    }
    let softness = globals.shader_params[__MAP_WATER_SHORE_FADE__].x;
    return 1.0 - smoothstep_safe(max(width - softness, 0.0), max(width, 0.001), nearest_edge);
}

fn owner_albedo(owner_id: u32) -> vec3<f32> {
    if owner_id < 256u {
        return player_colors.colors[owner_id].rgb;
    }
    return vec3<f32>(0.5, 0.5, 0.5); // Fallback if out of bounds
}

fn smoothstep_safe(edge0: f32, edge1: f32, x: f32) -> f32 {
    let span = edge1 - edge0;
    if abs(span) < 0.000001 {
        return select(0.0, 1.0, x >= edge1);
    }
    let t = clamp((x - edge0) / span, 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn apply_skin(base: vec3<f32>, owner_id: u32, world_pos: vec2<f32>) -> vec3<f32> {
    if owner_id >= 256u {
        return base;
    }
    let style = u32(round(player_skin_styles.styles[owner_id].x));
    if style == 1u && globals.shader_params[__MAP_SKIN_STRIPE_ENABLED__].x > 0.5 {
        let width = globals.shader_params[__MAP_SKIN_STRIPE_WIDTH__].x;
        let direction = globals.shader_params[__MAP_SKIN_STRIPE_DIRECTION__].xy;
        let stripe = smoothstep_safe(0.5 - width, 0.50, abs(fract(dot(world_pos, direction) * globals.shader_params[__MAP_SKIN_STRIPE_SCALE__].x) - 0.5));
        return mix(base, min(base * globals.shader_params[__MAP_SKIN_STRIPE_TINT__].rgb, vec3<f32>(4.0)), stripe * globals.shader_params[__MAP_SKIN_STRIPE_STRENGTH__].x);
    } else if style == 2u && globals.shader_params[__MAP_SKIN_GRID_ENABLED__].x > 0.5 {
        let width = globals.shader_params[__MAP_SKIN_GRID_WIDTH__].x;
        let grid_x = smoothstep_safe(0.5 - width, 0.50, abs(fract(world_pos.x * globals.shader_params[__MAP_SKIN_GRID_SCALE__].x) - 0.5));
        let grid_y = smoothstep_safe(0.5 - width, 0.50, abs(fract(world_pos.y * globals.shader_params[__MAP_SKIN_GRID_SCALE__].x) - 0.5));
        return mix(base, globals.shader_params[__MAP_SKIN_GRID_TINT__].rgb, max(grid_x, grid_y) * globals.shader_params[__MAP_SKIN_GRID_STRENGTH__].x);
    } else if style == 3u && globals.shader_params[__MAP_SKIN_DIAMOND_ENABLED__].x > 0.5 {
        let diamond = abs(fract((world_pos.x - world_pos.y) * globals.shader_params[__MAP_SKIN_DIAMOND_SCALE__].x) - 0.5);
        let lattice = 1.0 - smoothstep_safe(0.0, globals.shader_params[__MAP_SKIN_DIAMOND_WIDTH__].x, abs(diamond - globals.shader_params[__MAP_SKIN_DIAMOND_OFFSET__].x));
        return mix(base, globals.shader_params[__MAP_SKIN_DIAMOND_TINT__].rgb, lattice * globals.shader_params[__MAP_SKIN_DIAMOND_STRENGTH__].x);
    }
    return base;
}

fn apply_alliance_celebration(
    base: vec3<f32>,
    world_pos: vec2<f32>,
    center: vec2<f32>,
    age: f32,
    accent: vec3<f32>,
    is_border: bool
) -> vec3<f32> {
    if globals.shader_params[__MAP_ALLIANCE_ENABLED__].x <= 0.5 {
        return base;
    }
    let fade_start = min(globals.shader_params[__MAP_ALLIANCE_FADE_START__].x, 0.999);
    let fade = 1.0 - smoothstep_safe(fade_start, 1.0, age);
    let intro = 1.0 - smoothstep_safe(0.0, globals.shader_params[__MAP_ALLIANCE_INTRO__].x, age);
    let wave_radius = age * globals.shader_params[__MAP_ALLIANCE_WAVE_SPEED__].x;
    let wave_inner = globals.shader_params[__MAP_ALLIANCE_WAVE_INNER__].x;
    let wave_outer = max(wave_inner + 0.001, globals.shader_params[__MAP_ALLIANCE_WAVE_OUTER__].x);
    let wave = 1.0 - smoothstep_safe(wave_inner, wave_outer, abs(distance(world_pos, center) - wave_radius));

    let tile = floor(world_pos);
    let tile_hash = fract(sin(dot(tile, vec2<f32>(127.1, 311.7))) * 43758.5453);
    let sparkle_phase = fract(age * globals.shader_params[__MAP_ALLIANCE_SPARKLE_RATE__].x + tile_hash);
    let sparkle_low = globals.shader_params[__MAP_ALLIANCE_SPARKLE_LOW__].x;
    let sparkle_peak = max(sparkle_low + 0.001, globals.shader_params[__MAP_ALLIANCE_SPARKLE_PEAK__].x);
    let sparkle_high = max(sparkle_peak + 0.001, globals.shader_params[__MAP_ALLIANCE_SPARKLE_HIGH__].x);
    let sparkle_end = max(sparkle_high + 0.001, globals.shader_params[__MAP_ALLIANCE_SPARKLE_END__].x);
    let sparkle_pulse = smoothstep_safe(sparkle_low, sparkle_peak, sparkle_phase)
        * (1.0 - smoothstep_safe(sparkle_high, sparkle_end, sparkle_phase));
    let sparkle = select(0.0, sparkle_pulse, tile_hash > 1.0 - globals.shader_params[__MAP_ALLIANCE_SPARKLE_CHANCE__].x);

    let fill_alpha = clamp(fade * (globals.shader_params[__MAP_ALLIANCE_FILL_BASE__].x + intro * globals.shader_params[__MAP_ALLIANCE_FILL_INTRO__].x + wave * globals.shader_params[__MAP_ALLIANCE_FILL_WAVE__].x + sparkle * globals.shader_params[__MAP_ALLIANCE_FILL_SPARKLE__].x), 0.0, globals.shader_params[__MAP_ALLIANCE_FILL_MAX__].x);
    var color = mix(base, accent, fill_alpha);
    color += accent * sparkle * fade * globals.shader_params[__MAP_ALLIANCE_SPARKLE_ADD__].x;
    if is_border {
        let border_sweep = fade * (globals.shader_params[__MAP_ALLIANCE_BORDER_BASE__].x + wave * globals.shader_params[__MAP_ALLIANCE_BORDER_WAVE__].x);
        color = mix(color, accent, border_sweep);
    }
    return color;
}

// ── Dialog Terms Border Glow ─────────────────────────────────────────────────
// Highlights the entity requesting terms while its dialog is open: a golden
// outline that flashes on open, breathes, and carries marching light streaks
// with per-tile glints around the whole territory border.
fn dialog_border_glow_color(world_pos: vec2<f32>, age: f32) -> vec3<f32> {
    let dialog_time = select(globals.time, globals.preview_time, globals.preview_effect == 7.0);
    let intro = 1.0 - smoothstep_safe(0.0, globals.shader_params[__MAP_DIALOG_INTRO_DURATION__].x, age);
    let march = sin((world_pos.x + world_pos.y) * globals.shader_params[__MAP_DIALOG_STREAK_SPATIAL__].x - dialog_time * globals.shader_params[__MAP_DIALOG_STREAK_RATE__].x) * 0.5 + 0.5;
    let streak_low = globals.shader_params[__MAP_DIALOG_STREAK_LOW__].x;
    let streak_high = max(streak_low + 0.001, globals.shader_params[__MAP_DIALOG_STREAK_HIGH__].x);
    let streak = smoothstep_safe(streak_low, streak_high, march);
    let breathe = sin(dialog_time * globals.shader_params[__MAP_DIALOG_GLOW_BREATHE_RATE__].x) * 0.5 + 0.5;
    let tile = floor(world_pos);
    let tile_hash = fract(sin(dot(tile, vec2<f32>(127.1, 311.7))) * 43758.5453);
    let glint_phase = fract(dialog_time * globals.shader_params[__MAP_DIALOG_GLINT_RATE__].x + tile_hash);
    let glint_rise = globals.shader_params[__MAP_DIALOG_GLINT_RISE__].x;
    let glint_peak = max(glint_rise + 0.001, globals.shader_params[__MAP_DIALOG_GLINT_PEAK__].x);
    let glint_fall = max(glint_peak + 0.001, globals.shader_params[__MAP_DIALOG_GLINT_FALL__].x);
    let glint_end = max(glint_fall + 0.001, globals.shader_params[__MAP_DIALOG_GLINT_END__].x);
    let glint = smoothstep_safe(glint_rise, glint_peak, glint_phase) * (1.0 - smoothstep_safe(glint_fall, glint_end, glint_phase));
    let gold = globals.shader_params[__MAP_DIALOG_COLOR__].rgb;
    return gold * (globals.shader_params[__MAP_DIALOG_GLOW_BASE__].x + breathe * globals.shader_params[__MAP_DIALOG_GLOW_BREATHE__].x + streak * globals.shader_params[__MAP_DIALOG_GLOW_STREAK__].x + glint * globals.shader_params[__MAP_DIALOG_GLINT_STRENGTH__].x + intro * globals.shader_params[__MAP_DIALOG_GLOW_INTRO__].x);
}

fn blend_channel_overlay(base: f32, blend: f32) -> f32 {
    if (base < 0.5) {
        return 2.0 * base * blend;
    } else {
        return 1.0 - 2.0 * (1.0 - base) * (1.0 - blend);
    }
}

fn blend_overlay(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        blend_channel_overlay(base.r, blend.r),
        blend_channel_overlay(base.g, blend.g),
        blend_channel_overlay(base.b, blend.b)
    );
}

fn shade_map(in: VertexOutput) -> vec3<f32> {
    let screen_pixel = in.uv * globals.screen_size;
    let world_x = (screen_pixel.x - globals.camera_pos.x) / globals.zoom;
    let world_y = (screen_pixel.y - globals.camera_pos.y) / globals.zoom;

    var cell_hex = world_to_hex(vec2<f32>(world_x, world_y));

    let cell_x = cell_hex.x;
    let cell_y = cell_hex.y;

    if cell_x < 0 || cell_y < 0 || cell_x >= i32(globals.map_size.x) || cell_y >= i32(globals.map_size.y) {
        return globals.shader_params[__MAP_BACKDROP__].rgb;
    }

    let hex_center = hex_to_world(cell_hex);
    let local_pos = vec2<f32>(world_x, world_y) - hex_center;

    let pixel_coords = vec2<i32>(cell_x, cell_y);
    let terrain_rgba = textureLoad(terrain_texture, pixel_coords, 0);
    let terrain_byte = u32(terrain_rgba.x * 255.0 + 0.5);
    let owner_packed = textureLoad(owner_texture, pixel_coords, 0).x;
    let owner_id = owner_packed & 0xFFFFu;
    let flash_byte = (owner_packed >> 16u) & 0xFFu;
    var flash_val = f32(flash_byte) / 255.0;
    if globals.preview_effect == 2.0 && owner_id == u32(globals.my_player_id) {
        flash_val = max(flash_val, 1.0 - globals.preview_phase);
    }
    let is_land = (terrain_byte & 0x80u) != 0u;

    var terrain_color = vec4<f32>(0.0);
    var normal = vec3<f32>(0.0, 0.0, 1.0);
    var is_specular = globals.shader_params[__MAP_SPECULAR_ENABLED__].x > 0.5;
    
    if is_land {
        let is_shoreline = (terrain_byte & 0x40u) != 0u;
        var mag_center = f32(terrain_byte & 0x1Fu);

        let land_noise = terrain_rgba.w;
        let noise_offset = (land_noise - 0.5) * globals.shader_params[__MAP_LAND_NOISE__].x; // Stable per-tile variation; no diagonal wave pattern

        if is_shoreline {
            let base = globals.shader_params[__MAP_LAND_SHORE__].rgb;
            terrain_color = vec4<f32>(base + noise_offset * globals.shader_params[__MAP_LAND_SHORE_NOISE__].x, 1.0);
        } else if mag_center < globals.shader_params[__MAP_LAND_PLAINS_LIMIT__].x {
            let r = globals.shader_params[__MAP_LAND_PLAINS__].x;
            let g = globals.shader_params[__MAP_LAND_PLAINS__].y - mag_center * globals.shader_params[__MAP_LAND_PLAINS_SLOPE__].x;
            let b = globals.shader_params[__MAP_LAND_PLAINS__].z;
            terrain_color = vec4<f32>(vec3<f32>(r, g, b) + noise_offset, 1.0); // Plains
        } else if mag_center < globals.shader_params[__MAP_LAND_HIGHLANDS_LIMIT__].x {
            let highlands = globals.shader_params[__MAP_LAND_HIGHLANDS__].rgb + vec3<f32>(mag_center * globals.shader_params[__MAP_LAND_HIGHLANDS_SLOPE__].x);
            terrain_color = vec4<f32>(highlands + noise_offset * globals.shader_params[__MAP_LAND_HIGHLANDS_NOISE__].x, 1.0);
        } else {
            let highland_base = globals.shader_params[__MAP_LAND_MOUNTAINS__].rgb;
            let snowy_peak = globals.shader_params[__MAP_LAND_SNOW__].rgb;
            let snow_range = max(abs(globals.shader_params[__MAP_LAND_SNOW_RANGE__].x), 0.001);
            let blend = clamp((mag_center - globals.shader_params[__MAP_LAND_SNOW_START__].x) / snow_range, 0.0, 1.0);
            let peak_color = mix(highland_base, snowy_peak, blend);
            terrain_color = vec4<f32>(peak_color + noise_offset * globals.shader_params[__MAP_LAND_MOUNTAINS_NOISE__].x, 1.0);
        }

        // Unpack procedural 6-directional elevation normals directly from G/B channels in O(1) time
        let dx = (terrain_rgba.y * 16.0) - 8.0;
        let dy = (terrain_rgba.z * 16.0) - 8.0;
        let normal_scale = globals.shader_params[__MAP_LAND_NORMAL_SCALE__].x / 0.1;
        normal = normalize(vec3<f32>(-dx * normal_scale, -dy * normal_scale, 1.0));
        if globals.shader_params[__MAP_RELIEF_ENABLED__].x <= 0.5 {
            normal = vec3<f32>(0.0, 0.0, 1.0);
        }
    } else {
        let is_ocean_water = (terrain_byte & 0x20u) != 0u;

        if is_ocean_water {
            let depth = min(f32(terrain_byte & 0x1Fu) * globals.shader_params[__MAP_WATER_DEPTH_SCALE__].x, globals.shader_params[__MAP_WATER_DEPTH_CAP__].x);
            let shallow_end = globals.shader_params[__MAP_WATER_SHALLOW_LIMIT__].x;
            let shelf_end = max(shallow_end + 0.001, globals.shader_params[__MAP_WATER_SHELF_LIMIT__].x);
            let mid_end = max(shelf_end + 0.001, globals.shader_params[__MAP_WATER_MID_LIMIT__].x);
            let deep_end = max(mid_end + 0.001, globals.shader_params[__MAP_WATER_DEEP_LIMIT__].x);
            let band_width = globals.shader_params[__MAP_WATER_TRANSITION__].x;
            let shallow = globals.shader_params[__MAP_WATER_SHALLOW__].rgb;
            let shelf = globals.shader_params[__MAP_WATER_SHELF__].rgb;
            let mid = globals.shader_params[__MAP_WATER_MID__].rgb;
            let deep = globals.shader_params[__MAP_WATER_DEEP__].rgb;
            let abyss = globals.shader_params[__MAP_WATER_ABYSS__].rgb;
            var color_flat = abyss;
            if depth <= deep_end { color_flat = deep; }
            if depth <= mid_end { color_flat = mid; }
            if depth <= shelf_end { color_flat = shelf; }
            if depth <= shallow_end { color_flat = shallow; }
            if band_width > 0.0 {
                if depth > shallow_end - band_width && depth < shallow_end + band_width { color_flat = mix(shallow, shelf, smoothstep_safe(shallow_end - band_width, shallow_end + band_width, depth)); }
                if depth > shelf_end - band_width && depth < shelf_end + band_width { color_flat = mix(shelf, mid, smoothstep_safe(shelf_end - band_width, shelf_end + band_width, depth)); }
                if depth > mid_end - band_width && depth < mid_end + band_width { color_flat = mix(mid, deep, smoothstep_safe(mid_end - band_width, mid_end + band_width, depth)); }
                if depth > deep_end - band_width && depth < deep_end + band_width { color_flat = mix(deep, abyss, smoothstep_safe(deep_end - band_width, deep_end + band_width, depth)); }
            }
            color_flat = mix(shallow, color_flat, globals.shader_params[__MAP_WATER_STRENGTH__].x);
            color_flat *= globals.shader_params[__MAP_WATER_BRIGHTNESS__].x;
            terrain_color = vec4<f32>(color_flat, 1.0);
        } else {
            let inland = globals.shader_params[__MAP_WATER_INLAND__].rgb * globals.shader_params[__MAP_WATER_INLAND_STRENGTH__].x * globals.shader_params[__MAP_WATER_INLAND_BRIGHTNESS__].x;
            terrain_color = vec4<f32>(inland, 1.0);
        }
    }

    // Convert sRGB palette input to linear space
    terrain_color = vec4<f32>(pow(max(terrain_color.rgb, vec3<f32>(0.0)), vec3<f32>(2.2)), terrain_color.a);

    var base_color = terrain_color.rgb;
    if !is_land && (terrain_byte & 0x20u) != 0u {
        let shore = clamp(
            ocean_shore_edge(cell_hex, local_pos)
                * globals.shader_params[__MAP_WATER_SHORE_STRENGTH__].x,
            0.0,
            1.0
        );
        let shore_color = pow(
            max(globals.shader_params[__MAP_WATER_SHORE__].rgb, vec3<f32>(0.0)),
            vec3<f32>(2.2)
        );
        base_color = mix(base_color, shore_color, shore);
    }

    // ── Peak/Valley Elevation Shading (Premium 3D Relief) ──
    if is_land {
        let mag_center = f32(terrain_byte & 0x1Fu);
        let height_factor = mag_center / 32.0;
        if globals.shader_params[__MAP_RELIEF_ENABLED__].x > 0.5 {
            let elevation_shading = globals.shader_params[__MAP_LAND_RELIEF_BASE__].x + globals.shader_params[__MAP_LAND_RELIEF_GAIN__].x * height_factor;
            base_color = base_color * elevation_shading;
        }
    }
    if owner_id > 0u && is_land {
        let albedo = owner_albedo(owner_id);
        let territory_albedo = albedo;
        
        let opacity = globals.territory_opacity;

        // Territory overlay — supports different blend modes
        var blended_color = base_color;
        let mode = i32(round(globals.blend_mode));
        if (mode == 1) {
            // Multiply
            blended_color = base_color * territory_albedo;
        } else if (mode == 2) {
            // Overlay
            blended_color = blend_overlay(base_color, territory_albedo);
        } else if (mode == 3) {
            // All Albedo
            blended_color = territory_albedo;
        } else {
            // Normal Mix
            blended_color = territory_albedo;
        }

        if (mode == 3) {
            base_color = blended_color;
        } else {
            base_color = mix(base_color, blended_color, opacity);
        }

        // Cosmetic territory pattern for the local player's equipped skin.
        base_color = apply_skin(base_color, owner_id, vec2<f32>(world_x, world_y));
        if owner_id == u32(globals.my_player_id) {
            base_color *= globals.shader_params[__MAP_LOCAL_FILL__].x;
        }

        // ── Territory Heartbeat Pulse (living empire breathe) ──
        if (globals.effect_heartbeat > 0.0) {
            let heartbeat = globals.shader_params[__MAP_HEARTBEAT_BASE__].x + globals.shader_params[__MAP_HEARTBEAT_AMOUNT__].x * sin(globals.time * globals.shader_params[__MAP_HEARTBEAT_RATE__].x + f32(owner_id) * globals.shader_params[__MAP_HEARTBEAT_PHASE__].x);
            base_color = base_color * heartbeat;
        }

        // ── Dialog Terms Interior Shimmer (requesting entity glows softly) ──
        if globals.dialog_border.x > 0.0 && owner_id == u32(globals.dialog_border.x) {
            let dialog_time = select(globals.time, globals.preview_time, globals.preview_effect == 7.0);
            let intro = 1.0 - smoothstep_safe(0.0, globals.shader_params[__MAP_DIALOG_INTRO_DURATION__].x, globals.dialog_border.y);
            let shimmer = sin((world_x + world_y) * globals.shader_params[__MAP_DIALOG_INTERIOR_SPATIAL__].x - dialog_time * globals.shader_params[__MAP_DIALOG_INTERIOR_RATE__].x) * 0.5 + 0.5;
            let wash = globals.shader_params[__MAP_DIALOG_INTERIOR_BASE__].x + globals.shader_params[__MAP_DIALOG_INTERIOR_SHIMMER__].x * shimmer + intro * globals.shader_params[__MAP_DIALOG_INTERIOR_INTRO__].x;
            base_color = mix(base_color, globals.shader_params[__MAP_DIALOG_COLOR__].rgb, clamp(wash, 0.0, globals.shader_params[__MAP_DIALOG_INTERIOR_MAX__].x));
        }

        // Conquest shockwave flash on interior
        if flash_val > 0.0 && globals.effect_shockwave > 0.0 {
            let shockwave = flash_val * globals.effect_shockwave;
            let flash_color = mix(vec3<f32>(1.0, 1.0, 1.0), albedo, 1.0 - flash_val);
            base_color = mix(base_color, flash_color, shockwave * globals.shader_params[__MAP_CONQUEST_INTERIOR_FLASH__].x);
        }
    } else if is_land {
        // ── Wilderness Atmosphere Haze (unclaimed land fog) ──
        let haze_color = globals.shader_params[__MAP_LAND_HAZE_COLOR__].rgb;
        let haze_amount = globals.shader_params[__MAP_LAND_HAZE_BASE__].x + globals.shader_params[__MAP_LAND_HAZE_WAVE__].x * sin(world_x * globals.shader_params[__MAP_LAND_HAZE_SCALE_X__].x + world_y * globals.shader_params[__MAP_LAND_HAZE_SCALE_Y__].x);
        base_color = mix(base_color, haze_color, haze_amount);
    }

    var is_border = false;
    var is_green_border = false;
    var min_border_dist = 99.0;
    var is_shore_edge = false;
    var min_shore_dist = 99.0;

    var thickness = globals.border_thickness;
    let border_darkness = globals.border_darkness;
    let shore_thickness = globals.shore_thickness;
    let shore_darkness = globals.shore_darkness;

    // Border breathe: subtle thickness pulse per owner
    if globals.effect_breathe > 0.0 && owner_id > 0u {
        let breathe = (sin(globals.time * globals.shader_params[__MAP_BORDER_BREATHE_RATE__].x + f32(owner_id)) + 1.0) * 0.5;
        thickness += breathe * globals.shader_params[__MAP_BORDER_BREATHE_AMOUNT__].x * globals.effect_breathe;
    }
    // Shockwave border explosion
    if flash_val > 0.0 && globals.effect_shockwave > 0.0 {
        thickness += flash_val * globals.shader_params[__MAP_BORDER_CONQUEST_WIDTH__].x * globals.effect_shockwave;
    }
    // Dialog terms border: the requesting entity gets a thicker, breathing line.
    if globals.dialog_border.x > 0.0 && owner_id == u32(globals.dialog_border.x) {
        let dialog_time = select(globals.time, globals.preview_time, globals.preview_effect == 7.0);
        thickness += globals.shader_params[__MAP_DIALOG_BORDER_BASE__].x + globals.shader_params[__MAP_DIALOG_BORDER_PULSE_WIDTH__].x * (sin(dialog_time * globals.shader_params[__MAP_DIALOG_BORDER_PULSE_RATE__].x) * 0.5 + 0.5);
    }

    if is_land {
        let has_border = ((owner_packed >> 24u) & 1u) != 0u;
        let has_water_neighbor = ((owner_packed >> 25u) & 1u) != 0u;

        if has_border || has_water_neighbor {
            let is_tribe = owner_id >= 200u;
            const neighbor_dirs = array<vec2<f32>, 4>(
                vec2<f32>(1.0, 0.0),  // East
                vec2<f32>(-1.0, 0.0), // West
                vec2<f32>(0.0, -1.0), // North
                vec2<f32>(0.0, 1.0)   // South
            );

            for (var i = 0; i < 4; i = i + 1) {
                let neighbor_hex = get_hex_neighbor(cell_hex, i);
                let neighbor_terrain = get_cell_terrain(neighbor_hex);
                let neighbor_owner = get_cell_owner(neighbor_hex);
                let neighbor_is_land = (neighbor_terrain & 0x80u) != 0u;

                let dir = neighbor_dirs[i];
                let dist_to_edge = 0.5 - dot(local_pos, dir);

                if !neighbor_is_land {
                    if dist_to_edge < shore_thickness {
                        is_shore_edge = true;
                        min_shore_dist = min(min_shore_dist, dist_to_edge);
                    }
                } else {
                    if owner_id > 0u && neighbor_owner != owner_id {
                        let green_exists = is_tribe && (neighbor_owner >= 200u);
                        // LOD 3 Optimization: Skip drawing borders between minor tribes to reduce macro noise
                        if globals.zoom < globals.shader_params[__MAP_TRIBE_ZOOM_CUTOFF__].x && green_exists {
                            continue;
                        }
                        
                        if dist_to_edge < thickness {
                            is_border = true;
                            min_border_dist = min(min_border_dist, dist_to_edge);
                            if green_exists {
                                is_green_border = true;
                            }
                        }
                    }
                }
            }
        }

        if is_shore_edge {
            let shore_t = 1.0 - smoothstep_safe(shore_thickness - globals.shader_params[__MAP_COAST_SOFTNESS__].x, shore_thickness, min_shore_dist);
            // Sandy coast tint — not the near-black political border line
            var shore_col = globals.shader_params[__MAP_COAST_COLOR__].rgb * shore_darkness;
            if (terrain_byte & 0x40u) != 0u {
                shore_col = globals.shader_params[__MAP_COAST_SHORELINE_COLOR__].rgb * shore_darkness;
            }
            base_color = mix(base_color, shore_col, shore_t * globals.shader_params[__MAP_COAST_STRENGTH__].x);
        }

        if is_border {
            let border_t = 1.0 - smoothstep_safe(thickness - globals.shader_params[__MAP_BORDER_SOFTNESS__].x, thickness, min_border_dist);
            if is_green_border {
                let border_col = globals.shader_params[__MAP_BORDER_TRIBE_COLOR__].rgb * border_darkness;
                base_color = mix(base_color, border_col, border_t);
            } else {
                var border_albedo = globals.shader_params[__MAP_BORDER_NEUTRAL_COLOR__].rgb * border_darkness;
                if owner_id > 0u {
                    border_albedo = owner_albedo(owner_id) * border_darkness;
                    if owner_id == u32(globals.my_player_id) {
                        border_albedo = owner_albedo(owner_id) * globals.shader_params[__MAP_LOCAL_BORDER__].x;
                    }

                    // ── Dynamic Conquest Border Shockwave Pulse ──
                    if flash_val > 0.0 && globals.effect_shockwave > 0.0 {
                        let pulse = 1.0 - min_border_dist / max(abs(globals.border_thickness), 0.0001);
                        // Keep conquest pulse in the conquering player's hue instead of whitening it.
                        let conquer_glow = min(owner_albedo(owner_id) * globals.shader_params[__MAP_BORDER_CONQUEST_COLOR_GAIN__].x, vec3<f32>(1.0, 1.0, 1.0));
                        border_albedo = mix(
                            border_albedo,
                            conquer_glow,
                            flash_val * globals.effect_shockwave * pulse * globals.shader_params[__MAP_BORDER_CONQUEST_BLEND__].x
                        );
                    }

                    // ── Contested Border Energy Crackling (PvP shimmer) ──
                    if (globals.effect_energy_flow > 0.0) {
                        let neighbor_hex_0 = get_hex_neighbor(cell_hex, 0);
                        let contested_owner = get_cell_owner(neighbor_hex_0);
                        if contested_owner > 0u && contested_owner != owner_id {
                            let enemy_albedo = owner_albedo(contested_owner);
                            let energy_t = sin(globals.time * globals.shader_params[__MAP_BORDER_CONTESTED_RATE__].x) * 0.5 + 0.5;
                            let shimmer = enemy_albedo * border_darkness * globals.shader_params[__MAP_BORDER_CONTESTED_BRIGHTNESS__].x;
                            border_albedo = mix(border_albedo, shimmer, energy_t * globals.shader_params[__MAP_BORDER_CONTESTED_STRENGTH__].x);
                        }
                    }
                    // ── Attack Border Flash (red flash on click) ──
                    if globals.shader_params[__MAP_ATTACK_FLASH_ENABLED__].x > 0.5 && globals.attack_flash_t > 0.0 && owner_id == u32(globals.attack_flash_target) {
                        border_albedo = mix(border_albedo, globals.shader_params[__MAP_BORDER_ATTACK_COLOR__].rgb, globals.attack_flash_t * globals.shader_params[__MAP_BORDER_ATTACK_BLEND__].x);
                    }
                    if globals.tutorial_target_player > 0.0 && owner_id == u32(globals.tutorial_target_player) {
                        let gold_glow = globals.shader_params[__MAP_BORDER_TUTORIAL_COLOR__].rgb;
                        let pulse = globals.shader_params[__MAP_BORDER_TUTORIAL_BASE__].x + sin(globals.time * globals.shader_params[__MAP_BORDER_TUTORIAL_RATE__].x) * globals.shader_params[__MAP_BORDER_TUTORIAL_AMOUNT__].x;
                        border_albedo = mix(border_albedo, gold_glow * globals.shader_params[__MAP_BORDER_TUTORIAL_GLOW__].x, pulse);
                    }
                }

                base_color = mix(base_color, border_albedo, border_t);
            }

            // ── Dialog Terms Border Glow (overrides normal/tribe border) ──
            if globals.dialog_border.x > 0.0 && owner_id == u32(globals.dialog_border.x) {
                let glow = dialog_border_glow_color(vec2<f32>(world_x, world_y), globals.dialog_border.y);
                base_color = mix(base_color, glow, border_t * globals.shader_params[__MAP_DIALOG_BORDER_BLEND__].x);
            }
        }
    }

    // ── Bilateral alliance celebration: wave + territorial shimmer ──
    if owner_id > 0u && is_land {
        for (var ai = 0; ai < 9; ai = ai + 1) {
            let first = globals.alliance_celebration_slots[ai * 2];
            let second = globals.alliance_celebration_slots[ai * 2 + 1];
            if first.x > 0.0 && u32(round(first.x)) == owner_id {
                base_color = apply_alliance_celebration(
                    base_color,
                    hex_center,
                    first.yz,
                    first.w,
                    globals.shader_params[__MAP_ALLIANCE_PLAYER_COLOR__].rgb,
                    is_border
                );
            } else if second.x > 0.0 && u32(round(second.x)) == owner_id {
                base_color = apply_alliance_celebration(
                    base_color,
                    hex_center,
                    second.yz,
                    second.w,
                    globals.shader_params[__MAP_ALLIANCE_PARTNER_COLOR__].rgb,
                    is_border
                );
            }
        }
    }

    // ── WAR FOG + FRONTIER GLOW ──
    if (globals.effect_war_fog > 0.0) {
        let world_pos_hex = hex_to_world(cell_hex);
        let threat_time = select(
            globals.time,
            globals.preview_time,
            globals.preview_effect == 4.0 || globals.preview_effect == 5.0
        );
        for (var ti = 0; ti < 9; ti = ti + 1) {
            let slot = globals.threat_slots[ti];
            let packed = u32(slot.w + 0.5);
            let target_id = packed / 1024u;
            let attacker_id = packed % 1024u;
            var radius_scale = globals.shader_params[__MAP_COMBAT_PVP_REACH_SCALE__].x;
            if target_id == 0u {
                radius_scale = globals.shader_params[__MAP_EXPANSION_OTHER_REACH_SCALE__].x;
                if attacker_id == u32(globals.my_player_id + 0.5) {
                    radius_scale = globals.shader_params[__MAP_EXPANSION_LOCAL_REACH_SCALE__].x;
                }
            }
            let radius = slot.z * radius_scale;
            if radius <= 0.0 { continue; }
            if target_id != owner_id { continue; }

            let front_world = vec2<f32>(
                slot.x + 0.5,
                slot.y + 0.5
            );
            let dist = distance(world_pos_hex, front_world);
            let threat = 1.0 - smoothstep_safe(0.0, radius, dist);
            if threat <= 0.0 { continue; }

            if target_id == 0u && globals.shader_params[__MAP_EXPANSION_ENABLED__].x > 0.5 {
                let is_player = (attacker_id == u32(globals.my_player_id + 0.5));
                if is_player {
                    // ── PLAYER WILDERNESS EXPANSION: Special High-Tech Resonance ──
                    let player_color = owner_albedo(attacker_id);
                    let cyan_core = player_color * vec3<f32>(globals.shader_params[__MAP_EXPANSION_LOCAL_CYAN_RED__].x, globals.shader_params[__MAP_EXPANSION_LOCAL_CYAN_GREEN__].x, globals.shader_params[__MAP_EXPANSION_LOCAL_CYAN_BLUE__].x) + globals.shader_params[__MAP_EXPANSION_LOCAL_CYAN_LIFT__].rgb;
                    let indigo_glow = player_color * vec3<f32>(globals.shader_params[__MAP_EXPANSION_LOCAL_DARK_RED__].x, globals.shader_params[__MAP_EXPANSION_LOCAL_DARK_GREEN__].x, globals.shader_params[__MAP_EXPANSION_LOCAL_DARK_BLUE__].x) + globals.shader_params[__MAP_EXPANSION_LOCAL_DARK_LIFT__].rgb;

                    // Gentle base radial glow
                    let glow_color = mix(indigo_glow, cyan_core, threat);
                    let glow_blend = threat * globals.shader_params[__MAP_EXPANSION_LOCAL_GLOW__].x;

                    // Sharp scanning pulse ring propagating outwards
                    let wave_phase = dist * globals.shader_params[__MAP_EXPANSION_LOCAL_WAVE_SCALE__].x - threat_time * globals.shader_params[__MAP_EXPANSION_LOCAL_WAVE_SPEED__].x;
                    let scan_line = max(0.0, sin(wave_phase));
                    let pulse = pow(scan_line, globals.shader_params[__MAP_EXPANSION_LOCAL_WAVE_SHARPNESS__].x) * threat * globals.shader_params[__MAP_EXPANSION_LOCAL_WAVE_STRENGTH__].x;

                    // High-frequency subtle radar ripples
                    let ripple_phase = dist * globals.shader_params[__MAP_EXPANSION_LOCAL_RIPPLE_SCALE__].x - threat_time * globals.shader_params[__MAP_EXPANSION_LOCAL_RIPPLE_SPEED__].x;
                    let ripple = (sin(ripple_phase) + 1.0) * 0.5;
                    let ripple_glow = ripple * threat * threat * globals.shader_params[__MAP_EXPANSION_LOCAL_RIPPLE_STRENGTH__].x;

                    // Sharp leading edge at 90% of the radius
                    let edge_dist = abs(dist - radius * globals.shader_params[__MAP_EXPANSION_LOCAL_EDGE_POSITION__].x);
                    let edge_width = max(globals.shader_params[__MAP_EXPANSION_LOCAL_EDGE_WIDTH__].x, 0.001);
                    let edge = (1.0 - smoothstep_safe(0.0, edge_width, edge_dist)) * globals.shader_params[__MAP_EXPANSION_LOCAL_EDGE_STRENGTH__].x;

                    base_color = mix(base_color, glow_color, glow_blend);
                    base_color += cyan_core * pulse;
                    base_color += cyan_core * edge;
                    base_color += cyan_core * ripple_glow;
                } else {
                    // ── OTHER EMPIRES: Default Frontier Glow ──
                    let atk_color = owner_albedo(attacker_id);
                    let gold = globals.shader_params[__MAP_EXPANSION_OTHER_FRONTIER__].rgb;
                    let frontier_bright = mix(atk_color, gold, globals.shader_params[__MAP_EXPANSION_OTHER_BRIGHT_MIX__].x) * globals.shader_params[__MAP_EXPANSION_OTHER_BRIGHT_GAIN__].x + vec3<f32>(globals.shader_params[__MAP_EXPANSION_OTHER_BRIGHT_LIFT__].x);
                    let frontier_dark = mix(atk_color, gold, globals.shader_params[__MAP_EXPANSION_OTHER_DARK_MIX__].x) * globals.shader_params[__MAP_EXPANSION_OTHER_DARK_GAIN__].x;

                    // Gentle radial glow
                    let glow_color = mix(frontier_dark, frontier_bright, threat);
                    let glow_blend = threat * globals.shader_params[__MAP_EXPANSION_OTHER_GLOW__].x;

                    // Slower, wider expansion ripples
                    let wave_phase = dist * globals.shader_params[__MAP_EXPANSION_OTHER_WAVE_SCALE__].x - threat_time * globals.shader_params[__MAP_EXPANSION_OTHER_WAVE_SPEED__].x;
                    let ripple = (sin(wave_phase) + 1.0) * 0.5;
                    let ripple_glow = ripple * threat * globals.shader_params[__MAP_EXPANSION_OTHER_RIPPLE__].x;

                    // Soft leading edge (no aggressive corona)
                    let edge_dist = abs(dist - radius * globals.shader_params[__MAP_EXPANSION_OTHER_EDGE_POSITION__].x);
                    let edge_width = max(globals.shader_params[__MAP_EXPANSION_OTHER_EDGE_WIDTH__].x, 0.001);
                    let edge = (1.0 - smoothstep_safe(0.0, edge_width, edge_dist)) * globals.shader_params[__MAP_EXPANSION_OTHER_EDGE_STRENGTH__].x;
                    let edge_color = min(frontier_bright, vec3<f32>(1.0));

                    base_color = mix(base_color, glow_color, glow_blend);
                    base_color += edge_color * edge;
                    base_color += frontier_bright * ripple_glow;
                }
            } else if target_id != 0u && globals.shader_params[__MAP_COMBAT_HAZE_ENABLED__].x > 0.5 {
                // ── WAR FOG: PvP attack ──
                let atk_color = owner_albedo(attacker_id);
                let atk_bright = atk_color * globals.shader_params[__MAP_COMBAT_PVP_BRIGHT_GAIN__].x + vec3<f32>(globals.shader_params[__MAP_COMBAT_PVP_BRIGHT_LIFT__].x);
                let atk_dark = atk_color * globals.shader_params[__MAP_COMBAT_PVP_DARK_GAIN__].x;

                // Desaturation — territory drains to grey
                let lum = dot(base_color, vec3<f32>(0.299, 0.587, 0.114));
                let desat = mix(base_color, vec3<f32>(lum), threat * globals.shader_params[__MAP_COMBAT_PVP_DESATURATION__].x);

                // Attacker smoke
                let smoke_color = mix(atk_dark, atk_bright, threat * threat) * globals.shader_params[__MAP_COMBAT_PVP_SMOKE_COLOR__].rgb;
                let smoke_blend = threat * globals.shader_params[__MAP_COMBAT_PVP_SMOKE__].x;

                // Ripple waves
                let wave_phase = dist * globals.shader_params[__MAP_COMBAT_PVP_WAVE_SCALE__].x - threat_time * globals.shader_params[__MAP_COMBAT_PVP_WAVE_SPEED__].x;
                let ripple = (sin(wave_phase) + 1.0) * 0.5;
                let ripple_phase = dist * globals.shader_params[__MAP_COMBAT_PVP_RIPPLE_SCALE__].x - threat_time * globals.shader_params[__MAP_COMBAT_PVP_RIPPLE_SPEED__].x;
                let ripple_value = (sin(ripple_phase) + 1.0) * 0.5;
                let ripple_intensity = ripple_value * threat * threat * globals.shader_params[__MAP_COMBAT_PVP_RIPPLE__].x;

                // Corona front
                let corona_dist = abs(dist - radius * globals.shader_params[__MAP_COMBAT_PVP_CORONA_POSITION__].x);
                let corona_width = max(globals.shader_params[__MAP_COMBAT_PVP_CORONA_WIDTH__].x, 0.001);
                let corona = (1.0 - smoothstep_safe(0.0, corona_width, corona_dist)) * globals.shader_params[__MAP_COMBAT_PVP_CORONA_STRENGTH__].x;
                let corona_color = min(atk_color * globals.shader_params[__MAP_COMBAT_PVP_CORONA_COLOR_GAIN__].x + vec3<f32>(globals.shader_params[__MAP_COMBAT_PVP_CORONA_COLOR_LIFT__].x), vec3<f32>(1.0));

                var war_color = mix(desat, smoke_color, smoke_blend);
                war_color += corona_color * corona;
                war_color += atk_bright * ripple_intensity;
                base_color = war_color;
            }
        }
    }

    // ── NUCLEAR FALLOUT CONTAMINATION ZONES ──
    if (globals.effect_fallout > 0.0) {
        let cell_world = hex_to_world(cell_hex);
        for (var fi = 0; fi < 9; fi = fi + 1) {
            let slot = globals.fallout_slots[fi];
            var f_radius = slot.z;
            if f_radius <= 0.0 { continue; }

            let alpha_p = slot.w;
            let nuke_time = select(globals.time, globals.preview_time, globals.preview_effect == 6.0);
            let f_center = vec2<f32>(
                slot.x + 0.5,
                slot.y + 0.5
            );
            let dist = distance(cell_world, f_center);
            f_radius *= globals.shader_params[__MAP_NUKE_FALLOUT_RADIUS__].x;
            if f_radius <= 0.0 || dist > f_radius { continue; }

            let elapsed = (1.0 - alpha_p) * globals.shader_params[__MAP_NUKE_ZONE_DURATION__].x;

            // 1. SCORCHED EARTH (CRATER / BURN)
            let burn_radius = f_radius * globals.shader_params[__MAP_NUKE_BURN_RADIUS__].x;
            if globals.shader_params[__MAP_NUKE_BURN_ENABLED__].x > 0.5 && burn_radius > 0.0 && dist <= burn_radius {
                let burn_factor = (1.0 - dist / max(burn_radius, 0.0001)) * globals.shader_params[__MAP_NUKE_BURN_STRENGTH__].x * alpha_p;
                let charcoal = globals.shader_params[__MAP_NUKE_BURN_COLOR__].rgb;
                base_color = mix(base_color, charcoal, burn_factor);
            }

            // 2. ACTIVE EXPLOSION & STAGGERED SHOCKWAVE (First 3.0 seconds)
            if globals.shader_params[__MAP_NUKE_EXPLOSION_ENABLED__].x > 0.5 && elapsed < globals.shader_params[__MAP_NUKE_ACTIVE_DURATION__].x {
                let p_exp = elapsed / globals.shader_params[__MAP_NUKE_ACTIVE_DURATION__].x;
                let shockwave_r = p_exp * f_radius;

                let fire_noise = fract(sin(world_x * globals.shader_params[__MAP_NUKE_FIRE_NOISE_X__].x + world_y * globals.shader_params[__MAP_NUKE_FIRE_NOISE_Y__].x - nuke_time * globals.shader_params[__MAP_NUKE_FIRE_NOISE_SPEED__].x) * 43758.5453);

                // --- A. Rising Mushroom Cloud & Stem ---
                let rise = p_exp * f_radius * globals.shader_params[__MAP_NUKE_CLOUD_RISE__].x;
                let cap_center = f_center - vec2<f32>(0.0, rise);
                let dist_cap = distance(cell_world, cap_center);
                let cap_radius = f_radius * globals.shader_params[__MAP_NUKE_CLOUD_RADIUS__].x * (globals.shader_params[__MAP_NUKE_CLOUD_INITIAL__].x + globals.shader_params[__MAP_NUKE_CLOUD_GROWTH__].x * p_exp);
                let cap_r = max(cap_radius, 0.0001);

                let dist_stem = abs(cell_world.x - f_center.x);
                let stem_width = f_radius * globals.shader_params[__MAP_NUKE_STEM_WIDTH__].x * (1.0 - p_exp * globals.shader_params[__MAP_NUKE_STEM_TAPER__].x) * (1.0 + globals.shader_params[__MAP_NUKE_STEM_WOBBLE__].x * sin(cell_world.y * globals.shader_params[__MAP_NUKE_STEM_WAVE_SCALE__].x - nuke_time * globals.shader_params[__MAP_NUKE_STEM_WAVE_SPEED__].x));
                let stem_w = max(stem_width, 0.0001);
                let is_in_stem = stem_width > 0.0 && cell_world.y >= cap_center.y && cell_world.y <= f_center.y && dist_stem <= stem_width;
                let is_in_cap = cap_radius > 0.0 && dist_cap <= cap_radius;

                if is_in_cap || is_in_stem {
                    let age_fade = 1.0 - p_exp;

                    var dist_factor = 0.0;
                    if is_in_cap {
                        dist_factor = 1.0 - dist_cap / cap_r;
                    } else {
                        dist_factor = 1.0 - dist_stem / stem_w;
                    }

                    let fire_white = globals.shader_params[__MAP_NUKE_FIRE_WHITE__].rgb;
                    let fire_yellow = globals.shader_params[__MAP_NUKE_FIRE_YELLOW__].rgb;
                    let fire_red = globals.shader_params[__MAP_NUKE_FIRE_RED__].rgb;
                    let smoke = globals.shader_params[__MAP_NUKE_SMOKE_COLOR__].rgb;

                    let heat = clamp(age_fade * globals.shader_params[__MAP_NUKE_HEAT_AGE__].x - (1.0 - dist_factor) * globals.shader_params[__MAP_NUKE_HEAT_DISTANCE__].x + (fire_noise - 0.5) * globals.shader_params[__MAP_NUKE_HEAT_NOISE__].x, 0.0, 1.0);

                    var fire_rgb = vec3<f32>(0.0);
                    let white_threshold = globals.shader_params[__MAP_NUKE_WHITE_THRESHOLD__].x;
                    let red_threshold = min(globals.shader_params[__MAP_NUKE_RED_THRESHOLD__].x, white_threshold - 0.001);
                    if heat > white_threshold {
                        fire_rgb = mix(fire_yellow, fire_white, (heat - white_threshold) / max(1.0 - white_threshold, 0.001));
                    } else if heat > red_threshold {
                        fire_rgb = mix(fire_red, fire_yellow, (heat - red_threshold) / max(white_threshold - red_threshold, 0.001));
                    } else {
                        fire_rgb = mix(smoke, fire_red, heat / max(red_threshold, 0.001));
                    }

                    base_color = mix(base_color, fire_rgb, smoothstep_safe(0.0, globals.shader_params[__MAP_NUKE_FIRE_BLEND__].x, heat) * age_fade);
                }

                // --- B. Blinding Ground-Zero Flash ---
            }

            if globals.shader_params[__MAP_NUKE_FLASH_ENABLED__].x > 0.5 && elapsed < globals.shader_params[__MAP_NUKE_FLASH_DURATION__].x {
                let flash_fade = 1.0 - elapsed / globals.shader_params[__MAP_NUKE_FLASH_DURATION__].x;
                let flash_radius = f_radius * globals.shader_params[__MAP_NUKE_FLASH_RADIUS__].x;
                if flash_radius > 0.0 && dist <= flash_radius {
                    let flash_intensity = (1.0 - dist / max(flash_radius, 0.0001)) * flash_fade * globals.shader_params[__MAP_NUKE_FLASH_STRENGTH__].x;
                    base_color += globals.shader_params[__MAP_NUKE_FLASH_COLOR__].rgb * flash_intensity;
                }
            }

            // --- Expanding shockwave can be tuned independently from the fireball. ---
            if globals.shader_params[__MAP_NUKE_SHOCKWAVE_ENABLED__].x > 0.5 && elapsed < globals.shader_params[__MAP_NUKE_ACTIVE_DURATION__].x {
                let p_exp = elapsed / globals.shader_params[__MAP_NUKE_ACTIVE_DURATION__].x;
                let shockwave_r = p_exp * f_radius;
                let wave_width = globals.shader_params[__MAP_NUKE_WAVE_WIDTH__].x;
                let edge_dist = abs(dist - shockwave_r);
                if edge_dist < wave_width {
                    let wave_factor = 1.0 - edge_dist / wave_width;
                    let wave_pulse = wave_factor * wave_factor * (1.0 - p_exp);
                    base_color += globals.shader_params[__MAP_NUKE_WAVE_COLOR__].rgb * wave_pulse * globals.shader_params[__MAP_NUKE_WAVE_STRENGTH__].x;
                }
            }

            // 3. TOXIC RADIOACTIVE FALLOUT (Pulsing, boiling green sludge)
            if globals.shader_params[__MAP_NUKE_FALLOUT_ENABLED__].x > 0.5 && dist <= f_radius {
                let falloff = 1.0 - dist / f_radius;
                let pulse = sin(nuke_time * globals.shader_params[__MAP_NUKE_FALLOUT_PULSE_RATE__].x) * globals.shader_params[__MAP_NUKE_FALLOUT_PULSE_AMOUNT__].x + globals.shader_params[__MAP_NUKE_FALLOUT_PULSE_BASE__].x;

                let n1 = fract(sin(world_x * globals.shader_params[__MAP_NUKE_NOISE1_X__].x + world_y * globals.shader_params[__MAP_NUKE_NOISE1_Y__].x + nuke_time * globals.shader_params[__MAP_NUKE_NOISE1_SPEED__].x) * 43758.5453);
                let n2 = fract(sin(world_x * globals.shader_params[__MAP_NUKE_NOISE2_X__].x - world_y * globals.shader_params[__MAP_NUKE_NOISE2_Y__].x + nuke_time * globals.shader_params[__MAP_NUKE_NOISE2_SPEED__].x) * 23421.631);
                let noise = (n1 + n2) * 0.5;

                let bubbles = sin(world_x * globals.shader_params[__MAP_NUKE_BUBBLE_SCALE_X__].x + sin(nuke_time * globals.shader_params[__MAP_NUKE_BUBBLE_SCALE_X__].x + world_y) * globals.shader_params[__MAP_NUKE_BUBBLE_WARP__].x) * cos(world_y * globals.shader_params[__MAP_NUKE_BUBBLE_SCALE_Y__].x + cos(nuke_time * globals.shader_params[__MAP_NUKE_BUBBLE_SCALE_Y__].x - world_x) * globals.shader_params[__MAP_NUKE_BUBBLE_WARP__].x);
                let bubble_threshold = min(globals.shader_params[__MAP_NUKE_BUBBLE_THRESHOLD__].x, 0.999);
                let bubble_glow = smoothstep_safe(bubble_threshold, 1.0, bubbles) * pulse * globals.shader_params[__MAP_NUKE_BUBBLE_STRENGTH__].x * alpha_p;

                let toxic_green = globals.shader_params[__MAP_NUKE_FALLOUT_GREEN__].rgb;
                let toxic_bright = globals.shader_params[__MAP_NUKE_FALLOUT_BRIGHT__].rgb;
                let glow_color = mix(toxic_green, toxic_bright, noise * globals.shader_params[__MAP_NUKE_FALLOUT_NOISE_MIX__].x) * globals.shader_params[__MAP_NUKE_FALLOUT_COLOR_GAIN__].x;

                let intensity = falloff * falloff * alpha_p * pulse;
                let core_edge = max(globals.shader_params[__MAP_NUKE_FALLOUT_CORE_EDGE__].x, 0.001);
                let core_mix = smoothstep_safe(0.0, core_edge, falloff * alpha_p);

                base_color = mix(base_color, glow_color, core_mix * globals.shader_params[__MAP_NUKE_FALLOUT_CORE_BLEND__].x);
                base_color += glow_color * intensity * globals.shader_params[__MAP_NUKE_FALLOUT_GLOW__].x;
                base_color += globals.shader_params[__MAP_NUKE_BUBBLE_COLOR__].rgb * bubble_glow * globals.shader_params[__MAP_NUKE_BUBBLE_MIX__].x;
            }
        }
    }

    // ── Upgraded Lighting (Sun directional diffuse + Specular highlights) ──
    let light_vector = globals.shader_params[__MAP_LIGHT_DIRECTION__].xyz;
    let light_dir = light_vector / max(length(light_vector), 0.0001);
    if globals.shader_params[__MAP_DIFFUSE_ENABLED__].x > 0.5 {
        let diffuse = max(globals.shader_params[__MAP_LIGHT_DIFFUSE_FLOOR__].x, dot(normal, light_dir));
        base_color = base_color * (diffuse * globals.shader_params[__MAP_LIGHT_DIFFUSE_GAIN__].x);
    }

    if is_specular {
        let view_dir = vec3<f32>(0.0, 0.0, 1.0);
        let half_dir = normalize(light_dir + view_dir);
        let spec = pow(max(0.0, dot(normal, half_dir)), globals.shader_params[__MAP_SPECULAR_POWER__].x);
        base_color = base_color + globals.shader_params[__MAP_SPECULAR_COLOR__].rgb * spec * globals.shader_params[__MAP_SPECULAR_STRENGTH__].x;
    }

    // ── Golden Hour Sun Sweep (slow warm-cool color temperature cycle) ──
    if (globals.effect_golden_hour > 0.0) {
        let sun_phase = sin(globals.time * globals.shader_params[__MAP_WARM_SPEED__].x) * 0.5 + 0.5;
        let warm_tint = vec3<f32>(globals.shader_params[__MAP_WARM_RED_BASE__].x + globals.shader_params[__MAP_WARM_RED_GAIN__].x * sun_phase, globals.shader_params[__MAP_WARM_GREEN_BASE__].x + globals.shader_params[__MAP_WARM_GREEN_GAIN__].x * sun_phase, globals.shader_params[__MAP_WARM_BLUE_BASE__].x + globals.shader_params[__MAP_WARM_BLUE_GAIN__].x * sun_phase);
        base_color = base_color * warm_tint;
    }

    // Hex SDF reused by building-placement grid
    let min_dist_to_edge = 0.5 - max(abs(local_pos.x), abs(local_pos.y));

    // ── Screen Vignetting (Soft shading at screen edges) ──
    if (globals.effect_vignette > 0.0) {
        let d_center = length(in.uv - 0.5);
        let vignette_inner = globals.shader_params[__MAP_VIGNETTE_INNER__].x;
        let vignette_outer = max(vignette_inner + 0.001, globals.shader_params[__MAP_VIGNETTE_OUTER__].x);
        let vignette = 1.0 - smoothstep_safe(vignette_inner, vignette_outer, d_center);
        base_color = base_color * mix(globals.shader_params[__MAP_VIGNETTE_EDGE_GAIN__].x, globals.shader_params[__MAP_VIGNETTE_CENTER_GAIN__].x, vignette);
    }

    // Construction grid shows real occupied cells; the selected 4x4 base is
    // validated and drawn by Rust, so free individual cells never promise a fit.
    if (globals.placement_active > 0.0 && is_land && owner_id == u32(globals.my_player_id)) {
        let occupied = textureLoad(occupancy_texture, cell_hex, 0).x > 0u;
        let color = select(vec3<f32>(0.78, 0.86, 0.82), vec3<f32>(0.7, 0.35, 0.3), occupied);
        let edge_width = max(fwidth(min_dist_to_edge), 0.001);
        let edge = 1.0 - smoothstep_safe(0.0, edge_width, min_dist_to_edge);
        let readable = smoothstep_safe(2.0, 5.0, globals.zoom);
        base_color = mix(base_color, color, edge * readable * select(select(0.20, 0.28, globals.effect_holo_grid > 0.0), 0.42, occupied));
        base_color = mix(base_color, vec3<f32>(0.12, 0.08, 0.08), select(0.0, 0.22, occupied));
    }

    // ── Fog of War shading ──
    if (globals.fog_of_war > 0.0) {
        let vision_fade = f32((owner_packed >> 26u) & 0x3Fu) / 63.0;
        base_color = base_color * mix(globals.shader_params[__MAP_FOG_VISIBILITY_FLOOR__].x, globals.shader_params[__MAP_FOG_VISIBILITY_GAIN__].x, vision_fade);
    }

    // ── Unified Viewport Alert Vignette ──
    if globals.shader_params[__MAP_ALERTS_ENABLED__].x > 0.5 && globals.alert_intensity > 0.0 {
        let alert_time = select(globals.time, globals.preview_time, globals.preview_effect == 8.0);
        let edge_dist = min(min(in.uv.x, 1.0 - in.uv.x), min(in.uv.y, 1.0 - in.uv.y));
        let edge_width = globals.shader_params[__MAP_ALERT_EDGE_WIDTH__].x;
        let edge_softness = globals.shader_params[__MAP_ALERT_EDGE_SOFTNESS__].x;
        var vignette = 0.0;
        if edge_width > 0.0 {
            vignette = 1.0 - smoothstep_safe(max(edge_width - edge_softness, 0.0), edge_width, edge_dist);
        }
        let breathe = sin(alert_time * globals.shader_params[__MAP_ALERT_BREATHE_RATE__].x) * globals.shader_params[__MAP_ALERT_BREATHE_AMOUNT__].x + globals.shader_params[__MAP_ALERT_BREATHE_BASE__].x;
        let intensity = globals.alert_intensity * vignette * breathe;
        base_color = mix(base_color, globals.alert_color.rgb, intensity * globals.alert_color.a);
    }

    return base_color;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(shade_map(in), 1.0);
}

@fragment
fn fs_main_srgb(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = shade_map(in);
    return vec4(pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2)), 1.0);
}
