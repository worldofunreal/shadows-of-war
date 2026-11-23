#!/usr/bin/env bun
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { BITAL_SIZE, bitalTerrain, type PetenRoster } from "./bital-terrain.ts";
import { campaignScene, campaignTerrain } from "./peten-campaign-terrain.ts";

type Point = [number, number, number];
type Mesh = { vertices: Point[]; faces: number[][]; colors: Point[]; materials: number[] };

const map = process.argv[2] ?? "peten_clearing";
const chapter = campaignScene(map);
if (map !== "peten_clearing" && map !== "bital_pass" && !chapter) {
    throw new Error(`Unknown Petén map: ${map}`);
}
const bital = map === "bital_pass";
const sourceDir = join(import.meta.dir, "../assets/map_sources", map);
const maskPath = join(sourceDir, "image.png");
const episode = chapter?.episode ?? (bital ? 2 : 1);
const roster: PetenRoster = JSON.parse(readFileSync(join(import.meta.dir, "../assets/campaign", `six_sky_maya_0${episode}.json`), "utf8"));
const [width, height] = bital || chapter ? BITAL_SIZE : (() => {
    const mask = readFileSync(maskPath);
    return [mask.readUInt32BE(16), mask.readUInt32BE(20)] as const;
})();
const resolution = [width * 8, height * 8] as const;
const [playerX, playerY] = roster.player_spawn;
const opponentId = chapter?.target ?? (bital ? "bital" : "kinichil_kab");
const opponent = roster.factions.find(({ id }) => id === opponentId);
if (!opponent) throw new Error(`The campaign roster must include ${opponentId}`);
const clearingX = (playerX + opponent.x) / 2;
const clearingY = (playerY + opponent.y) / 2;
const clearingRadiusX = Math.abs(playerX - opponent.x) / 2 + 12;
const sampleTerrain = chapter ? campaignTerrain(roster, chapter) : bital ? bitalTerrain(roster) : null;
let seed = chapter ? 0x70657465 + episode : bital ? 0x62697461 : 0x70657465;
function random() {
    seed ^= seed << 13;
    seed ^= seed >>> 17;
    seed ^= seed << 5;
    return (seed >>> 0) / 4294967296;
}

function addPart(mesh: Mesh, vertices: Point[], faces: number[][], color: Point) {
    const base = mesh.vertices.length;
    const material = mesh.colors.push(color) - 1;
    mesh.vertices.push(...vertices);
    for (const face of faces) {
        mesh.faces.push(face.map(index => index + base));
        mesh.materials.push(material);
    }
}

function branch(mesh: Mesh, start: Point, end: Point, base: number, tip: number) {
    const heading = Math.atan2(end[1] - start[1], end[0] - start[0]);
    const tilt = Math.atan2(Math.hypot(end[0] - start[0], end[1] - start[1]), end[2] - start[2]);
    const vertices: Point[] = [];
    const faces: number[][] = [];
    const sides = 6;
    for (const [point, radius] of [[start, base], [end, tip]] as const) {
        for (let i = 0; i < sides; i++) {
            const angle = i * Math.PI * 2 / sides;
            const u = Math.cos(angle) * radius;
            const v = Math.sin(angle) * radius;
            vertices.push([
                point[0] + u * Math.cos(heading) * Math.cos(tilt) - v * Math.sin(heading),
                point[1] + u * Math.sin(heading) * Math.cos(tilt) + v * Math.cos(heading),
                point[2] - u * Math.sin(tilt),
            ]);
        }
    }
    for (let i = 0; i < sides; i++) {
        const next = (i + 1) % sides;
        faces.push([i, next, next + sides, i + sides]);
    }
    faces.push(Array.from({ length: sides }, (_, i) => i + sides));
    addPart(mesh, vertices, faces, [0.24, 0.17, 0.09]);
}

