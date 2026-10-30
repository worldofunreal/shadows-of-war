// Shape of the menu state published by sow-client/src/web_menu.rs (menu_payload).
// Types only: nothing here is emitted into the bundle. The Rust serializer is the
// source of truth; only the fields the menu reads are described.

export type LocalizedText = string | { key: string; values?: Record<string, unknown> };

export interface MenuLeader {
    id: string;
    slug: string;
    art_slug?: string;
    name?: string;
    civilization?: string;
    civilization_key?: string;
    perk_key?: string;
    region?: string;
    free_rotation?: boolean;
    permanently_free?: boolean;
    owned?: boolean;
    available?: boolean;
    cost_crowns?: number;
    cost_gems?: number;
}

export interface MenuLobby {
    id: number;
    num_players: number;
    max_players: number;
    is_counting_down: boolean;
    timer_secs: number;
    map_name: string;
    game_mode: string;
    players: MenuLobbyPlayer[];
    has_password: boolean;
    host_name: string;
    bot_count: number;
    nation_count: number;
    kind?: string;
    is_private?: boolean;
    bot_difficulty?: string;
}

export interface MenuLobbyPlayer {
    name: string;
    is_ready: boolean;
    download_progress: number;
    leader: string | null;
    player_id: number;
    team: "Red" | "Blue" | null;
}

export interface MenuCampaignEpisode {
    id: string;
    title: LocalizedText;
    subtitle: LocalizedText;
    completed: boolean;
    unlocked: boolean;
    replayable: boolean;
}

export interface MenuCampaign {
    tutorial_completed: boolean;
    episodes: MenuCampaignEpisode[];
    continue_episode: string | null;
}

type MenuConfigValue =
    string | number | boolean | null | MenuConfigValue[] | { [key: string]: MenuConfigValue };

export interface MenuCustomGameConfig {
    [field: string]: MenuConfigValue;
    max_players: number;
    bot_count: number;
    nation_count: number;
    bot_difficulty: string;
    seed: number;
    map_name: string;
    map_roster_preset: string | null;
    game_mode: string;
    map_width: number;
    map_height: number;
    random_spawn: boolean;
}

export interface MenuRewardReceipt {
    id: string;
    match_id: string;
    xp: number;
    leader_xp: number;
    crowns: number;
    laurels: number;
    unlocked_leaders: string[];
    level_before?: number | null;
    level_after?: number | null;
    status: string;
    presented_at?: number | null;
    verification_status?: string;
}

export interface MenuReturn {
    account_id: string;
    cycle_id: number;
    source: string;
    match_id: string;
    requested_screen: string | null;
    status: string;
    receipt_ids: string[];
    ranked_result?: RankedResult | null;
    ranked_result_presented?: boolean;
}

export interface RankedStep {
    score: number;
    tier: string;
    division: string | null;
}

export interface RankedLadder {
    steps: RankedStep[];
    max_score: number;
}

export interface RankedResult {
    ladder?: RankedLadder | null;
    score_before: number;
    score_after: number;
    rating_delta: number;
    tier_before: string;
    tier_after: string;
    division_before: string | null;
    division_after: string | null;
    games_played: number;
    placements_complete: boolean;
    points_breakdown?: RankedPointsBreakdown | null;
}

export interface RankedPointsBreakdown {
    base: number;
    opponent: number;
    league: number;
    streak: number;
    protection: number;
    limit: number;
    protected_loss: boolean;
}

export interface MenuLeaderOffer {
    id: string;
    owned: boolean;
    free_rotation: boolean;
    permanently_free: boolean;
    available: boolean;
    cost_crowns: number;
    cost_gems: number;
}

export interface MenuSkinOffer {
    id: string;
    name: string;
    owned: boolean;
    cost_gems: number;
    asset_path: string;
}

export interface MenuGemBundle {
    id: string;
    product_id: string;
    gems: number;
    asset_path: string;
    bonus_percent: number;
}

