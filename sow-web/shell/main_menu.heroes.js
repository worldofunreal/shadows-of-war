// Hero selection, filtering, dropdowns, and preview updates.

    function renderHeroesCard(leader, activeId) {
        var selected = leader.id === activeId;
        var avatarUrl = asset("gameplay/avatars/" + leader.slug + ".webp");
        return "<button class='sow-heroes__card" + (selected ? " is-selected" : "") + "' type='button' data-command='preview_leader' data-leader-id='" + esc(leader.id) + "' aria-pressed='" + selected + "'>" +
            "<span class='sow-heroes__card-art' style=\"background-image:url('" + esc(avatarUrl) + "')\"></span>" +
            "<span class='sow-heroes__card-copy'><strong>" + esc(leader.name) + "</strong><small>" + esc(leader.civilization) + "</small><em>" + esc(leader.perk || SOW_t("heroes.command_trait")) + "</em></span>" +
            (selected ? "<span class='sow-heroes__card-state'>" + esc(SOW_t("heroes.selected")) + "</span>" : "") +
            "</button>";
    }

    function heroesRoster(leaders) {
        var query = heroesSearchQuery.trim().toLowerCase();
        return leaders.filter(function (leader) {
            var matchesQuery = !query ||
                String(leader.name || "").toLowerCase().indexOf(query) !== -1 ||
                String(leader.civilization || "").toLowerCase().indexOf(query) !== -1;
            var regionKey = String(leader.slug || leader.id || "").toLowerCase();
            var matchesRegion = heroesRegionFilter === "all" || LEADER_REGIONS[regionKey] === heroesRegionFilter;
            return matchesQuery && matchesRegion;
        });
    }

    function renderDropdown(config) {
        var value = config.value == null ? "" : String(config.value);
        var options = config.options || [];
        var key = String(config.key);
        var inputAttrs = " data-dropdown-value" + (config.setting ? " data-setting='" + esc(config.setting) + "'" : "");
        return "<div class='sow-control-dropdown" + (config.className ? " " + esc(config.className) : "") + "' data-control-dropdown data-dropdown-key='" + esc(key) + "'>" +
            "<button class='sow-control-dropdown__trigger' type='button' data-command='toggle_dropdown' data-dropdown-key='" + esc(key) + "' data-role='dropdown-trigger' aria-haspopup='listbox' aria-expanded='false' aria-controls='sow-dropdown-" + esc(key) + "'>" +
                "<span data-dropdown-label>" + esc((options.find(function (option) { return String(option.value) === value; }) || {}).label || value) + "</span><span class='sow-control-dropdown__chevron' aria-hidden='true'>⌄</span>" +
            "</button>" +
            "<div class='sow-control-dropdown__menu' id='sow-dropdown-" + esc(key) + "' role='listbox' aria-label='" + esc(config.label || SOW_t("lobbies.search_option")) + "' hidden>" +
                options.map(function (option) {
                    var selected = String(option.value) === value;
                    return "<button class='sow-control-dropdown__option' type='button' role='option' data-command='select_dropdown' data-dropdown-key='" + esc(key) + "' data-dropdown-option-value='" + esc(option.value) + "' aria-selected='" + (selected ? "true" : "false") + "'>" + esc(option.label) + "</button>";
                }).join("") +
            "</div>" +
            "<input type='hidden' name='" + esc(config.name || key) + "' value='" + esc(value) + "' data-dropdown-input" + inputAttrs + ">" +
        "</div>";
    }

    function renderHeroesRegionDropdown() {
        return renderDropdown({
            key: "heroes-region",
            label: SOW_t("heroes.filter_leaders_region"),
            name: "region",
            value: heroesRegionFilter,
            className: "sow-heroes__region",
            options: [
                { value: "all", label: SOW_t("heroes.all_regions") },
                { value: "Europe", label: SOW_t("heroes.europe") },
                { value: "Africa", label: SOW_t("heroes.africa") },
                { value: "Asia", label: SOW_t("heroes.asia") },
                { value: "Americas", label: SOW_t("heroes.americas") }
            ]
        });
    }

    function renderHeroesRoster(activeId) {
        var leaders = state && Array.isArray(state.leaders) ? state.leaders : [];
        var filtered = heroesRoster(leaders);
        var cards = filtered.map(function (leader) { return renderHeroesCard(leader, activeId); }).join("");
        return cards || "<p class='sow-menu__empty sow-heroes__empty'>" + esc(SOW_t("heroes.no_leaders")) + "</p>";
    }

    function refreshHeroesRoster(resetScroll) {
        var panel = activeScreenPanel() || root;
        var heroesRosterContainer = panel.querySelector("[data-heroes-roster]");
        if (!heroesRosterContainer) return;
        var activeHeroesId = tempSelectedLeader || (state && state.selected_leader) || "Caesar";
        heroesRosterContainer.innerHTML = renderHeroesRoster(activeHeroesId);
        if (resetScroll) {
            var heroesRosterPanel = panel.querySelector(".sow-heroes__roster");
            if (heroesRosterPanel) heroesRosterPanel.scrollTop = 0;
        }
    }

    function updateHeroesPreview() {
        if (currentScreen() !== "heroes") return false;
        var activeLeader = leaderById(tempSelectedLeader || (state && state.selected_leader) || "Caesar");
        var panel = activeScreenPanel() || root;
        var featured = panel.querySelector(".sow-heroes__featured");
        if (!activeLeader || !featured) return false;
        var picture = featured.querySelector("picture");
        var source = picture && picture.querySelector("source");
        var image = picture && picture.querySelector("img");
        var title = featured.querySelector("#sow-heroes-selected");
        var civilization = featured.querySelector(".sow-heroes__civilization");
        var perk = featured.querySelector(".sow-heroes__perk");
        var confirm = featured.querySelector("[data-command='confirm_leader']");
        var selectedAsset = asset("shell/leaders/" + activeLeader.slug + "_mobile.webp");
        var landscapeAsset = asset("shell/leaders/" + activeLeader.slug + "_desktop.webp");
        if (source) source.srcset = landscapeAsset;
        if (image) {
            image.src = selectedAsset;
            image.alt = activeLeader.name;
        }
        if (title) title.textContent = activeLeader.name;
        if (civilization) civilization.textContent = activeLeader.civilization;
        if (perk) perk.textContent = activeLeader.perk || SOW_t("heroes.enhanced_bonuses");
        var purchase = featured.querySelector("[data-hero-purchase]");
        if (purchase) purchase.innerHTML = renderLeaderPurchase(activeLeader);
        if (confirm) {
            confirm.dataset.leaderId = activeLeader.id;
            confirm.firstChild.nodeValue = SOW_t("heroes.confirm_leader", { name: activeLeader.name.toUpperCase() }) + " ";
        }
        var cards = panel.querySelectorAll("[data-command='preview_leader']");
        for (var i = 0; i < cards.length; i++) {
            var selected = cards[i].dataset.leaderId === activeLeader.id;
            cards[i].classList.toggle("is-selected", selected);
            cards[i].setAttribute("aria-pressed", selected ? "true" : "false");
            var marker = cards[i].querySelector(".sow-heroes__card-state");
            if (selected && !marker) {
                cards[i].insertAdjacentHTML("beforeend", "<span class='sow-heroes__card-state'>" + esc(SOW_t("heroes.selected")) + "</span>");
            } else if (!selected && marker) {
                marker.remove();
            }
        }
        return true;
    }

    function syncDropdowns(focusTarget) {
        var scopes = [];
        var panel = activeScreenPanel();
        if (panel) scopes.push(panel);
        root.querySelectorAll("[data-menu-overlay]").forEach(function (overlay) { scopes.push(overlay); });
        if (!scopes.length) scopes.push(root);
        for (var s = 0; s < scopes.length; s++) {
            var dropdowns = scopes[s].querySelectorAll("[data-control-dropdown]");
            for (var i = 0; i < dropdowns.length; i++) {
                var dropdown = dropdowns[i];
            var key = dropdown.dataset.dropdownKey;
            var open = dropdownOpenKey === key;
            var trigger = dropdown.querySelector("[data-role='dropdown-trigger']");
            var menu = dropdown.querySelector("[role='listbox']");
            if (!trigger || !menu) continue;
            trigger.setAttribute("aria-expanded", open ? "true" : "false");
            menu.hidden = !open;
            var input = dropdown.querySelector("[data-dropdown-value]");
            var value = input ? input.value : "";
            var label = dropdown.querySelector("[data-dropdown-label]");
            var options = dropdown.querySelectorAll("[role='option']");
            for (var j = 0; j < options.length; j++) {
                var selected = options[j].dataset.dropdownOptionValue === value;
                options[j].setAttribute("aria-selected", selected ? "true" : "false");
                if (label && selected) label.textContent = options[j].textContent;
            }
            }
        }
        if (focusTarget) focusTarget.focus();
    }

    function setDropdownOpen(key, open, focusTarget) {
        dropdownOpenKey = open ? key : null;
        syncDropdowns(focusTarget);
    }

    function selectDropdown(key, value) {
        var panel = activeScreenPanel();
        var dropdown = panel && panel.querySelector("[data-control-dropdown][data-dropdown-key='" + key + "']");
        if (!dropdown) dropdown = root.querySelector("[data-menu-overlay] [data-control-dropdown][data-dropdown-key='" + key + "']");
        if (!dropdown) return;
        var input = dropdown.querySelector("[data-dropdown-input]");
        if (!input) return;
        input.value = value;
        dropdownOpenKey = null;
        if (key === "heroes-region") {
            heroesRegionFilter = value || "all";
            syncDropdowns();
            refreshHeroesRoster(true);
            return;
        }
        syncDropdowns();
        input.dispatchEvent(new Event("change", { bubbles: true }));
    }

    function renderHeroes() {
        var activeId = tempSelectedLeader || (state && state.selected_leader) || "Caesar";
        var activeLeader = leaderById(activeId);
        var portraitAsset = asset("shell/leaders/" + activeLeader.slug + "_mobile.webp");
        var landscapeAsset = asset("shell/leaders/" + activeLeader.slug + "_desktop.webp");
        return "<main class='sow-menu__main sow-menu__main--heroes' data-screen-panel='heroes'><section class='sow-menu__heroes-slot' aria-label='" + esc(SOW_t("menu.heroes")) + "'>" +
                    "<div class='sow-heroes__workspace'>" +
                        "<section class='sow-heroes__featured' aria-labelledby='sow-heroes-selected'><picture><source media='(max-width: 700px) and (orientation: portrait)' srcset='" + esc(landscapeAsset) + "'><img src='" + esc(portraitAsset) + "' alt='" + esc(activeLeader.name) + "' width='1080' height='1920' fetchpriority='high'></picture><div class='sow-heroes__featured-copy'><p class='sow-heroes__featured-label'>" + esc(SOW_t("heroes.selected")) + "</p><h2 id='sow-heroes-selected'>" + esc(activeLeader.name) + "</h2><p class='sow-heroes__civilization'>" + esc(activeLeader.civilization) + "</p><p class='sow-heroes__perk'>" + esc(activeLeader.perk || SOW_t("heroes.enhanced_bonuses")) + "</p><div data-hero-purchase>" + renderLeaderPurchase(activeLeader) + "</div><button class='sow-menu__primary sow-heroes__confirm' type='button' data-command='confirm_leader' data-leader-id='" + esc(activeLeader.id) + "'>" + esc(SOW_t("heroes.confirm_leader", { name: activeLeader.name.toUpperCase() })) + " <span>✓</span></button></div></section>" +
                        "<section class='sow-heroes__roster' aria-label='" + esc(SOW_t("heroes.leader_list")) + "'><div class='sow-heroes__section-head'><div class='sow-heroes__filters'><label class='sow-heroes__search'><span class='sow-heroes__sr-only'>" + esc(SOW_t("heroes.search_leaders")) + "</span><input data-role='heroes-search' type='search' placeholder='" + esc(SOW_t("heroes.search_leader_civilization")) + "' value=\"" + esc(heroesSearchQuery) + "\" autocomplete='off' spellcheck='false'></label>" + renderHeroesRegionDropdown() + "</div></div><div class='sow-heroes__grid' data-heroes-roster aria-live='polite'>" + renderHeroesRoster(activeId) + "</div></section>" +
                    "</div>" +
        "</section></main>";
    }
