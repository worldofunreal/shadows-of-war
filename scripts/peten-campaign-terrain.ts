import { readFileSync } from "node:fs";
import { join } from "node:path";
import mapPreview from "../sow-tools/editors/campaign-editor/map-preview.ts";
import { BITAL_SIZE, mix, noise, smoothstep, type PetenRoster } from "./bital-terrain.ts";

type Ridge = [x: number, y: number, rise: number, radiusX: number, radiusY: number];
type PathSegment = [x1: number, y1: number, x2: number, y2: number, strength: number];
type Stela = [x: number, y: number, angle?: number];
type Terrace = [x: number, y: number, width: number, depth: number, angle: number];

type Scene = {
    episode: number;
    name: string;
    target: string;
    ridges: Ridge[];
    /** Visual-only terrain relief (never alters playable map.bin elevation). */
    visualRidges?: Ridge[];
    woods: number;
    trail: number;
    sacbe?: number;
    soil: [number, number, number];
    extraPaths?: PathSegment[];
    plazas?: [x: number, y: number, radius: number][];
    stelae?: Stela[];
    terraces?: Terrace[];
};

const scenes: Record<string, Scene> = {
    peten_yellow_rabbit: {
        episode: 3, name: "Yellow Rabbit", target: "tikal_at_yellow_rabbit",
        ridges: [[62, 41, 21, 20, 11], [91, 44, 19, 17, 12], [135, 65, 17, 14, 24], [81, 100, 18, 19, 10], [143, 113, 20, 17, 19]],
        woods: 1.02, trail: 0.92, sacbe: 0.28, soil: [0.55, 0.39, 0.22],
        plazas: [[106, 76, 5.5]],
        extraPaths: [[106, 76, 84, 28, 0.35], [106, 76, 68, 116, 0.35]],
        stelae: [[96, 58, -0.32], [93, 89, 0.26]],
    },
    peten_komkom: {
        episode: 4, name: "Komkom", target: "komkom",
        ridges: [[89, 56, 15, 24, 10], [149, 59, 17, 27, 13], [144, 105, 16, 21, 14], [180, 80, 16, 22, 27], [81, 112, 13, 24, 21]],
        woods: 0.88, trail: 1.05, sacbe: 0.78, soil: [0.54, 0.37, 0.20],
        plazas: [[106, 76, 8.0], [128, 84, 5.2]],
        extraPaths: [[106, 76, 84, 28, 0.32], [106, 76, 68, 116, 0.32]],
        stelae: [[113, 70, 0.18]],
    },
    peten_bahlam_jol: {
        episode: 5, name: "Bahlam Jol", target: "bahlam_jol",
        ridges: [[80, 49, 15, 23, 14], [94, 97, 17, 15, 13], [115, 96, 17, 19, 12], [140, 89, 18, 20, 12], [154, 46, 18, 20, 14]],
        // Subtle low visual saddle dividing Naranjo's rear from the eastern front.
        visualRidges: [[114, 57, 7.5, 8, 11], [120, 82, 7.0, 9, 10]],
        woods: 0.94, trail: 0.72, sacbe: 0.18, soil: [0.53, 0.40, 0.23],
        plazas: [[106, 76, 5.0]],
        extraPaths: [[106, 76, 68, 116, 0.30]],
    },
    peten_ucanal: {
        episode: 6, name: "Ucanal", target: "ucanal",
        ridges: [[72, 79, 23, 17, 16], [72, 99, 23, 16, 16], [125, 101, 24, 13, 22], [139, 112, 24, 16, 20], [113, 133, 22, 15, 18], [165, 71, 18, 23, 20]],
        woods: 1.06, trail: 0.96, sacbe: 0.36, soil: [0.56, 0.39, 0.21],
        plazas: [[104, 80, 5.5], [98, 104, 6.2]],
        extraPaths: [[106, 76, 84, 28, 0.28], [106, 76, 68, 116, 0.32]],
        terraces: [[90, 112, 5.6, 2.2, 0.32], [108, 110, 5.2, 2.1, -0.40]],
    },
    peten_dos_pilas: {
        episode: 7, name: "Dos Pilas", target: "dos_pilas",
        ridges: [[56, 78, 18, 16, 24], [62, 111, 20, 16, 24], [96, 117, 15, 20, 18], [136, 99, 18, 19, 20], [123, 51, 18, 22, 14]],
        woods: 0.92, trail: 0.98, sacbe: 0.52, soil: [0.51, 0.37, 0.21],
        plazas: [[106, 76, 5.5], [80, 94, 5.8], [80, 80, 4.8]],
        extraPaths: [[106, 76, 80, 80, 0.72], [80, 94, 80, 80, 0.45], [106, 76, 84, 28, 0.34]],
        stelae: [[80, 87, 0.12]],
    },
    peten_tikal: {
        episode: 8, name: "Tikal", target: "tikal",
        ridges: [[48, 50, 22, 15, 22], [55, 86, 24, 16, 15], [85, 98, 24, 20, 13], [145, 103, 20, 26, 19], [134, 61, 22, 18, 16], [164, 126, 22, 22, 16]],
        woods: 1.00, trail: 0.95, sacbe: 0.85, soil: [0.55, 0.40, 0.23],
        plazas: [[102, 79, 5.0], [110, 73, 5.0], [68, 64, 10.5]],
        extraPaths: [[106, 76, 84, 28, 0.36], [106, 76, 68, 116, 0.36]],
        stelae: [[87, 62, -0.22], [84, 77, 0.24]],
        terraces: [[64, 52, 7.8, 2.8, -0.18], [56, 62, 2.8, 7.2, 0.14], [75, 54, 5.6, 2.4, 0.32]],
    },
};