export interface MenuStore {
    crowns: number;
    gems: number;
    rotation_period: number;
    rotation_ends_at: number;
    free_leaders: string[];
    leaders: MenuLeaderOffer[];
    skins: MenuSkinOffer[];
    gem_bundles: MenuGemBundle[];
}

export interface MenuSettings {
    mute_all: boolean;
    music_volume: number;
    reduced_motion: boolean;
    free_zoom_out: boolean;
    show_fps_ping: boolean;
    show_dev_tools: boolean;
    show_dev_tools_available: boolean;
}

export interface MenuMap {
    key: string;
    display_name: string;
    width: number;
    height: number;
    default_roster?: string | null;
    roster_presets?: { id: string; name: string }[];
}

export interface MenuProfileStats {
    wins: number;
    matches_played: number;
    players_defeated: number;
    empires_defeated: number;
    tribes_defeated: number;
    kills: number;
    deaths: number;
    assists: number;
    leader_xp: number;
}

export interface MenuState {
    phase: string;
    loader_cycle_id: number;
    loader_job: string;
    loader_leader: string | null;
    loader_info_leader: string;
    loader_show_hero_briefing: boolean;
    loader_progress: number;
    loader_done: boolean;
    boot_campaign: string | null;
    connected: boolean;
    connecting: boolean;
    update_available: boolean;
    waiting: boolean;
    ranked_matchup?: MenuLobby | null;
    ranked_search_active?: boolean;
    ranked_search_elapsed_secs?: number;
    match_preparing: boolean;
    player_name: string;
    selected_leader: string;
    selected_leader_name: string;
    selected_civilization: string;
    prefer_random_leader: boolean;
    show_browser: boolean;
    show_create: boolean;
    join_lobby_code: string;
    joined_lobby_id: number | null;
    pending_lobby_id: number | null;
    is_lobby_host: boolean;
    my_player_id: number | null;
    downloading_map: boolean;
    map_download_progress: number;
    map_download_error: boolean;
    lobbies: MenuLobby[];
    error: LocalizedText | null;
    notice: string | null;
    level: number;
    xp: number;
    crowns: number;
    laurels: number;
    gems: number;
    reward_receipts: MenuRewardReceipt[];
    menu_return: MenuReturn | null;
    profile_ready: boolean;
    menu_guides: unknown;
    name_save_pending: boolean;
    quick_match_revision: number;
    menu_guide_save_pending: boolean;
    achievements: unknown;
    campaign: MenuCampaign;
    selected_skin: string;
    store_busy: boolean;
    store: MenuStore;
    native_purchase_scheme: string;
    native_restore_scheme: string;
    account_id: string | null;
    profile_stats: MenuProfileStats;
    leaders: MenuLeader[];
    map_catalog: MenuMap[];
    custom_game_config: MenuCustomGameConfig;
    bot_limits: { tribes: number; nations: number };
    custom_game_is_private: boolean;
    custom_game_is_sp: boolean;
    settings: MenuSettings;
}

export interface HudAvatarIdentity {
    kind: "portrait" | "wilderness" | "fallback";
    slug?: string;
    expression?: "happy" | "calm" | "fear" | "angry";
    transition?: HudAvatarTransition | null;
}

export interface HudAvatarTransition {
    id: number;
    from: "happy" | "calm" | "fear" | "angry";
    to: "happy" | "calm" | "fear" | "angry";
    stage: "exit" | "enter";
}

export interface HudLeaderboardPlayer {
    id: number;
    name: string;
    troops: number;
    max_troops: number;
    tile_count: number;
    centroid_x: number;
    centroid_y: number;
    territory_pct: number;
    is_alive: boolean;
    is_me: boolean;
    leader: string;
    avatar: string | null;
    avatar_expression?: "happy" | "calm" | "fear" | "angry" | null;
    avatar_transition?: HudAvatarTransition | null;
    manual_emote_active?: boolean;
    team: string | null;
    active_emote: { kind: string; pinned: boolean; scripted: boolean; message?: string } | null;
    disconnected: boolean;
    traitor: boolean;
    kills: number;
    deaths: number;
    assists: number;
    rank?: number;
    cities?: number;
    factories?: number;
    ports?: number;
    towers?: number;
    farms?: number;
    campaign_faction_id?: string;
}

