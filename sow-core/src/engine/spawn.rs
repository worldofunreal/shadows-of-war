use crate::engine::SowEngine;

/// Minimum clear radius (tiles) around a new bot; nations sit farther apart.
const NATION_SPAWN_SPACING: i32 = 24;
const TRIBE_SPAWN_SPACING: i32 = 15;

fn sample_map_roster(
    preset: Option<&crate::map_file::MapRosterPreset>,
    role: crate::map_file::MapRosterRole,
    map: &crate::map::GameMap,
    count: usize,
    rng: &mut wyrand::WyRand,
) -> Vec<crate::map_file::MapRosterEntry> {
    let Some(preset) = preset else {
        return Vec::new();
    };
    let mut candidates: Vec<usize> = preset
        .entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            if !map.is_valid_coord(entry.x as i32, entry.y as i32) {
                return None;
            }
            let tile = map.ref_id(entry.x, entry.y);
            (entry.role == role
                && crate::geo_entities::by_id(&entry.entity_id).is_some()
                && map.terrain[tile].is_land()
                && map.owner_id(entry.x, entry.y) == 0)
                .then_some(index)
        })
        .collect();
    let mut selected = Vec::with_capacity(count.min(candidates.len()));
    for _ in 0..count.min(candidates.len()) {
        let index = (rng.rand() as usize) % candidates.len();
        selected.push(preset.entries[candidates.swap_remove(index)].clone());
    }
    selected
}

pub struct HumanSpawn {
    pub player_id: u16,
    pub name: String,
    pub color: [f32; 3],
    pub team: Option<crate::protocol::Team>,
    pub leader: crate::player::Leader,
    pub skin_style: u8,
    pub is_ai_controlled: bool,
}

impl SowEngine {
    /// Random-match bots. Positions are spread over all land first (nations
    /// farther apart than tribes); each position then takes the free catalog
    /// character of that role whose home is closest. Map rosters keep their
    /// curated character and tile. Humans' heroes are skipped while any other
    /// character is free, and two bots never share a character.
    pub fn spawn_ai(&mut self, nation_count: u32, tribe_count: u32) {
        use crate::geo_entities::Role;
        use wyrand::WyRand;

        let mut rng = WyRand::new(self.state.seed);
        let mut roster_rng = WyRand::new(self.state.seed ^ 0x534f_5752_4f53_5445);
        let active_roster = self
            .state
            .config
            .map_roster_preset
            .as_deref()
            .and_then(|id| self.state.map_rosters.iter().find(|preset| preset.id == id))
            .cloned();
        if self.state.config.map_roster_preset.is_some() && active_roster.is_none() {
            log::error!(
                "spawn_ai: selected map roster {:?} is missing from map '{}'",
                self.state.config.map_roster_preset,
                self.state.config.map_name
            );
        }
        let mut used: std::collections::HashSet<&'static str> = std::collections::HashSet::new();

        for (role, count, spacing) in [
            (Role::Nation, nation_count, NATION_SPAWN_SPACING),
            (Role::Tribe, tribe_count, TRIBE_SPAWN_SPACING),
        ] {
            let roster_role = match role {
                Role::Nation => crate::map_file::MapRosterRole::Nation,
                _ => crate::map_file::MapRosterRole::Tribe,
            };
            let roster = sample_map_roster(
                active_roster.as_ref(),
                roster_role,
                &self.state.map,
                count as usize,
                &mut roster_rng,
            );
            let mut spawned = 0u32;
            for entry in &roster {
                let Some(character) = crate::geo_entities::by_id(&entry.entity_id) else {
                    continue;
                };
                if self.spawn_catalog_bot(character, (entry.x, entry.y)) {
                    used.insert(character.id.as_str());
                    spawned += 1;
                }
            }
            while spawned < count {
                let Some(position) = self.find_spaced_spawn(&mut rng, spacing) else {
                    log::error!("spawn_ai: no free land left for {role:?} bots");
                    break;
                };
                let Some(character) = self.closest_free_character(role, position, &used, &mut rng)
                else {
                    log::error!("spawn_ai: catalog has no free {role:?} character left");
                    break;
                };
                if !self.spawn_catalog_bot(character, position) {
                    break;
                }
                used.insert(character.id.as_str());
                spawned += 1;
            }
            log::info!(
                "spawn_ai: {spawned}/{count} {role:?} bots on '{}'",
                self.state.config.map_name
            );
        }
    }

