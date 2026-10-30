(function () {
  "use strict";

  const $ = <T extends HTMLElement = HTMLElement>(id: string): T => document.getElementById(id) as T,
    mapsList = $("maps"),
    emptyLine = $("empty"),
    statusLine = $("status");

  function setStatus(text: string, error = false) {
    statusLine.textContent = text;
    statusLine.className = error ? "status error" : "status";
  }

  function row(name: string) {
    const item = document.createElement("div");
    item.className = "map-row";
    const label = document.createElement("code");
    label.textContent = name;
    const remove = document.createElement("button");
    remove.textContent = "Delete from server";
    remove.addEventListener("click", () => deleteMap(name, remove));
    item.append(label, remove);
    return item;
  }

  async function loadMaps() {
    mapsList.replaceChildren();
    emptyLine.hidden = true;
    const response = await fetch("/__map-library");
    const text = await response.text();
    if (!response.ok) {
      setStatus(text || "Could not read the server's maps.", true);
      return;
    }
    const { maps } = JSON.parse(text) as { maps: string[] };
    if (maps.length === 0) emptyLine.hidden = false;
    mapsList.append(...maps.map(row));
  }

  async function deleteMap(name: string, button: HTMLButtonElement) {
    if (!confirm(`Delete ${name} from the server? Players lose it right away.`)) return;
    button.disabled = true;
    setStatus(`Deleting ${name}…`);
    try {
      const response = await fetch(`/__map-delete?map=${encodeURIComponent(name)}`, { method: "POST" });
      const text = await response.text();
      if (!response.ok) {
        setStatus(text || "Could not delete the map.", true);
        button.disabled = false;
        return;
      }
      setStatus(`Deleted ${name} from the server.`);
      await loadMaps();
    } catch (error) {
      setStatus(String(error), true);
      button.disabled = false;
    }
  }

  loadMaps().catch((error) => setStatus(String(error), true));
})();