export interface HudLeaderboardTeam {
    team: "Red" | "Blue";
    tile_count: number;
    territory_pct: number;
    members: number;
}

export interface HudMapMenuItem {
    action: string;
    cost: number | null;
    level: number | null;
    disabled: boolean;
    reason_key: string | null;
    reason_level: number | null;
    reason_seconds: number | null;
    reason_troops: number | null;
}

export interface HudBuildingChip {
    icon: string;
    label: string;
}

export interface HudBuildingUpgradeOffer {
    next_level: number;
    cost: number;
    seconds: number;
    enabled: boolean;
}

export interface HudMapMenuBuilding {
    kind: string;
    level: number;
    max_level?: number;
    under_construction?: boolean;
    anchor_x?: number;
    anchor_y?: number;
    anchor_radius?: number;
    remaining_seconds?: number | null;
    progress?: number | null;
    chips?: HudBuildingChip[];
    upgrade?: HudBuildingUpgradeOffer | null;
}

export interface HudBuildingPlacement {
    pending: boolean;
    can_confirm: boolean;
    kind?: string;
    tile_idx?: number;
    anchor_x?: number;
    anchor_y?: number;
    arrows?: {
        up: [number, number];
        right: [number, number];
        down: [number, number];
        left: [number, number];
    };
    cost?: number;
    reason?: string | null;
}

export interface HudBuildingInProgress {
    kind: string;
    level: number;
    remaining_seconds: number;
    progress: number;
}

export interface HudBuildingProgressItem {
    tile_idx: number;
    kind: string;
    level: number;
    active_level?: number;
    remaining_seconds: number;
    progress: number;
    screen_x: number;
    screen_y: number;
    visible?: boolean;
}

export interface HudBuildingCelebrationItem {
    tile_idx: number;
    kind: string;
    level: number;
    screen_x: number;
    screen_y: number;
    age_ms: number;
    visible?: boolean;
}

export interface HudMapMenu {
    open?: boolean;
    view?: "building_details" | "radial";
    x?: number;
    y?: number;
    tile_idx?: number;
    target_player_id?: number;
    session?: number;
    actions?: string[];
    items?: HudMapMenuItem[];
    alliance_state?: string | null;
    relation?: string | null;
    target_name?: string | null;
    target_avatar?: HudAvatarIdentity | null;
    building?: HudMapMenuBuilding | null;
}

export interface HudNotification {
    id: number;
    key: string | null;
    values: Record<string, string> | null;
    avatars: (HudAvatarIdentity | null)[];
    priority: number;
    group: string | null;
    sum_values: boolean;
    age_ms: number;
}

export interface HudMapFeedback {
    id: number;
    key: string | null;
    values: Record<string, string> | null;
    x: number;
    y: number;
    age_ms: number;
    avatars?: never;
}

export interface HudInboxRequest {
    kind: "alliance" | "resources";
    requester_id: number;
    name: string;
    avatar: HudAvatarIdentity;
    active: boolean;
    gold?: number;
    troops?: number;
}

export interface HudCombatOperation {
    kind: "attack" | "fleet";
    id: number;
    direction: "incoming" | "outgoing";
    player_id: number;
    name: string | null;
    avatar: HudAvatarIdentity;
    neutral: boolean;
    troops: number;
    eta_seconds: number | null;
    retreating: boolean;
    focus_x: number | null;
    focus_y: number | null;
    incoming_troops?: number;
}

export interface HudDevParameter {
    id: string;
    label: string;
    section: string;
    kind: "number" | "bool" | "color" | "vector" | "select";
    unit: string;
    alpha: boolean;
    min: number;
    max: number;
    step: number;
    default: boolean | number | number[];
    options: { label: string; value: number }[];
}