function crown(mesh: Mesh, center: Point, radius: number, depth: number, color: Point) {
    const sides = 9;
    const vertices: Point[] = [[center[0], center[1], center[2] - depth]];
    const faces: number[][] = [];
    for (let ring = 1; ring <= 3; ring++) {
        const latitude = (ring - 2) * Math.PI / 4;
        for (let i = 0; i < sides; i++) {
            const angle = (i + 0.3 * ring) * 2 * Math.PI / sides;
            const r = radius * Math.cos(latitude) * (0.86 + 0.25 * random());
            vertices.push([
                center[0] + Math.cos(angle) * r,
                center[1] + Math.sin(angle) * r,
                center[2] + depth * (Math.sin(latitude) + 0.09 * (random() - 0.5)),
            ]);
        }
    }
    const top = vertices.push([center[0], center[1], center[2] + depth]) - 1;
    for (let i = 0; i < sides; i++) {
        const next = (i + 1) % sides;
        faces.push([0, next + 1, i + 1], [top, 1 + sides * 2 + i, 1 + sides * 2 + next]);
        for (let ring = 0; ring < 2; ring++) {
            const a = 1 + ring * sides;
            faces.push([a + i, a + next, a + sides + next], [a + i, a + sides + next, a + sides + i]);
        }
    }
    addPart(mesh, vertices, faces, color);
}

function tree(height: number, spread: number, branches: number, foliage: Point): Mesh {
    const mesh: Mesh = { vertices: [], faces: [], colors: [], materials: [] };
    branch(mesh, [0, 0, -0.04], [0.1, 0, height * 0.94], 0.15, 0.04);
    crown(mesh, [0.1, 0, height], spread * 0.73, spread * 0.47, foliage);
    for (let i = 0; i < branches; i++) {
        const angle = (i + random() * 0.4) * Math.PI * 2 / branches;
        const radius = spread * (0.45 + 0.23 * random());
        const end: Point = [Math.cos(angle) * radius, Math.sin(angle) * radius, height * (0.70 + 0.22 * random())];
        branch(mesh, [0.05, 0, height * 0.48], end, 0.079, 0.022);
        const tint = 0.78 + 0.27 * random();
        crown(mesh, end, spread * 0.57, spread * 0.38, [foliage[0] * tint, foliage[1] * tint, foliage[2] * tint]);
    }
    return mesh;
}

const trees = [
    tree(2.7, 1.34, 5, [0.22, 0.42, 0.13]),
    tree(3.5, 1.04, 4, [0.14, 0.32, 0.15]),
    tree(1.8, 0.93, 3, [0.31, 0.46, 0.14]),
];

// Broad groves leave open land along the path between the two starting positions.
const groves = [
    [23, 29, 15, 12], [42, 13, 15, 8], [55, 33, 12, 10], [69, 14, 15, 8],
    [89, 22, 13, 9], [118, 19, 19, 10], [138, 36, 13, 11], [159, 21, 17, 12],
    [32, 54, 13, 13], [15, 71, 10, 14], [48, 79, 16, 10], [72, 74, 15, 10],
    [99, 80, 16, 10], [117, 64, 13, 10], [139, 80, 17, 10], [160, 65, 12, 16],
];
const instances: number[][] = [];
for (let row = 0; row < height; row += 1.65) {
    for (let col = 0; col < width; col += 1.65) {
        const x = col + random() * 1.45;
        const y = row + random() * 1.45;
        const terrain = sampleTerrain?.(x, y);
        if (terrain) {
            if (random() > 0.54 * terrain.density ** 1.55) continue;
        } else {
            const clearing = Math.hypot((x - clearingX) / clearingRadiusX, (y - clearingY) / 11);
            if (clearing < 1.15 + 0.15 * random()) continue;
            const density = Math.max(...groves.map(([cx, cy, rx, ry]) => {
                const distance = Math.hypot((x - cx) / rx, (y - cy) / ry);
                return Math.exp(-1.9 * distance * distance);
            }));
            if (random() > 0.45 * density ** 1.55) continue;
        }
        instances.push([
            Math.floor(random() * trees.length), x, height - y,
            0.65 + random() * 0.67, random() * Math.PI * 2, terrain?.height ?? 0,
        ]);
    }
}
if (new Set(instances.map(([variant]) => variant)).size !== trees.length) {
    throw new Error("Petén bake must contain all three tree variants");
}