    /// Closest free character of `role` to a map tile, by home coordinates.
    /// Maps without real geography pick at random. Basic f64 ops only (lockstep-safe).
    pub(crate) fn closest_free_character(
        &self,
        role: crate::geo_entities::Role,
        (x, y): (u32, u32),
        used: &std::collections::HashSet<&'static str>,
        rng: &mut wyrand::WyRand,
    ) -> Option<&'static crate::geo_entities::GeoEntity> {
        let home = self
            .state
            .geo_bounds
            .map(|bounds| bounds.unproject(x, y, self.state.map.width, self.state.map.height));
        let free: Vec<_> = crate::geo_entities::bots(role)
            .filter(|character| !used.contains(character.id.as_str()))
            .collect();
        if free.is_empty() {
            return None;
        }
        let Some((lat, lon)) = home else {
            return Some(free[(rng.rand() as usize) % free.len()]);
        };
        let distance = |character: &crate::geo_entities::GeoEntity| {
            let (Some(clat), Some(clon)) = (character.lat, character.lon) else {
                return f64::MAX;
            };
            let dlat = clat as f64 - lat;
            let dlon = (clon as f64 - lon).abs();
            let dlon = dlon.min(360.0 - dlon);
            dlat * dlat + dlon * dlon
        };
        free.into_iter()
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
    }

    fn spawn_catalog_bot(
        &mut self,
        character: &'static crate::geo_entities::GeoEntity,
        (x, y): (u32, u32),
    ) -> bool {
        use crate::geo_entities::Role;
        use crate::player::Player;
        let Some(bot_id) = self.state.next_free_player_id() else {
            log::error!("spawn_ai: player ID space exhausted");
            return false;
        };
        let Some((sx, sy)) = self.nearest_free_land(x, y) else {
            return false;
        };
        let config = self.state.config.clone();
        let color = match character.role {
            Role::Nation => {
                let Some(color) = crate::player::next_nation_palette_color(&self.state.players)
                else {
                    log::error!("spawn_ai: Nation color palette is exhausted");
                    return false;
                };
                color
            }
            Role::Tribe => [0.5; 3],
        };
        let mut player = if character.role == Role::Nation {
            let mut player = Player::new_nation(
                bot_id,
                character.name.clone(),
                color,
                &config,
            );
            // Nations form the opposing team in Humans vs Nations.
            if config.game_mode == "HumansVsNations" {
                player.team = Some(crate::protocol::Team::Blue);
            }
            player
        } else {
            Player::new_bot(
                bot_id,
                character.name.clone(),
                color,
                &config,
            )
        };
        // The bot is shown as its catalog people; its random hero only carries the ability.
        player.realm = Some(character.entity.clone());
        self.state.spawn_player(player, sx, sy);
        if let Some(avatar) = &character.avatar {
            self.campaign_avatars.insert(bot_id, avatar.clone());
        }
        true
    }

    pub fn spawn_human(&mut self, spawn: HumanSpawn) {
        let HumanSpawn {
            player_id,
            name,
            color,
            team,
            leader,
            skin_style,
            is_ai_controlled,
        } = spawn;
        use crate::player::Player;
        use wyrand::WyRand;

        // Use a different seed offset for human to avoid clashing exactly with bots
        let mut rng = WyRand::new(self.state.seed.wrapping_add(player_id as u64));
        let config = self.state.config.clone();
        let color = config.campaign_player_color.unwrap_or(color);

        // Ghost (is_ai_controlled) Humans are TOP of the food chain — the
        // novice's guide. Single top band (above nations, far above tribes);
        // no id-based tiers. Deterministic per (seed, player_id) →
        // lockstep-safe across clients.
        let ghost_iq: Option<u32> = if is_ai_controlled {
            use crate::rng::NextIntExt;
            let mut iq_rng = WyRand::new(
                self.state
                    .seed
                    .wrapping_add(player_id as u64)
                    .wrapping_add(0xA11CE),
            );
            Some(if config.ranked_1v1 {
                config.ranked_bot_iq.unwrap_or(100)
            } else {
                iq_rng.next_int(160, 181) as u32
            })
        } else {
            None
        };

        // Campaign: place the human at a fixed homeland tile (auto-spawn, no picker).
        if let Some((tx, ty)) = config.player_spawn {
            if let Some((sx, sy)) = self.nearest_free_land(tx, ty) {
                let mut player = Player::new_human(player_id, name, color, &config);
                player.team = team;
                player.leader = leader;
                player.skin_style = skin_style;
                player.is_ai_controlled = is_ai_controlled;
                if let Some(iq) = ghost_iq {
                    player.iq = iq;
                }
                self.state.spawn_player(player, sx, sy);
                log::info!("spawn_human: scripted at ({},{})", sx, sy);
                return;
            }
            log::warn!("spawn_human: no land near scripted spawn ({},{})", tx, ty);
        }

        if !config.random_spawn {
            let mut player = Player::new_human(player_id, name, color, &config);
            player.team = team;
            player.leader = leader;
            player.skin_style = skin_style;
            player.is_ai_controlled = is_ai_controlled;
            if let Some(iq) = ghost_iq {
                player.iq = iq;
            }
            self.state.register_player(player);
            return;
        }

        // OF teamSpawnArea parity (GameImpl.ts:960 + per-map manifests): each
        // team owns a map half (Team {Red, Blue} → Red left, Blue right) and
        // EVERY member spawns inside it — the zone is the cohesion, the
        // member floor is the separation. Ring/anchor path kept as fallback.
        let spawn_point = team
            .as_ref()
            .map(|t| self.team_spawn_area(t))
            .and_then(|area| self.find_spawn_in_area(&mut rng, area))
            .or_else(|| {
                team.as_ref()
                    .and_then(|t| self.team_centroid(t))
                    .and_then(|(cx, cy)| self.find_valid_spawn_near(&mut rng, cx, cy, 12, 36))
            })
            .or_else(|| self.find_valid_spawn(&mut rng));

        if let Some((sx, sy)) = spawn_point {
            let mut player = Player::new_human(player_id, name, color, &config);
            player.team = team;
            player.leader = leader;
            player.skin_style = skin_style;
            player.is_ai_controlled = is_ai_controlled;
            if let Some(iq) = ghost_iq {
                player.iq = iq;
            }
            self.state.spawn_player(player, sx, sy);
        } else {
            log::warn!("Failed to spawn Human {} - no room!", player_id);
        }
    }

    pub(crate) fn find_valid_spawn(&self, rng: &mut wyrand::WyRand) -> Option<(u32, u32)> {
        self.find_spaced_spawn(rng, TRIBE_SPAWN_SPACING)
    }

    /// Random land tile with no owner within `spacing` tiles; the radius
    /// shrinks (2/3, 1/3, 0) as attempts run out so crowded maps still fill.
    pub(crate) fn find_spaced_spawn(
        &self,
        rng: &mut wyrand::WyRand,
        spacing: i32,
    ) -> Option<(u32, u32)> {
        use crate::rng::NextIntExt;
        let mut tries = 0;

        while tries < 1000 {
            let sx = rng.next_int(0, self.state.map.width as i32) as u32;
            let sy = rng.next_int(0, self.state.map.height as i32) as u32;

            if self.state.map.terrain[self.state.map.ref_id(sx, sy)].is_water() {
                tries += 1;
                continue;
            }

            let dist: i32 = if tries < 300 {
                spacing
            } else if tries < 600 {
                spacing * 2 / 3
            } else if tries < 900 {
                spacing / 3
            } else {
                0
            };

            let mut valid = true;

            for dy in -dist..=dist {
                for dx in -dist..=dist {
                    let nx = sx as i32 + dx;
                    let ny = sy as i32 + dy;
                    if self.state.map.is_valid_coord(nx, ny)
                        && self.state.map.owner_id(nx as u32, ny as u32) != 0
                    {
                        valid = false;
                        break;
                    }
                }
                if !valid {
                    break;
                }
            }
            if valid {
                return Some((sx, sy));
            }
            tries += 1;
        }

        // Absolute fallback: scan the map for any land tile (preferably unowned)
        let mut first_land = None;
        for y in 0..self.state.map.height {
            for x in 0..self.state.map.width {
                if self.state.map.terrain[self.state.map.ref_id(x, y)].is_land() {
                    if self.state.map.owner_id(x, y) == 0 {
                        return Some((x, y));
                    }
                    if first_land.is_none() {
                        first_land = Some((x, y));
                    }
                }
            }
        }
        if let Some(pos) = first_land {
            return Some(pos);
        }

        // Extreme fallback: scan the map for any unowned tile
        for y in 0..self.state.map.height {
            for x in 0..self.state.map.width {
                if self.state.map.owner_id(x, y) == 0 {
                    return Some((x, y));
                }
            }
        }

        None
    }

    pub fn spawn_scripted(&mut self) {
        use crate::player::Player;
        let spawns = self.state.config.scripted_spawns.clone();
        if spawns.is_empty() {
            return;
        }
        let config = self.state.config.clone();
        let human_id = self
            .state
            .players
            .iter()
            .find(|player| player.player_type == crate::player::PlayerType::Human)
            .map(|player| player.id);
        let mut placed = 0;
        for s in &spawns {
            let Some(bot_id) = self.state.next_free_player_id() else {
                log::error!("spawn_scripted: player ID space exhausted");
                break;
            };
            let Some((sx, sy)) = self.nearest_free_land(s.x, s.y) else {
                log::warn!(
                    "spawn_scripted: no land near '{}' ({},{})",
                    s.name,
                    s.x,
                    s.y
                );
                continue;
            };
            let mut player = Player::new_bot(bot_id, s.name.clone(), s.color, &config);
            player.team = s.team;
            player.leader = s.leader;
            if let Some(t) = s.troops {
                player.troops = t;
                player.troops_received = t.max(0.0);
            }
            if let Some(iq) = s.iq {
                player.iq = iq;
            }
            self.state.spawn_player(player, sx, sy);
            if let Some(avatar) = &s.campaign_avatar {
                self.campaign_avatars.insert(bot_id, avatar.clone());
            }
            if let Some(interval) = s.campaign_support_interval_seconds {
                self.campaign_support_intervals.insert(bot_id, interval);
            }
            if let Some(gold) = s.campaign_gold_loot_bonus.filter(|gold| *gold <= 1_000_000) {
                self.campaign_gold_loot_bonus.insert(bot_id, gold);
            }
            if let Some(gold) = s
                .campaign_gold_loot_override
                .filter(|gold| *gold <= 1_000_000)
            {
                self.campaign_gold_loot_override.insert(bot_id, gold);
            }
            if let Some(group) = &s.campaign_alliance_group {
                self.campaign_alliance_groups.insert(bot_id, group.clone());
            }
            if let Some(faction_id) = &s.campaign_faction_id {
                self.campaign_faction_ids.insert(bot_id, faction_id.clone());
            }
            if let Some(can_request) = s.campaign_can_request_alliance {
                self.campaign_can_request_alliance
                    .insert(bot_id, can_request);
            }
            if let Some(can_attack_player) = s.campaign_can_attack_player {
                self.campaign_can_attack_player
                    .insert(bot_id, can_attack_player);
            }
            if s.campaign_assault_force {
                self.campaign_assault_force_ids.insert(bot_id);
            }
            if s.campaign_hold_structures_until_contact {
                self.campaign_structure_hold_ids.insert(bot_id);
            }
            if s.campaign_city_requires_gold {
                self.campaign_city_requires_gold_ids.insert(bot_id);
            }
            if s.campaign_accepts_alliance_requests {
                self.campaign_accepts_alliance_requests_ids.insert(bot_id);
            }
            if let Some(relation) = s.campaign_relation {
                self.campaign_relations.insert(bot_id, relation);
                if relation == crate::protocol::CampaignRelation::Allied
                    && let Some(human_id) = human_id
                {
                    if let Some(human) = self.state.player_mut(human_id) {
                        human.alliances.push(bot_id);
                    }
                    if let Some(ally) = self.state.player_mut(bot_id) {
                        ally.alliances.push(human_id);
                    }
                }
            }
            placed += 1;
            log::info!(
                "spawn_scripted: [{}] '{}' team={:?} at ({},{})",
                bot_id,
                s.name,
                s.team,
                sx,
                sy
            );
        }
        log::info!("spawn_scripted: placed {}/{}", placed, spawns.len());
    }

    /// OF teamSpawnArea parity (GameImpl.ts:960 + per-map manifests): each
    /// team owns a map half — `Team {Red, Blue}` → Red left, Blue right — and
    /// every member spawns inside it (2 teams → left/right halves, the same
    /// split OF's manifests hand-author).
    pub(crate) fn team_spawn_area(&self, team: &crate::protocol::Team) -> (u32, u32, u32, u32) {
        let (w, h) = (self.state.map.width, self.state.map.height);
        let half = w / 2;
        match team {
            crate::protocol::Team::Red => (0, 0, half, h),
            crate::protocol::Team::Blue => (half, 0, w - half, h),
        }
    }

    /// Sample free land inside a team's area, keeping the member floor to
    /// every other home. If the floor can't be met the area still wins —
    /// floor is waived and any free land in the zone is taken. Zone cohesion
    /// outranks spacing; spacing never pushes a member out of its zone.
    pub(crate) fn find_spawn_in_area(
        &self,
        rng: &mut wyrand::WyRand,
        area: (u32, u32, u32, u32),
    ) -> Option<(u32, u32)> {
        use crate::rng::NextIntExt;
        let (ax, ay, aw, ah) = area;
        let sample = |rng: &mut wyrand::WyRand| -> Option<(u32, u32)> {
            if aw == 0 || ah == 0 {
                return None;
            }
            let nx = ax + rng.next_int(0, aw as i32) as u32;
            let ny = ay + rng.next_int(0, ah as i32) as u32;
            let ux = nx.min(ax + aw - 1);
            let uy = ny.min(ay + ah - 1);
            if self.state.map.owner_id(ux, uy) != 0
                || !self.state.map.terrain[self.state.map.ref_id(ux, uy)].is_land()
            {
                None
            } else {
                Some((ux, uy))
            }
        };
        for _ in 0..300 {
            if let Some((ux, uy)) = sample(rng)
                && self.home_clear(ux, uy)
            {
                return Some((ux, uy));
            }
        }
        for _ in 0..300 {
            if let Some(free) = sample(rng) {
                return Some(free);
            }
        }
        None
    }

    /// Member floor: ≥14 tiles between any two homes ("cerca sí, encimados
    /// no", widened after the Aug 27 follow-up screenshot — first cut was an
    /// 8-tile floor and blobs still rendered stacked).
    pub(crate) fn home_clear(&self, ux: u32, uy: u32) -> bool {
        const MEMBER_FLOOR_SQ: f64 = 196.0;
        self.state.players.iter().all(|p| {
            if p.tile_count == 0 {
                return true;
            }
            let hx = p.sum_x as f64 / p.tile_count as f64;
            let hy = p.sum_y as f64 / p.tile_count as f64;
            let (dx, dy) = (hx - ux as f64, hy - uy as f64);
            dx * dx + dy * dy >= MEMBER_FLOOR_SQ
        })
    }

    /// OpenFront teamSpawnArea fallback: same-team members cluster around
    /// their first spawned member instead of scattering across the map.
    pub(crate) fn find_valid_spawn_near(
        &self,
        rng: &mut wyrand::WyRand,
        cx: u32,
        cy: u32,
        min_d: i32,
        max_d: i32,
    ) -> Option<(u32, u32)> {
        use crate::rng::NextIntExt;
        for _ in 0..200 {
            let ang = rng.next_int(0, 360) as f64 * std::f64::consts::TAU / 360.0;
            let dist = rng.next_int(min_d, max_d.max(min_d + 1)) as f64;
            let nx = cx as i32 + (ang.cos() * dist).round() as i32;
            let ny = cy as i32 + (ang.sin() * dist).round() as i32;
            if !self.state.map.is_valid_coord(nx, ny) {
                continue;
            }
            let (ux, uy) = (nx as u32, ny as u32);
            if self.state.map.owner_id(ux, uy) != 0
                || !self.state.map.terrain[self.state.map.ref_id(ux, uy)].is_land()
            {
                continue;
            }
            if self.home_clear(ux, uy) {
                return Some((ux, uy));
            }
        }
        None
    }

    /// Centroid of an already-placed team (first member spawns randomly and
    /// becomes the anchor; everyone else rings around it).
    pub(crate) fn team_centroid(&self, team: &crate::protocol::Team) -> Option<(u32, u32)> {
        let mut sum = (0u64, 0u64, 0u64);
        for p in &self.state.players {
            if p.team.as_ref() == Some(team) && p.tile_count > 0 {
                sum.0 += p.sum_x;
                sum.1 += p.sum_y;
                sum.2 += p.tile_count as u64;
            }
        }
        if sum.2 == 0 {
            return None;
        }
        Some(((sum.0 / sum.2) as u32, (sum.1 / sum.2) as u32))
    }

    pub(crate) fn nearest_free_land(&self, tx: u32, ty: u32) -> Option<(u32, u32)> {
        let map = &self.state.map;
        let free = |x: i32, y: i32| -> bool {
            map.is_valid_coord(x, y)
                && map.owner_id(x as u32, y as u32) == 0
                && map.terrain[map.ref_id(x as u32, y as u32)].is_land()
        };
        let (cx, cy) = (tx as i32, ty as i32);
        if free(cx, cy) {
            return Some((tx, ty));
        }
        for r in 1..=120i32 {
            for dx in -r..=r {
                for dy in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    if free(cx + dx, cy + dy) {
                        return Some(((cx + dx) as u32, (cy + dy) as u32));
                    }
                }
            }
        }
        None
    }
}
