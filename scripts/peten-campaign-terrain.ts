import { readFileSync } from "node:fs";
import { join } from "node:path";
import mapPreview from "../sow-tools/editors/campaign-editor/map-preview.ts";
import { BITAL_SIZE, mix, noise, smoothstep, type PetenRoster } from "./bital-terrain.ts";

type Ridge = [x: number, y: number, rise: number, radiusX: number, radiusY: number];
type Scene = {
    episode: number;
    name: string;
    target: string;
    ridges: Ridge[];
    woods: number;
    trail: number;
    soil: [number, number, number];
    plazas?: [x: number, y: number, radius: number][];
    stelae?: [x: number, y: number][];
};

const scenes: Record<string, Scene> = {
    peten_yellow_rabbit: {
        episode: 3, name: "Yellow Rabbit", target: "tikal_at_yellow_rabbit",
        ridges: [[62, 41, 21, 20, 11], [91, 44, 19, 17, 12], [135, 65, 17, 14, 24], [81, 100, 18, 19, 10], [143, 113, 20, 17, 19]],
        woods: 0.52, trail: 0.8, soil: [0.55, 0.39, 0.22], stelae: [[96, 58], [93, 89]],
    },
    peten_komkom: {
        episode: 4, name: "Komkom", target: "komkom",
        ridges: [[89, 56, 15, 24, 10], [149, 59, 17, 27, 13], [144, 105, 16, 21, 14], [180, 80, 16, 22, 27], [81, 112, 13, 24, 21]],
        woods: 0.41, trail: 1.10, soil: [0.52, 0.34, 0.19], plazas: [[106, 76, 8]],
    },
    peten_bahlam_jol: {
        episode: 5, name: "Bahlam Jol", target: "bahlam_jol",
        ridges: [[80, 49, 15, 23, 14], [94, 97, 17, 15, 13], [115, 96, 17, 19, 12], [140, 89, 18, 20, 12], [154, 46, 18, 20, 14]],
        woods: 0.46, trail: 0.52, soil: [0.53, 0.42, 0.24],
    },
    peten_ucanal: {
        episode: 6, name: "Ucanal", target: "ucanal",
        ridges: [[72, 79, 23, 17, 16], [72, 99, 23, 16, 16], [125, 101, 24, 13, 22], [139, 112, 24, 16, 20], [113, 133, 22, 15, 18], [165, 71, 18, 23, 20]],
        woods: 0.55, trail: 0.89, soil: [0.57, 0.40, 0.22], plazas: [[104, 80, 5]],
    },
    peten_dos_pilas: {
        episode: 7, name: "Dos Pilas", target: "dos_pilas",
        ridges: [[56, 78, 18, 16, 24], [62, 111, 20, 16, 24], [96, 117, 15, 20, 18], [136, 99, 18, 19, 20], [123, 51, 18, 22, 14]],
        woods: 0.45, trail: 1.02, soil: [0.49, 0.37, 0.22],
    },
    peten_tikal: {
        episode: 8, name: "Tikal", target: "tikal",
        ridges: [[48, 50, 22, 15, 22], [55, 86, 24, 16, 15], [85, 98, 24, 20, 13], [145, 103, 20, 26, 19], [134, 61, 22, 18, 16], [164, 126, 22, 22, 16]],
        woods: 0.50, trail: 0.89, soil: [0.55, 0.40, 0.24], plazas: [[102, 79, 4.5], [110, 73, 4.5], [68, 64, 10]],
    },
};

const crop = [518, 828] as const;

export function campaignScene(map: string): Scene | undefined {
    return Object.hasOwn(scenes, map) ? scenes[map] : undefined;
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
        const relief = smoothstep(12.75, 20, nearest) * smoothstep(6.75, 13, lane);
        const heightGain = relief * Math.max(0, ridge + (fine - 0.5) * 2.2);
        const elevation = Math.min(30, base + heightGain);
        // Smooth the Yucatán source steps for lighting, retaining its exact gameplay heights.
        const visualHeight = Math.min(30, smoothBase + heightGain);
        const wornPath = scene.trail * (1 - smoothstep(0.9, 3.9, lane + (fine - 0.5) * 1.6));
        let plaza = 0;
        for (const [px, py, size] of scene.plazas ?? []) {
            plaza = Math.max(plaza, 1 - smoothstep(size - 1, size + 2, Math.hypot(x - px, y - py)));
        }
        const woodland = 0.75 * broad + 0.25 * noise(x / 6 + 72, y / 6 + salt);
        const density = scene.woods * smoothstep(0.20, 0.69, woodland)
            * smoothstep(9.5, 14.5, nearest) * smoothstep(4.5, 9, lane)
            * (1 - smoothstep(19, 26, elevation)) * (1 - plaza);
        return {
            land, elevation,
            height: (visualHeight - 8) * 0.52 + Math.max(0, visualHeight - 16) ** 2 * 0.046,
            rock: smoothstep(16, 27, visualHeight + (fine - 0.5) * 4),
            trail: Math.max(wornPath, plaza * 0.72) * (0.62 + fine * 0.38),
            density: land ? density : 0,
        };
    };
}
