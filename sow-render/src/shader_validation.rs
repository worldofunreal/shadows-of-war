//! Headless WGSL validation. Blade parses and validates every shader with
//! naga at pipeline creation — the exact same pass runs on the web build at
//! runtime. These tests run it under `cargo test` so a broken shader fails
//! the build instead of the running game.

fn parse_and_validate(source: &str) -> naga::Module {
    let module = naga::front::wgsl::parse_str(source).unwrap_or_else(|error| {
        panic!(
            "WGSL parse failed:\n{}",
            error.emit_to_string_with_path(source, "")
        )
    });
    // Bindings are assigned at pipeline creation (blade fills them in), so
    // mirror blade's own runtime pass and skip that stage here.
    let flags = naga::valid::ValidationFlags::all() ^ naga::valid::ValidationFlags::BINDINGS;
    naga::valid::Validator::new(flags, naga::valid::Capabilities::empty())
        .validate(&module)
        .unwrap_or_else(|error| panic!("WGSL validation failed: {error:?}"));
    module
}

fn assert_struct_size(module: &naga::Module, struct_name: &str, expected: usize) {
    let actual = module
        .types
        .iter()
        .find(|&(_, ty)| ty.name.as_deref() == Some(struct_name))
        .map(|(_, ty)| match ty.inner {
            naga::TypeInner::Struct { span, .. } => span as usize,
            _ => panic!("'{struct_name}' is not a struct in the shader"),
        })
        .unwrap_or_else(|| panic!("Struct '{struct_name}' is not found in the shader"));
    assert_eq!(
        actual, expected,
        "'{struct_name}' host/shader layout mismatch"
    );
}

#[test]
fn map_shader_parses_and_matches_globals_layout() {
    let source = crate::shader_params::prepare_shader("MAP_", include_str!("shaders/map.wgsl"));
    let module = parse_and_validate(&source);
    assert_struct_size(&module, "Globals", std::mem::size_of::<crate::MapGlobals>());
}

/// The hero conquest tints are read from the owner's style row. A refactor that drops
/// either flag or the mountain test would silently remove the visual; naga cannot see that.
#[test]
fn map_shader_tints_hero_conquest_flashes() {
    let source = include_str!("shaders/map.wgsl");
    for expected in [
        "player_skin_styles.styles[owner_id].y > 0.5",
        "player_skin_styles.styles[owner_id].z > 0.5",
        "f32(terrain_byte & 0x1Fu) >= globals.shader_params[__MAP_LAND_HIGHLANDS_LIMIT__].x",
        "__MAP_CONQUEST_MOUNTAIN_COLOR__",
    ] {
        assert!(source.contains(expected), "map shader lost hero flash tint: {expected}");
    }
}

#[test]
fn text_shader_parses_and_matches_globals_layout() {
    let source =
        crate::shader_params::prepare_shader("TEXT_", include_str!("shaders/text_glow.wgsl"));
    let module = parse_and_validate(&source);
    assert_struct_size(
        &module,
        "TextGlobals",
        std::mem::size_of::<crate::TextGlobals>(),
    );
}

#[test]
fn avatar_sprite_mask_stays_circular_without_an_adjustable_corner_control() {
    let source = include_str!("shaders/text_glow.wgsl");
    assert!(source.contains("rounded_rect_distance(local, 0.5)"));
    assert!(!source.contains("__TEXT_AVATAR_CORNER__"));
}

#[test]
fn rect_shader_uses_local_quad_coordinates_for_coverage() {
    let source = include_str!("shaders/text_glow.wgsl");
    let rect = source
        .split_once("fn shade_rect(in: VertexOutput) -> vec4<f32> {")
        .and_then(|(_, body)| body.split_once("\n}\n\nfn shade_rounded_rect"))
        .map(|(body, _)| body)
        .expect("rectangle shader body is present");
    assert!(rect.contains("fwidth(in.local_uv)"));
    assert!(!rect.contains("in.uv"));
}

#[test]
fn placement_map_keeps_territory_signals_without_a_circular_highlight() {
    let source = include_str!("shaders/map.wgsl");
    let placement = source
        .split_once("if (globals.placement_scale > 0.0) {")
        .and_then(|(_, body)| body.split_once("\n    }\n    return base_color"))
        .map(|(body, _)| body)
        .expect("placement map block is present");
    assert!(placement.contains("if (!own_land)"));
    assert!(placement.contains("boundary"));
    assert!(!placement.contains("spotlight"));
    assert!(!placement.contains("length("));
}

#[test]
fn status_sprite_shader_supports_individual_grayscale_tint() {
    let source = include_str!("shaders/text_glow.wgsl");
    let status = source
        .split_once("fn shade_status_sprite(in: VertexOutput) -> vec4<f32> {")
        .and_then(|(_, body)| body.split_once("\n}\n\n// Anti-aliased filled rectangle"))
        .map(|(body, _)| body)
        .expect("status sprite shader body is present");
    assert!(status.contains("clamp(in.face_dilate, 0.0, 1.0)"));
    assert!(status.contains("mix(tinted, vec3<f32>(luminance)"));
}

#[test]
fn mover_shaders_parse_and_match_globals_layout() {
    let sprite_source =
        crate::shader_params::prepare_shader("MOVER_", include_str!("shaders/mover_sprites.wgsl"));
    let sprites = parse_and_validate(&sprite_source);
    assert_struct_size(
        &sprites,
        "MoverGlobals",
        std::mem::size_of::<crate::MoverGlobals>(),
    );
    let trail_source =
        crate::shader_params::prepare_shader("MOVER_", include_str!("shaders/mover_trails.wgsl"));
    let trails = parse_and_validate(&trail_source);
    assert_struct_size(
        &trails,
        "MoverGlobals",
        std::mem::size_of::<crate::MoverGlobals>(),
    );
}
