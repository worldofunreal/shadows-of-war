//! Headless WGSL validation. Blade parses and validates every shader with
//! naga at pipeline creation — the exact same pass runs on the web build at
//! runtime. These tests run it under `cargo test` so a broken shader fails
//! the build instead of the running game.

const MAP_SHADER: &str = include_str!("shaders/map.wgsl");
const TEXT_SHADER: &str = include_str!("shaders/text_glow.wgsl");
const MOVER_SPRITES_SHADER: &str = include_str!("shaders/mover_sprites.wgsl");
const MOVER_TRAILS_SHADER: &str = include_str!("shaders/mover_trails.wgsl");

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
    let module = parse_and_validate(MAP_SHADER);
    assert_struct_size(&module, "Globals", std::mem::size_of::<crate::MapGlobals>());
}

#[test]
fn text_shader_parses_and_matches_globals_layout() {
    let module = parse_and_validate(TEXT_SHADER);
    assert_struct_size(
        &module,
        "TextGlobals",
        std::mem::size_of::<crate::TextGlobals>(),
    );
}

#[test]
fn mover_shaders_parse_and_match_globals_layout() {
    let sprites = parse_and_validate(MOVER_SPRITES_SHADER);
    assert_struct_size(
        &sprites,
        "MoverGlobals",
        std::mem::size_of::<crate::MoverGlobals>(),
    );
    let trails = parse_and_validate(MOVER_TRAILS_SHADER);
    assert_struct_size(
        &trails,
        "MoverGlobals",
        std::mem::size_of::<crate::MoverGlobals>(),
    );
}
