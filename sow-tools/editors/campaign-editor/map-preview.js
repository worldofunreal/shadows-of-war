(function (host) {
    "use strict";

    function parse(buffer) {
        var view = new DataView(buffer), offset = 4;
        if (buffer.byteLength < 20 || String.fromCharCode(view.getUint8(0), view.getUint8(1), view.getUint8(2), view.getUint8(3)) !== "SOWM") {
            throw new Error("Invalid SOW map file.");
        }
        var version = view.getUint16(offset, true); offset += 4; // version and reserved
        if (version !== 1 && version !== 2) throw new Error("Unsupported SOW map version: " + version);
        var width = view.getUint32(offset, true); offset += 4;
        var height = view.getUint32(offset, true); offset += 4;
        offset += 4; // land-tile count
        function readString() {
            if (offset + 2 > buffer.byteLength) throw new Error("Invalid SOW map string.");
            var length = view.getUint16(offset, true); offset += 2;
            if (offset + length > buffer.byteLength) throw new Error("Invalid SOW map string.");
            var value = new TextDecoder().decode(new Uint8Array(buffer, offset, length));
            offset += length;
            return value;
        }
        var displayName = readString();
        if (offset + 2 > buffer.byteLength) throw new Error("Invalid SOW map header.");
        var spawnCount = view.getUint16(offset, true); offset += 2;
        var spawns = [];
        for (var i = 0; i < spawnCount; i++) {
            var name = readString(), flag = readString();
            if (offset + 8 > buffer.byteLength) throw new Error("Invalid SOW map header.");
            var x = view.getUint32(offset, true), y = view.getUint32(offset + 4, true);
            offset += 8;
            spawns.push({ name: name, flag: flag, x: x, y: y });
        }
        var geoBounds = null;
        if (version === 2) {
            if (offset >= buffer.byteLength) throw new Error("Missing SOW map geographic bounds tag.");
            var inlineGeoTag = view.getUint8(offset++);
            if (inlineGeoTag === 1) {
                if (offset + 16 > buffer.byteLength) throw new Error("Invalid SOW map geographic bounds.");
                geoBounds = {
                    minLon: view.getInt32(offset, true) / 1e6,
                    minLat: view.getInt32(offset + 4, true) / 1e6,
                    maxLon: view.getInt32(offset + 8, true) / 1e6,
                    maxLat: view.getInt32(offset + 12, true) / 1e6
                };
                if (geoBounds.maxLon <= geoBounds.minLon || geoBounds.maxLat <= geoBounds.minLat) {
                    throw new Error("Invalid SOW map geographic bounds.");
                }
                offset += 16;
            } else if (inlineGeoTag !== 0) {
                throw new Error("Invalid SOW map geographic bounds tag: " + inlineGeoTag);
            }
        }
        var tileCount = width * height;
        if (!width || !height || !Number.isSafeInteger(tileCount) || offset + tileCount > buffer.byteLength) {
            throw new Error("Invalid SOW map dimensions.");
        }
        var terrain = new Uint8Array(buffer, offset, tileCount), tail = offset + tileCount;
        var defaultRoster = null, rosters = [];
        if (tail < buffer.byteLength) {
            if (version === 1) {
                var tag = view.getUint8(tail++);
                if (tag === 1) {
                    if (tail + 16 > buffer.byteLength) throw new Error("Invalid SOW map geographic bounds.");
                    geoBounds = {
                        minLon: view.getInt32(tail, true) / 1e6,
                        minLat: view.getInt32(tail + 4, true) / 1e6,
                        maxLon: view.getInt32(tail + 8, true) / 1e6,
                        maxLat: view.getInt32(tail + 12, true) / 1e6
                    };
                    if (geoBounds.maxLon <= geoBounds.minLon || geoBounds.maxLat <= geoBounds.minLat) {
                        throw new Error("Invalid SOW map geographic bounds.");
                    }
                    tail += 16;
                } else if (tag !== 0) {
                    throw new Error("Invalid SOW map geographic bounds tag: " + tag);
                }
            }
            if (tail < buffer.byteLength) {
                if (view.getUint8(tail++) !== 2 || tail + 4 > buffer.byteLength) throw new Error("Invalid SOW map roster tag.");
                var rosterLength = view.getUint32(tail, true); tail += 4;
                var rosterEnd = tail + rosterLength, cursor = tail;
                if (!Number.isSafeInteger(rosterEnd) || rosterEnd > buffer.byteLength || rosterEnd - cursor < 4) throw new Error("Invalid SOW map roster length.");
                function rosterU16() { if (cursor + 2 > rosterEnd) throw new Error("Truncated SOW map roster."); var n = view.getUint16(cursor, true); cursor += 2; return n; }
                function rosterU32() { if (cursor + 4 > rosterEnd) throw new Error("Truncated SOW map roster."); var n = view.getUint32(cursor, true); cursor += 4; return n; }
                function rosterString() { var length = rosterU16(); if (cursor + length > rosterEnd) throw new Error("Truncated SOW map roster string."); var s = new TextDecoder().decode(new Uint8Array(buffer, cursor, length)); cursor += length; return s; }
                if (rosterU16() !== 1) throw new Error("Unsupported SOW map roster version.");
                defaultRoster = rosterString();
                var presetCount = rosterU16();
                for (var p = 0; p < presetCount; p++) {
                    var preset = { id: rosterString(), name: rosterString(), entries: [] };
                    var entryCount = rosterU16();
                    for (var e = 0; e < entryCount; e++) {
                        var entityId = rosterString();
                        var role = cursor < rosterEnd ? view.getUint8(cursor++) : -1;
                        if (role !== 0 && role !== 1) throw new Error("Invalid SOW map roster role.");
                        var entryX = rosterU32(), entryY = rosterU32(), anchor = rosterU16();
                        preset.entries.push({ entity_id: entityId, role: role === 0 ? "nation" : "tribe", x: entryX, y: entryY, legacy_anchor: anchor === 65535 ? null : anchor });
                    }
                    rosters.push(preset);
                }
                if (cursor !== rosterEnd || rosterEnd !== buffer.byteLength || !rosters.some(function (preset) { return preset.id === defaultRoster; })) throw new Error("Invalid SOW map roster data.");
            }
        }
        return { displayName: displayName, width: width, height: height, terrain: terrain, geoBounds: geoBounds, spawns: spawns, defaultRoster: defaultRoster, rosters: rosters };
    }

    function geoToMapPoint(lat, lon, bounds, width, height) {
        if (!bounds || !Number.isFinite(lat) || !Number.isFinite(lon) || !width || !height) return null;
        var normalizedLon = bounds.maxLon > 180 && lon < bounds.minLon ? lon + 360 : lon;
        var lonSpan = bounds.maxLon - bounds.minLon, latSpan = bounds.maxLat - bounds.minLat;
        if (lonSpan <= 0 || latSpan <= 0 || lat < bounds.minLat || lat > bounds.maxLat ||
            normalizedLon < bounds.minLon || normalizedLon > bounds.maxLon) return null;
        return {
            x: Math.min(width - Number.EPSILON * width, (normalizedLon - bounds.minLon) / lonSpan * width),
            y: Math.min(height - Number.EPSILON * height, (bounds.maxLat - lat) / latSpan * height)
        };
    }

    function mapPointToGeo(x, y, bounds, width, height) {
        if (!bounds || !Number.isFinite(x) || !Number.isFinite(y) || !width || !height ||
            x < 0 || y < 0 || x >= width || y >= height) return null;
        var lonSpan = bounds.maxLon - bounds.minLon, latSpan = bounds.maxLat - bounds.minLat;
        if (lonSpan <= 0 || latSpan <= 0) return null;
        var lon = bounds.minLon + x / width * lonSpan;
        while (lon > 180) lon -= 360;
        while (lon < -180) lon += 360;
        var lat = bounds.maxLat - y / height * latSpan;
        if (lat < -90 || lat > 90) return null;
        return {
            lat: lat,
            lon: lon
        };
    }

    function geoToMapPoints(lat, lon, bounds, width, height) {
        if (!bounds || !Number.isFinite(lat) || !Number.isFinite(lon) || !width || !height) return [];
        var firstWrap = Math.ceil((bounds.minLon - lon) / 360);
        var lastWrap = Math.floor((bounds.maxLon - lon) / 360);
        var points = [];
        for (var wrap = firstWrap; wrap <= lastWrap; wrap++) {
            var point = geoToMapPoint(lat, lon + wrap * 360, bounds, width, height);
            if (point && !points.some(function (other) { return Math.abs(other.x - point.x) < 1e-9; })) {
                points.push(point);
            }
        }
        return points;
    }

    function centerMapOffsets(point, scale, viewWidth, viewHeight) {
        return {
            x: viewWidth / 2 - point.x * scale,
            y: viewHeight / 2 - point.y * scale
        };
    }

    function resizeCanvas(canvas, dpr) {
        var rect = canvas.getBoundingClientRect();
        dpr = Number.isFinite(dpr) && dpr > 0 ? dpr : 1;
        canvas.width = Math.max(1, Math.round(rect.width * dpr));
        canvas.height = Math.max(1, Math.round(rect.height * dpr));
        return { width: rect.width, height: rect.height, dpr: dpr };
    }

    function fitMapView(mapWidth, mapHeight, viewWidth, viewHeight) {
        if (![mapWidth, mapHeight, viewWidth, viewHeight].every(function (value) {
            return Number.isFinite(value) && value > 0;
        })) return null;
        var scale = Math.min(viewWidth / mapWidth, viewHeight / mapHeight);
        return {
            scale: scale,
            fitScale: scale,
            minScale: scale * 0.5,
            x: (viewWidth - mapWidth * scale) / 2,
            y: (viewHeight - mapHeight * scale) / 2
        };
    }

    function mapToScreen(point, view) {
        return [point[0] * view.scale + view.x, point[1] * view.scale + view.y];
    }

    function screenToMap(point, view) {
        return [(point[0] - view.x) / view.scale, (point[1] - view.y) / view.scale];
    }

    function panMapView(view, dx, dy) {
        return { scale: view.scale, x: view.x + dx, y: view.y + dy };
    }

    function zoomMapView(view, point, factor, minScale, maxScale) {
        var mapPoint = screenToMap(point, view);
        var scale = Math.max(minScale, Math.min(maxScale, view.scale * factor));
        return {
            scale: scale,
            x: point[0] - mapPoint[0] * scale,
            y: point[1] - mapPoint[1] * scale
        };
    }

    function geoToTile(lat, lon, bounds, width, height) {
        var point = geoToMapPoint(lat, lon, bounds, width, height);
        return point && { x: Math.trunc(point.x), y: Math.trunc(point.y) };
    }

    function drawMapLabel(ctx, name, x, y, bold) {
        ctx.font = (bold ? "bold " : "") + "11px system-ui";
        ctx.textAlign = "center";
        ctx.fillStyle = "rgba(0,0,0,.6)";
        ctx.fillText(name, x + 1, y + 1);
        ctx.fillStyle = "#fff";
        ctx.fillText(name, x, y);
    }

    function tileToGeo(x, y, bounds, width, height) {
        if (!bounds || !Number.isInteger(x) || !Number.isInteger(y) ||
            x < 0 || y < 0 || x >= width || y >= height) return null;
        var lon = bounds.minLon + (x + 0.5) / width * (bounds.maxLon - bounds.minLon);
        if (lon > 180) lon -= 360;
        var geo = {
            lat: bounds.maxLat - (y + 0.5) / height * (bounds.maxLat - bounds.minLat),
            lon: lon
        };
        var projected = geoToTile(geo.lat, geo.lon, bounds, width, height);
        return projected && projected.x === x && projected.y === y ? geo : null;
    }

    function isLand(terrain, width, height, x, y) {
        x = Math.floor(x); y = Math.floor(y);
        return !!terrain && x >= 0 && y >= 0 && x < width && y < height &&
            (terrain[y * width + x] & 0x80) !== 0;
    }

    function clampTile(value, size) {
        var coordinate = Number(value);
        if (!Number.isFinite(coordinate) || !Number.isInteger(size) || size < 1) return 0;
        return Math.max(0, Math.min(size - 1, Math.round(coordinate)));
    }

    function terrainCanvas(terrain, width, height) {
        var canvas = document.createElement("canvas");
        canvas.width = width; canvas.height = height;
        var image = canvas.getContext("2d").createImageData(width, height), pixels = image.data;
        for (var i = 0; i < width * height; i++) {
            var value = terrain[i], land = (value & 0x80) !== 0, elevation = value & 0x7f;
            var red = !land ? 44 : elevation >= 0x18 ? 150 : elevation >= 0x0c ? 120 : 104;
            var green = !land ? 92 : elevation >= 0x18 ? 132 : elevation >= 0x0c ? 140 : 142;
            var blue = !land ? 140 : elevation >= 0x18 ? 96 : elevation >= 0x0c ? 86 : 90;
            var at = i * 4;
            pixels[at] = red; pixels[at + 1] = green; pixels[at + 2] = blue; pixels[at + 3] = 255;
        }
        canvas.getContext("2d").putImageData(image, 0, 0);
        return canvas;
    }

    function load(mapId) {
        return fetch("/assets/maps/" + encodeURIComponent(mapId) + "/map.bin", { cache: "no-store" }).then(function (response) {
            if (!response.ok) throw new Error("Map unavailable: " + mapId);
            return Promise.all([response.arrayBuffer(), response.headers.get("ETag")]);
        }).then(function (result) {
            var buffer = result[0];
            var map = parse(buffer);
            map.id = mapId;
            map.etag = result[1];
            map.image = terrainCanvas(map.terrain, map.width, map.height);
            return map;
        });
    }

    var api = { parse: parse, geoToMapPoint: geoToMapPoint, mapPointToGeo: mapPointToGeo, geoToMapPoints: geoToMapPoints, centerMapOffsets: centerMapOffsets, resizeCanvas: resizeCanvas, fitMapView: fitMapView, mapToScreen: mapToScreen, screenToMap: screenToMap, panMapView: panMapView, zoomMapView: zoomMapView, geoToTile: geoToTile, tileToGeo: tileToGeo, isLand: isLand, clampTile: clampTile, drawMapLabel: drawMapLabel, terrainCanvas: terrainCanvas, load: load };
    if (typeof module !== "undefined" && module.exports) module.exports = api;
    else host.SOWCampaignMapPreview = api;
})(typeof globalThis !== "undefined" ? globalThis : this);