const crop = [518, 828] as const;

export function campaignScene(map: string): Scene | undefined {
    return Object.hasOwn(scenes, map) ? scenes[map] : undefined;
}

function segmentDistance(x: number, y: number, x1: number, y1: number, x2: number, y2: number) {
    const dx = x2 - x1;
    const dy = y2 - y1;
    const len2 = dx * dx + dy * dy;
    if (!len2) return Math.hypot(x - x1, y - y1);
    const t = Math.max(0, Math.min(1, ((x - x1) * dx + (y - y1) * dy) / len2));
    return Math.hypot(x - x1 - t * dx, y - y1 - t * dy);
}

export function campaignTerrain(roster: PetenRoster, scene: Scene) {
    const raw = readFileSync(join(import.meta.dir, "../assets/maps/yucatan_peninsula/map.bin"));
    const source = mapPreview.parse(Uint8Array.from(raw).buffer);
    const [width, height] = BITAL_SIZE;
    if (crop[0] + width > source.width || crop[1] + height > source.height) {
        throw new Error("The Yucatán source does not contain the campaign crop");
    }
    const terrain = (x: number, y: number) => {
        x = Math.max(0, Math.min(width - 1, x));
        y = Math.max(0, Math.min(height - 1, y));
        return source.terrain[(crop[1] + y) * source.width + crop[0] + x];
    };
    const mag = (x: number, y: number) => {
        const tile = terrain(x, y);
        return tile & 128 ? tile & 31 : 0;
    };
    const weights = Array.from({ length: 21 }, (_, i) => Math.exp(-((i - 10) ** 2) / 36));
    const weightSum = weights.reduce((sum, weight) => sum + weight, 0);
    const horizontal = Float32Array.from({ length: width * height }, (_, i) =>
        weights.reduce((sum, weight, j) => sum + weight * mag(i % width + j - 10, Math.floor(i / width)), 0) / weightSum);
    const smoothed = Float32Array.from({ length: width * height }, (_, i) =>
        weights.reduce((sum, weight, j) => {
            const row = Math.max(0, Math.min(height - 1, Math.floor(i / width) + j - 10));
            return sum + weight * horizontal[row * width + i % width];
        }, 0) / weightSum);
    const visualMag = (x: number, y: number) => smoothed[
        Math.max(0, Math.min(height - 1, y)) * width + Math.max(0, Math.min(width - 1, x))];
    const points = [roster.player_spawn, ...roster.factions.map(f => [f.x, f.y])];
    const [ax, ay] = roster.player_spawn;
    const target = roster.factions.find(f => f.id === scene.target);
    if (!target) throw new Error(`Missing target ${scene.target} in episode ${scene.episode}`);
    const dx = target.x - ax;
    const dy = target.y - ay;
    const lengthSquared = dx * dx + dy * dy;
    if (!lengthSquared) throw new Error("The player and lesson target share an anchor");
    const salt = scene.episode * 19;

    return (x: number, y: number) => {
        const row = Math.floor(y), col = Math.floor(x);
        const land = (terrain(col, row) & 128) !== 0;
        // Sample the crop at tile centres, with interpolation for the visible mesh.
        const cx = Math.floor(x - 0.5), cy = Math.floor(y - 0.5);
        const u = x - 0.5 - cx, v = y - 0.5 - cy;
        const base = mix(mix(mag(cx, cy), mag(cx + 1, cy), u), mix(mag(cx, cy + 1), mag(cx + 1, cy + 1), u), v);
        const smoothBase = mix(mix(visualMag(cx, cy), visualMag(cx + 1, cy), u), mix(visualMag(cx, cy + 1), visualMag(cx + 1, cy + 1), u), v);
        const progress = Math.max(0, Math.min(1, ((x - ax) * dx + (y - ay) * dy) / lengthSquared));
        const lane = Math.hypot(x - ax - progress * dx, y - ay - progress * dy);
        const nearest = Math.min(...points.map(([px, py]) => Math.hypot(x - px, y - py)));
        const broad = noise(x / 13 + salt, y / 13 + 8);
        const fine = noise(x / 3 + 17, y / 3 + salt);
        const warpX = x + (broad - 0.5) * 7;
        const warpY = y + (noise(x / 16 + salt, y / 16 + 34) - 0.5) * 9;
        let ridge = 0;
        for (const [px, py, rise, rx, ry] of scene.ridges) {
            const distance = ((warpX - px) / rx) ** 2 + ((warpY - py) / ry) ** 2;
            ridge = Math.max(ridge, rise * Math.exp(-1.8 * distance));
        }
        // The extra half tile keeps all integer centres on the protected side of the blend.
        // Do NOT modify `elevation` or `land`: they define the playable map.bin heightmap.
        const relief = smoothstep(12.75, 20, nearest) * smoothstep(6.75, 13, lane);
        const heightGain = relief * Math.max(0, ridge + (fine - 0.5) * 2.2);
        const elevation = Math.min(30, base + heightGain);

        // Visual-only secondary ridges (e.g. Bahlam Jol's low tactical divider) stay clear of bases.
        let visualRidge = 0;
        for (const [px, py, rise, rx, ry] of scene.visualRidges ?? []) {
            const distance = ((warpX - px) / rx) ** 2 + ((warpY - py) / ry) ** 2;
            visualRidge = Math.max(visualRidge, rise * Math.exp(-1.9 * distance));
        }
        const visualRidgeMask = smoothstep(7.5, 12.5, nearest);
        // Smooth the Yucatán source steps for lighting, retaining its exact gameplay heights.
        const visualHeight = Math.min(30, smoothBase + heightGain + visualRidge * visualRidgeMask);

        const mainTrail = scene.trail * (1 - smoothstep(0.8, 3.8, lane + (fine - 0.5) * 1.5));
        let secondaryTrail = 0;
        for (const [x1, y1, x2, y2, strength] of scene.extraPaths ?? []) {
            const d = segmentDistance(x, y, x1, y1, x2, y2);
            secondaryTrail = Math.max(secondaryTrail, strength * (1 - smoothstep(0.7, 3.2, d + (fine - 0.5) * 1.4)));
        }
        let plaza = 0;
        let plazaStone = 0;
        for (const [px, py, size] of scene.plazas ?? []) {
            const d = Math.hypot(x - px, y - py);
            plaza = Math.max(plaza, 1 - smoothstep(size - 1.2, size + 2.0, d));
            plazaStone = Math.max(plazaStone, 1 - smoothstep(size * 0.55, size * 0.92, d + (fine - 0.5) * 0.7));
        }
        const sacbeRoad = (scene.sacbe ?? 0)
            * (1 - smoothstep(0.65, 2.2, lane + (fine - 0.5) * 0.55))
            * smoothstep(3.5, 7.5, nearest);
        const stone = Math.min(1, Math.max(plazaStone * 0.85, sacbeRoad * 0.92));

        // Keep woodland density in 0..1 like B'ital so groves are lush, while preserving clearings & routes.
        const woodland = 0.72 * broad + 0.28 * noise(x / 6 + 72, y / 6 + salt);
        let stelaClear = 1;
        for (const [sx, sy] of scene.stelae ?? []) {
            stelaClear = Math.min(stelaClear, smoothstep(2.6, 5.2, Math.hypot(x - sx, y - sy)));
        }
        let terraceClear = 1;
        for (const [tx, ty, tw, td] of scene.terraces ?? []) {
            terraceClear = Math.min(terraceClear, smoothstep(Math.max(tw, td) * 0.7 + 1.5, Math.max(tw, td) * 0.7 + 4.5, Math.hypot(x - tx, y - ty)));
        }
        const baseDensity = smoothstep(0.18, 0.68, woodland)
            * smoothstep(9.0, 14.0, nearest)
            * smoothstep(4.2, 8.8, lane)
            * (1 - smoothstep(18.5, 25.5, visualHeight))
            * (1 - plaza * 0.95)
            * (1 - secondaryTrail * 0.75)
            * stelaClear
            * terraceClear;

        // Lush dark mossy jungle floor follows woodland coverage.
        const canopy = land ? Math.min(1, baseDensity * 1.15 * (0.75 + 0.25 * fine)) : 0;
        const rock = smoothstep(15.2, 26.2, visualHeight + (fine - 0.5) * 4.2);
        const boulder = land
            ? smoothstep(11.5, 17.0, visualHeight)
              * (1 - smoothstep(21.0, 26.0, visualHeight))
              * smoothstep(9.0, 14.0, nearest)
              * smoothstep(4.5, 8.5, lane)
              * (1 - plaza)
              * stelaClear
              * terraceClear
            : 0;

        // Soft sandy bank on land bordering the bottom-right water corner.
        let nearWater = 0;
        if (land) {
            for (const [ox, oy] of [[-2, 0], [2, 0], [0, -2], [0, 2], [3, 3], [0, 4], [4, 0]]) {
                if ((terrain(col + ox, row + oy) & 128) === 0) {
                    nearWater = 1;
                    break;
                }
            }
        }
        const waterDip = land ? 0 : -1.2;

        return {
            land, elevation,
            height: (visualHeight - 8) * 0.54 + Math.max(0, visualHeight - 15.5) ** 2 * 0.048 + waterDip,
            rock,
            trail: Math.min(1, Math.max(mainTrail, secondaryTrail, plaza * 0.65) * (0.64 + fine * 0.36)),
            stone,
            canopy,
            bank: nearWater * (0.7 + 0.3 * fine),
            boulder,
            density: land ? baseDensity : 0,
            woodsScale: scene.woods,
        };
    };
}