// Blender does the lighting and rasterization. The meshes and layout above are the source art.
const renderScene = String.raw`
import bpy, json, math, sys
from mathutils import Vector

scene_file, output = sys.argv[sys.argv.index('--') + 1:]
with open(scene_file, encoding='utf-8') as source:
    data = json.load(source)
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.render.engine = 'CYCLES'
scene.cycles.device = 'CPU'
scene.cycles.samples = data['samples']
scene.cycles.seed = 407
scene.cycles.use_denoising = True
scene.render.resolution_x, scene.render.resolution_y = data['resolution']
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = 'PNG'
scene.render.image_settings.color_mode = 'RGB'
scene.render.filepath = output
scene.view_settings.view_transform = 'Standard'
width, height = data['size']

def material(name, srgb):
    result = bpy.data.materials.new(name)
    result.use_nodes = True
    linear = [((c + 0.055) / 1.055) ** 2.4 if c > 0.04045 else c / 12.92 for c in srgb]
    result.node_tree.nodes.get('Principled BSDF').inputs['Base Color'].default_value = (*linear, 1)
    return result

ground = material('Petén ground', (0.63, 0.72, 0.38))
nodes = ground.node_tree.nodes
links = ground.node_tree.links
coord = nodes.new('ShaderNodeTexCoord')
noise = nodes.new('ShaderNodeTexNoise')
noise.inputs['Scale'].default_value = 10
noise.inputs['Detail'].default_value = 3
links.new(coord.outputs['Generated'], noise.inputs['Vector'])
palette = nodes.new('ShaderNodeValToRGB')
palette.color_ramp.elements.remove(palette.color_ramp.elements[1])
palette.color_ramp.elements[0].position = 0.16
palette.color_ramp.elements[0].color = (0.24, 0.37, 0.10, 1)
palette.color_ramp.elements.new(0.54).color = (0.36, 0.46, 0.15, 1)
palette.color_ramp.elements.new(0.78).color = (0.49, 0.48, 0.24, 1)
links.new(noise.outputs['Fac'], palette.inputs[0])
ground_color = palette.outputs[0]
plane = bpy.data.meshes.new('Ground')
surface = data['surface']
if surface:
    stride = width + 1
    vertices = [(i % stride, height - i // stride, sample[0]) for i, sample in enumerate(surface)]
    faces = [(i, i+stride, i+stride+1, i+1) for y in range(height) for i in range(y*stride, y*stride+width)]
    plane.from_pydata(vertices, [], faces)
    plane.polygons.foreach_set('use_smooth', [True] * len(faces))

    def tint(base, attribute_name, values, color):
        weights = plane.attributes.new(attribute_name, 'FLOAT', 'POINT')
        weights.data.foreach_set('value', values)
        attribute = nodes.new('ShaderNodeAttribute')
        attribute.attribute_name = attribute_name
        blend = nodes.new('ShaderNodeMixRGB')
        links.new(attribute.outputs['Fac'], blend.inputs[0])
        links.new(base, blend.inputs[1])
        if isinstance(color, tuple):
            blend.inputs[2].default_value = color
        else:
            links.new(color, blend.inputs[2])
        return blend.outputs[0]

    stone_noise = nodes.new('ShaderNodeTexNoise')
    stone_noise.inputs['Scale'].default_value = 0.95
    stone_noise.inputs['Detail'].default_value = 4
    links.new(coord.outputs['Object'], stone_noise.inputs['Vector'])
    bump = nodes.new('ShaderNodeBump')
    bump.inputs['Strength'].default_value = 0.24
    bump.inputs['Distance'].default_value = 0.36
    links.new(stone_noise.outputs['Fac'], bump.inputs['Height'])
    links.new(bump.outputs['Normal'], nodes.get('Principled BSDF').inputs['Normal'])

    layers = nodes.new('ShaderNodeTexWave')
    layers.bands_direction = 'Z'
    layers.inputs['Scale'].default_value = 0.20
    layers.inputs['Distortion'].default_value = 2.0
    layers.inputs['Detail Scale'].default_value = 2.3
    links.new(coord.outputs['Object'], layers.inputs['Vector'])
    weathering = nodes.new('ShaderNodeMixRGB')
    weathering.inputs[0].default_value = 0.66
    links.new(layers.outputs['Fac'], weathering.inputs[1])
    links.new(stone_noise.outputs['Fac'], weathering.inputs[2])
    limestone = nodes.new('ShaderNodeValToRGB')
    limestone.color_ramp.elements[0].color = (0.35, 0.30, 0.22, 1)
    limestone.color_ramp.elements[1].color = (0.76, 0.68, 0.54, 1)
    links.new(weathering.outputs[0], limestone.inputs['Fac'])
    ground_color = tint(ground_color, 'Limestone', [s[1] for s in surface], limestone.outputs[0])
    ground_color = tint(ground_color, 'Worn ground', [s[2] * 0.61 for s in surface], (*data['soil'], 1))
else:
    plane.from_pydata([(-10, -10, 0), (width+10, -10, 0), (width+10, height+10, 0), (-10, height+10, 0)], [], [(0, 1, 2, 3)])
links.new(ground_color, nodes.get('Principled BSDF').inputs['Base Color'])
plane.materials.append(ground)
scene.collection.objects.link(bpy.data.objects.new('Ground', plane))

meshes = []
for index, tree in enumerate(data['trees']):
    mesh = bpy.data.meshes.new(f'Tree {index + 1}')
    mesh.from_pydata(tree['vertices'], [], tree['faces'])
    for color in tree['colors']:
        mesh.materials.append(material('Bark or foliage', color))
    for polygon, material_index in zip(mesh.polygons, tree['materials']):
        polygon.material_index = material_index
        polygon.use_smooth = tree['colors'][material_index][1] > 0.2
    meshes.append(mesh)
for variant, x, y, scale, rotation, z in data['instances']:
    obj = bpy.data.objects.new('Tree', meshes[variant])
    scene.collection.objects.link(obj)
    obj.location = (x, y, z)
    obj.scale = (scale, scale, scale)
    obj.rotation_euler.z = rotation

for x, y, z in data['stelae']:
    stone = material('Carved limestone', (0.64, 0.60, 0.48))
    bpy.ops.mesh.primitive_cube_add(size=2, location=(x, y, z + 1.1), rotation=(0, 0, -0.25))
    stela = bpy.context.object
    stela.scale = (0.68, 0.34, 1.22)
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    bevel = stela.modifiers.new('Rounded edges', 'BEVEL')
    bevel.width = 0.14
    bevel.segments = 2
    stela.data.materials.append(stone)

world = bpy.data.worlds.new('Skylight')
scene.world = world
world.use_nodes = True
sky = world.node_tree.nodes.get('Background')
sky.inputs['Color'].default_value = (0.74, 0.82, 1.0, 1)
sky.inputs['Strength'].default_value = 0.56
sun_data = bpy.data.lights.new('Sun', 'SUN')
sun_data.energy = 2.15
sun_data.color = (1.0, 0.90, 0.72)
sun_data.angle = math.radians(6)
sun = bpy.data.objects.new('Sun', sun_data)
scene.collection.objects.link(sun)
sun.rotation_euler = Vector((0.57, -0.48, -0.67)).to_track_quat('-Z', 'Y').to_euler()
camera_data = bpy.data.cameras.new('Overhead')
camera_data.type = 'ORTHO'
camera_data.ortho_scale = width
camera = bpy.data.objects.new('Overhead', camera_data)
scene.collection.objects.link(camera)
camera.location = (width / 2, height / 2, 100)
scene.camera = camera
bpy.ops.render.render(write_still=True)
`;

