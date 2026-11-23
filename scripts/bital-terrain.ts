export type PetenRoster = {
    player_spawn: [number, number];
    factions: { id: string; x: number; y: number }[];
};

export const BITAL_SIZE = [224, 156] as const;

const peaks = [
    [41, 64, 16, 29, 17], [61, 51, 19, 16, 15], [32, 117, 13, 18, 15],
    [113, 43, 15, 22, 16], [131, 39, 22, 15, 15], [148, 44, 20, 19, 16],
    [165, 55, 22, 20, 16], [180, 69, 17, 18, 15], [205, 41, 19, 25, 22],
    [112, 114, 16, 21, 16], [133, 112, 20, 16, 17], [151, 120, 23, 19, 18],
    [170, 116, 22, 19, 19], [188, 105, 16, 22, 15], [204, 142, 17, 25, 24],
];

function smoothstep(low: number, high: number, value: number) {
    const t = Math.min(1, Math.max(0, (value - low) / (high - low)));
    return t * t * (3 - 2 * t);
}

function mix(a: number, b: number, t: number) {
    return a + (b - a) * t;
}

function noise(x: number, y: number) {
    const ix = Math.floor(x);
    const iy = Math.floor(y);
    const u = smoothstep(0, 1, x - ix);
    const v = smoothstep(0, 1, y - iy);
    const lattice = (a: number, b: number) => {
        const h = Math.imul(a, 0x1e35a7bd) ^ Math.imul(b, 0x94d049bb);
        return (Math.imul(h ^ (h >>> 15), 0x85ebca6b) >>> 0) / 4294967296;
    };
    const north = mix(lattice(ix, iy), lattice(ix + 1, iy), u);
    const south = mix(lattice(ix, iy + 1), lattice(ix + 1, iy + 1), u);
    return mix(north, south, v);
}

export function bitalTerrain(roster: PetenRoster) {
    const [startX, startY] = roster.player_spawn;
    const opponent = roster.factions.find(faction => faction.id === "bital");
    if (!opponent) throw new Error("The pass requires B’ital's starting position");
    const dx = opponent.x - startX;
    const dy = opponent.y - startY;
    const passLengthSquared = dx * dx + dy * dy;
    if (passLengthSquared === 0) throw new Error("Naranjo and B’ital must occupy separate tiles");
    // Preserve the elevations measured around the four seats and along the march on Yucatán.
    const settlements = [
        { x: startX, y: startY, height: 12 },
        ...roster.factions.map(faction => ({
            x: faction.x,
            y: faction.y,
            height: faction.id === "calakmul" ? 7 : faction.id === "dos_pilas" ? 11 : 12,
        })),
    ];
    return (x: number, y: number) => {
        const along = Math.max(0, Math.min(1, ((x - startX) * dx + (y - startY) * dy) / passLengthSquared));
        const passDistance = Math.hypot(x - (startX + dx * along), y - (startY + dy * along));
        const broad = noise(x / 15 + 7, y / 15 + 3);
        const detail = noise(x / 3 + 25, y / 3 + 17);
        const warpedX = x + 8 * (broad - 0.5) + 2 * (detail - 0.5);
        const warpedY = y + 10 * (noise(x / 17 + 9, y / 17 + 32) - 0.5);
        let ridge = 0;
        for (const [cx, cy, rise, rx, ry] of peaks) {
            const distance = Math.hypot((warpedX - cx) / rx, (warpedY - cy) / ry);
            ridge = Math.max(ridge, rise * Math.exp(-1.95 * distance * distance));
        }
        let elevation = 7.5 + 3 * broad + ridge + smoothstep(2, 16, ridge) * (detail - 0.5) * 3.6;
        elevation = mix(12, elevation, smoothstep(10, 20, passDistance));
        let nearestSettlement = Infinity;
        for (const settlement of settlements) {
            const distance = Math.hypot(x - settlement.x, y - settlement.y);
            nearestSettlement = Math.min(nearestSettlement, distance);
            elevation = mix(settlement.height, elevation, smoothstep(8, 15, distance));
        }
        elevation = Math.min(30, elevation);
        const trail = (1 - smoothstep(1.0, 3.8, passDistance + (detail - 0.5) * 1.8)) * (0.56 + 0.44 * detail);
        const woodland = 0.75 * broad + 0.25 * noise(x / 6 + 72, y / 6 + 13);
        const density = smoothstep(0.20, 0.72, woodland)
            * smoothstep(9, 13, nearestSettlement)
            * smoothstep(4.5, 9, passDistance)
            * (1 - smoothstep(19, 26, elevation));
        return {
            elevation,
            height: (elevation - 8) * 0.55 + Math.max(0, elevation - 15) ** 2 * 0.043,
            rock: smoothstep(16, 27, elevation + (detail - 0.5) * 4.5),
            trail,
            density,
        };
    };
}
