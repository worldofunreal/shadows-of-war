"use strict";

(function (global) {
    const MAX_RESULTS = 40;

    function normalize(value) {
        return String(value || "").trim().toLocaleLowerCase();
    }

    function resolve(entities, value) {
        const query = normalize(value);
        if (!query) return null;
        return entities.find(entity => normalize(entity.id) === query || normalize(entity.name) === query) || null;
    }

    function search(entities, value) {
        const query = normalize(value);
        if (query.length < 2) return [];
        const exact = resolve(entities, query);
        const matches = entities.filter(entity => normalize(entity.name).includes(query) || normalize(entity.id).includes(query));
        if (exact) return [exact, ...matches.filter(entity => entity !== exact)].slice(0, MAX_RESULTS);
        return matches.slice(0, MAX_RESULTS);
    }

    function attach(options) {
        const input = options.input;
        const results = options.results;
        const status = options.status;
        const getEntities = options.getEntities;
        const onChange = options.onChange || function () {};
        let entities = null;
        let loading = null;
        let selected = null;
        let revision = 0;

        input.setAttribute("aria-autocomplete", "list");
        input.setAttribute("aria-controls", results.id);
        input.setAttribute("aria-expanded", "false");

        function message(text) {
            results.replaceChildren();
            status.textContent = text;
            input.setAttribute("aria-expanded", "false");
        }

        function loadEntities() {
            if (entities) return Promise.resolve(entities);
            if (!loading) {
                loading = Promise.resolve().then(getEntities).then(value => {
                    if (!Array.isArray(value)) throw new Error("Invalid Atlas entity list.");
                    entities = value;
                    return entities;
                }).catch(error => {
                    loading = null;
                    throw error;
                });
            }
            return loading;
        }

        function choose(entity) {
            selected = entity;
            input.value = entity.name;
            results.replaceChildren();
            status.textContent = `Selected: ${entity.name} · ${entity.kind.replaceAll("_", " ")} · ${entity.region.replaceAll("_", " ")}`;
            input.setAttribute("aria-expanded", "false");
            onChange(entity);
        }

        async function render() {
            const request = ++revision;
            const query = input.value.trim();
            selected = null;
            onChange(null);
            if (query.length < 2) {
                message("Type at least 2 characters to search the Atlas.");
                return;
            }
            try {
                const list = await loadEntities();
                if (request !== revision) return;
                selected = resolve(list, query);
                onChange(selected);
                const matches = search(list, query);
                results.replaceChildren();
                if (!matches.length) {
                    status.textContent = "No Atlas entities match that search.";
                    input.setAttribute("aria-expanded", "false");
                    return;
                }
                for (const entity of matches) {
                    const option = document.createElement("button");
                    option.type = "button";
                    option.className = "atlas-picker__option";
                    option.setAttribute("aria-pressed", String(entity === selected));
                    const name = document.createElement("span");
                    name.textContent = entity.name;
                    const meta = document.createElement("span");
                    meta.className = "atlas-picker__meta";
                    meta.textContent = `${entity.kind.replaceAll("_", " ")} · ${entity.region.replaceAll("_", " ")} · ${entity.id}`;
                    option.append(name, meta);
                    option.addEventListener("click", () => choose(entity));
                    results.append(option);
                }
                status.textContent = selected
                    ? `Exact match selected: ${selected.name}.`
                    : matches.length === MAX_RESULTS
                        ? `Showing the first ${MAX_RESULTS} matches. Refine the search if needed.`
                        : `${matches.length} Atlas ${matches.length === 1 ? "entity" : "entities"} found. Select one.`;
                input.setAttribute("aria-expanded", "true");
            } catch (error) {
                if (request !== revision) return;
                message(`Atlas entities could not be loaded: ${error.message}`);
            }
        }

        input.addEventListener("input", render);
        input.addEventListener("focus", () => {
            if (input.value.trim().length < 2) message("Type at least 2 characters to search the Atlas.");
            else render();
        });
        input.addEventListener("keydown", event => {
            if (event.key === "Escape") message("Search closed.");
        });

        return {
            getSelected() {
                if (selected && normalize(input.value) === normalize(selected.name)) return selected;
                return resolve(entities || [], input.value);
            },
            async select(id) {
                const list = await loadEntities();
                const entity = list.find(item => item.id === id);
                if (!entity) return false;
                choose(entity);
                return true;
            },
            clear() {
                selected = null;
                input.value = "";
                message("Type at least 2 characters to search the Atlas.");
                onChange(null);
            }
        };
    }

    global.SOWAtlasEntityPicker = Object.freeze({ attach, resolve, search });
})(globalThis);
