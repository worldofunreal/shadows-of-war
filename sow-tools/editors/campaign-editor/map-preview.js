(function (host) {
    "use strict";

    function parse(buffer) {
        var view = new DataView(buffer), offset = 4;
        if (buffer.byteLength < 20 || String.fromCharCode(view.getUint8(0), view.getUint8(1), view.getUint8(2), view.getUint8(3)) !== "SOWM") {
            throw new Error("Invalid SOW map file.");
        }
        offset += 4; // version and reserved
        var width = view.getUint32(offset, true); offset += 4;
        var height = view.getUint32(offset, true); offset += 4;
        offset += 4; // land-tile count
        function skipString() {
            if (offset + 2 > buffer.byteLength) throw new Error("Truncated SOW map header.");
            var length = view.getUint16(offset, true); offset += 2;
            if (offset + length > buffer.byteLength) throw new Error("Truncated SOW map string.");
            offset += length;
        }
        skipString();
        if (offset + 2 > buffer.byteLength) throw new Error("Truncated SOW map spawns.");
        var spawnCount = view.getUint16(offset, true); offset += 2;
        for (var i = 0; i < spawnCount; i++) { skipString(); skipString(); offset += 8; }
        var tileCount = width * height;
        if (!width || !height || !Number.isSafeInteger(tileCount) || offset + tileCount > buffer.byteLength) {
            throw new Error("Invalid SOW map dimensions.");
        }
        return { width: width, height: height, terrain: new Uint8Array(buffer, offset, tileCount) };
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
            return response.arrayBuffer();
        }).then(function (buffer) {
            var map = parse(buffer);
            map.id = mapId;
            map.image = terrainCanvas(map.terrain, map.width, map.height);
            return map;
        });
    }

    var api = { parse: parse, terrainCanvas: terrainCanvas, load: load };
    if (typeof module !== "undefined" && module.exports) module.exports = api;
    else host.SOWCampaignMapPreview = api;
})(typeof globalThis !== "undefined" ? globalThis : this);
