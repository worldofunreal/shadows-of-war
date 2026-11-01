#!/usr/bin/env bun
import fs from "node:fs";
import path from "node:path";
import os from "node:os";

export interface DevSettingsPayload {
    version: number;
    exported_at: string;
    changes_count: number;
    changes: Record<string, boolean | number | number[]>;
}

export function formatRustFloat(val: number): string {
    if (!Number.isFinite(val)) return "0.0";
    return Number.isInteger(val) ? `${val}.0` : `${Number(val.toFixed(6))}`;
}

export function formatRustValue(val: boolean | number | number[]): string {
    if (typeof val === "boolean") {
        return val ? "true" : "false";
    }
    if (typeof val === "number") {
        return formatRustFloat(val);
    }
    if (Array.isArray(val)) {
        return "[" + val.map(formatRustFloat).join(", ") + "]";
    }
    return String(val);
}

export function updateShaderParams(
    content: string,
    id: string,
    val: boolean | number | number[],
): { updated: boolean; content: string } {
    const rustVal = formatRustValue(val);
    const escapedId = id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const regex = new RegExp(
        `(\\b\\w+!\\s*\\(\\s*"${escapedId}"\\s*,\\s*"[^"]*"\\s*,\\s*"[^"]*"\\s*,\\s*)(?:\\[[^\\]]+\\]|[^,\\)]+)([\\s\\S]*?\\))`,
    );
    if (!regex.test(content)) {
        return { updated: false, content };
    }
    const updatedContent = content.replace(regex, (_match, prefix, suffix) => {
        return `${prefix}${rustVal}${suffix}`;
    });
    return { updated: true, content: updatedContent };
}

export function updateThemeDefaults(
    content: string,
    id: string,
    val: boolean | number | number[],
): { updated: boolean; content: string } {
    // Only target the DevConfig Default implementation
    const defaultBlockMatch = content.match(
        /impl Default for DevConfig\s*\{[\s\S]*?fn default\(\)\s*->\s*Self\s*\{[\s\S]*?Self\s*\{([\s\S]*?)\}\s*\}/,
    );
    if (!defaultBlockMatch) return { updated: false, content };

    const block = defaultBlockMatch[0];
    const escapedId = id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const fieldRe = new RegExp(`(\\b${escapedId}:\\s*)([^,]+)(,)`);
    if (!fieldRe.test(block)) return { updated: false, content };

    const rustVal = formatRustValue(val);
    const newBlock = block.replace(fieldRe, `$1${rustVal}$3`);
    const updatedContent = content.replace(block, newBlock);
    return { updated: true, content: updatedContent };
}

export function applyDevSettings(
    filePath?: string,
    options?: { dryRun?: boolean; keepFile?: boolean },
): { appliedCount: number; missingKeys: string[]; appliedKeys: string[] } {
    const targetFile = filePath || path.join(os.homedir(), "Downloads", "sow_dev_settings.json");

    if (!fs.existsSync(targetFile)) {
        console.error(`[APPLY DEV SETTINGS] Settings file not found: ${targetFile}`);
        return { appliedCount: 0, missingKeys: [], appliedKeys: [] };
    }

    const raw = fs.readFileSync(targetFile, "utf8");
    let data: DevSettingsPayload;
    try {
        data = JSON.parse(raw);
    } catch (err) {
        console.error(`[APPLY DEV SETTINGS] Invalid JSON in ${targetFile}:`, err);
        return { appliedCount: 0, missingKeys: [], appliedKeys: [] };
    }

    const changes = data.changes || {};
    const keys = Object.keys(changes);
    if (keys.length === 0) {
        console.log("[APPLY DEV SETTINGS] No modified parameters in settings file.");
        if (!options?.keepFile) {
            fs.unlinkSync(targetFile);
        }
        return { appliedCount: 0, missingKeys: [], appliedKeys: [] };
    }

    const repoRoot = path.resolve(__dirname, "..");
    const shaderParamsPath = path.join(repoRoot, "sow-render", "src", "shader_params.rs");
    const themePath = path.join(repoRoot, "sow-client", "src", "ui", "theme.rs");

    let shaderParamsContent = fs.readFileSync(shaderParamsPath, "utf8");
    let themeContent = fs.existsSync(themePath) ? fs.readFileSync(themePath, "utf8") : "";

    let appliedCount = 0;
    const missingKeys: string[] = [];
    const appliedKeys: string[] = [];

    for (const key of keys) {
        const val = changes[key];
        const spRes = updateShaderParams(shaderParamsContent, key, val);
        if (spRes.updated) {
            shaderParamsContent = spRes.content;
            appliedCount++;
            appliedKeys.push(key);
        } else {
            missingKeys.push(key);
        }

        if (themeContent) {
            const thRes = updateThemeDefaults(themeContent, key, val);
            if (thRes.updated) {
                themeContent = thRes.content;
            }
        }
    }

    if (!options?.dryRun) {
        fs.writeFileSync(shaderParamsPath, shaderParamsContent, "utf8");
        if (themeContent) {
            fs.writeFileSync(themePath, themeContent, "utf8");
        }
        if (!options?.keepFile) {
            try {
                fs.unlinkSync(targetFile);
            } catch (err) {
                console.warn(`[APPLY DEV SETTINGS] Failed to delete ${targetFile}:`, err);
            }
        }
    }

    console.log(
        `[APPLY DEV SETTINGS] Applied ${appliedCount} settings to codebase (${targetFile}${options?.dryRun ? " [DRY RUN]" : " [DELETED]"}).`,
    );
    if (missingKeys.length > 0) {
        console.warn(`[APPLY DEV SETTINGS] Warning: ${missingKeys.length} keys not found in shader_params:`, missingKeys);
    }

    return { appliedCount, missingKeys, appliedKeys };
}

if (import.meta.main) {
    const customPath = process.argv[2];
    const result = applyDevSettings(customPath);
    if (result.appliedCount === 0 && result.missingKeys.length === 0) {
        process.exit(1);
    }
}