const directory = mkdtempSync(join(tmpdir(), "sow-peten-"));
try {
    const scene = join(directory, "scene.json");
    const renderScript = join(directory, "render.py");
    const render = join(directory, "render.png");
    const surface = sampleTerrain && Array.from({ length: (width + 1) * (height + 1) }, (_, i) => {
        const tile = sampleTerrain(i % (width + 1), Math.floor(i / (width + 1)));
        return [tile.height, tile.rock, tile.trail];
    });
    const stelae = chapter?.stelae?.map(([x, y]) => [x, height - y, sampleTerrain?.(x, y).height ?? 0]) ?? [];
    const soil = chapter?.soil ?? [0.56, 0.37, 0.17];
    const samples = chapter ? 16 : 24;
    writeFileSync(scene, JSON.stringify({ size: [width, height], resolution, trees, instances, surface, stelae, soil, samples }));
    const threads = process.env.PETEN_RENDER_THREADS ?? "8";
    writeFileSync(renderScript, renderScene);
    const blender = Bun.spawnSync([
        "blender", "-b", "--factory-startup", "-t", threads, "--python-exit-code", "1",
        "--python", renderScript, "--", scene, render,
    ], { stdout: "inherit", stderr: "inherit" });
    if (!blender.success) throw new Error(`Blender exited with ${blender.exitCode}`);
    const png = readFileSync(render);
    if (png.subarray(1, 4).toString() !== "PNG" || png.readUInt32BE(16) !== resolution[0] || png.readUInt32BE(20) !== resolution[1]) {
        throw new Error(`Blender output must be a ${resolution[0]} × ${resolution[1]} PNG`);
    }
    const mapDir = join(import.meta.dir, "../assets/maps", map);
    const output = chapter ? join(mapDir, "terrain.webp")
        : join(import.meta.dir, "../sow-client/src/render/frame", bital ? "bital-pass.webp" : "peten-canopy.png");
    if (sampleTerrain) {
        // The map importer reads one terrain tile per pixel and decodes its magnitude from blue.
        const pixels = new Uint8Array(width * height * 3);
        for (let i = 0; i < width * height; i++) {
            const tile = sampleTerrain(i % width + 0.5, Math.floor(i / width) + 0.5);
            const blue = "land" in tile && !tile.land ? 106 : 140 + 2 * Math.round(tile.elevation);
            pixels.set([190, 220, blue], i * 3);
        }
        const mask = join(directory, "image.png");
        const webp = join(directory, "terrain.webp");
        const encodeMask = Bun.spawnSync(["magick", "ppm:-", "-strip", `PNG24:${mask}`], {
            stdin: Buffer.concat([Buffer.from(`P6\n${width} ${height}\n255\n`), pixels]),
            stderr: "inherit",
        });
        if (!encodeMask.success) throw new Error(`Failed to encode the terrain mask (${encodeMask.exitCode})`);
        const encodeArt = Bun.spawnSync(["magick", render, "-strip", "-quality", "92", webp], { stderr: "inherit" });
        if (!encodeArt.success) throw new Error(`Failed to encode the terrain art (${encodeArt.exitCode})`);
        if (chapter) {
            mkdirSync(sourceDir, { recursive: true });
            mkdirSync(mapDir, { recursive: true });
            writeFileSync(join(sourceDir, "info.json"), JSON.stringify({ name: chapter.name, multiplayer_frequency: 0 }, null, 2) + "\n");
        }
        writeFileSync(maskPath, readFileSync(mask));
        writeFileSync(output, readFileSync(webp));
    } else {
        writeFileSync(output, png);
    }
} finally {
    rmSync(directory, { recursive: true, force: true });
}