export interface HudDevTools {
    open: boolean;
    config?: Record<string, boolean | number | number[]>;
    schema?: HudDevParameter[];
    groups?: { name: string; sections: { name: string; preview: number | null }[] }[];
    perf?: { name: string; state: "on" | "off" | "mixed" }[];
    preview?: {
        target: number | null;
        phase: number;
        paused: boolean;
        repeat: boolean;
        active: boolean;
        playing: boolean;
    };
}

export interface HudTutorial {
    active: boolean;
    episode_id?: string;
    campaign_session_id?: number;
    effective_unlocks?: { buildings: Record<string, number>; actions: string[] };
    tick?: number;
    pointer?: { x: number; y: number; [key: string]: number | boolean | undefined } | null;
}

export interface MenuHudPayload {
    camera_zoom_state: { current: number; floor: number; ceiling: number };
    gold: number;
    troops: number;
    max_troops: number;
    troop_rate: number;
    gold_rate: number;
    attack_ratio: number;
    operations: HudCombatOperation[];
    spawn_timer_secs: number | null;
    selected_building: string | null;
    building_placement: HudBuildingPlacement | null;
    building_in_progress?: HudBuildingInProgress | null;
    buildings_in_progress?: HudBuildingProgressItem[];
    building_celebrations?: HudBuildingCelebrationItem[];
    selected_warship_build: boolean;
    selected_nuke: string | null;
    fleet_panel: {
        trade_ships: number;
        trade_capacity: number;
        port_levels: number;
        city_level: number;
        trade_required_port_level: number;
        warship_required_port_level: number;
        nuke_required_city_level: number;
        warship_cost: number;
        nuke_cost: number;
        nuke_available: boolean;
        nuke_cooldown_ticks: number;
        military_used: number;
        military_capacity: number;
    };
    building_costs: Record<string, number>;
    emotes: {
        kind: string;
        expression: string;
        menu_key: string;
        bubble_key: string;
        notice_key: string;
        fallback: string;
    }[];
    emote_active: boolean;
    emote_pinned: boolean;
    fps: number | null;
    ping: number | null;
    inbox_count: number;
    match_over: boolean;
    is_spectating: boolean;
    tutorial: HudTutorial;
    notifications: HudNotification[];
    map_feedback: HudMapFeedback | null;
    map_menu: HudMapMenu | null;
    dev_tools: HudDevTools;
    is_winner: boolean;
    winner_name: string;
    featured_skin: { id: string; name: string; asset_path: string; cost_gems: number } | null;
    player_leader: string | null;
    player_avatar_expression?: HudAvatarIdentity["expression"] | null;
    player_avatar_transition?: HudAvatarTransition | null;
    player_name: string;
    player_endgame_stats?: {
        kills?: number;
        assists?: number;
        max_territory?: number;
        gold_received?: number;
        troops_received?: number;
    };
    players?: HudLeaderboardPlayer[];
    leaderboard?: HudLeaderboardPlayer[];
    team_leaderboard?: HudLeaderboardTeam[];
    inbox?: HudInboxRequest[];
    transfer?: {
        target_id: number;
        target_name: string;
        target_avatar: HudAvatarIdentity;
        target_alive: boolean;
        confirm_pending: boolean;
        suggested_gold: number;
        suggested_troops: number;
    } | null;
    betrayal?: { ally_id: number; ally_name: string; ally_avatar: HudAvatarIdentity } | null;
    sync?: { [key: string]: string | number | boolean | null } | null;
    hovered?:
        | (HudLeaderboardPlayer & {
              gold?: number;
              cities: number;
              factories: number;
              ports: number;
              towers: number;
              farms: number;
          })
        | null;
}

export interface HudStateEnvelope {
    phase: string;
    hud?: MenuHudPayload | null;
    settings?: MenuSettings;
    selected_leader?: string;
    leaders?: MenuLeader[];
}
