import { avatarImageAttribute } from "./avatar-presentation.ts";
import {
    asset,
    currencyAsset,
    esc,
    leaderArtSlug,
    leaderById,
    leaderCivilization,
    leaderDisplayName,
    menuUi,
} from "./main_menu.core.ts";
import type { MenuLeader } from "./main_menu.state.ts";

export interface ProfileAchievement {
    id: string;
    category: string;
    title?: string;
    description?: string;
    points: number;
    progress: number;
    target: number;
    unlocked: boolean;
}

export interface AchievementProfileLeader {
    leader: string;
    matches_played?: number;
    wins?: number;
    xp?: number;
}

export interface AchievementProfileInput {
    account_id?: string;
    matches_played?: number;
    wins?: number;
    kills?: number;
    assists?: number;
    players_defeated?: number;
    empires_defeated?: number;
    tribes_defeated?: number;
    laurels?: number;
    achievements?: ProfileAchievement[];
    leaders?: AchievementProfileLeader[];
}

export type WowCategoryId =
    | "summary"
    | "general"
    | "pvp_combat"
    | "pve_campaign"
    | "heroes_mastery"
    | "prestige"
    | "all";

export type WowConcretePillar = Exclude<WowCategoryId, "summary" | "all">;

export type AchievementTierLevel = "mythic" | "elite" | "standard";

export type HeroOwnershipStatus = "owned" | "weekly_free" | "starter" | "locked";

export interface EnrichedAchievement extends ProfileAchievement {
    percent: number;
    isMythic: boolean;
    tierLevel: AchievementTierLevel;
    wowCategory: WowConcretePillar;
    wowSubcategory: string;
    chainKey: string | null;
    chainTitleKey: string | null;
    chainFallbackTitle: string | null;
    heroSlug: string | null;
    heroRegion: string | null;
    heroKind: "recruit" | "mastery" | "wins" | null;
    heroPlayable: boolean;
    localizedTitle: string;
    localizedDescription: string;
}

export interface HeroAchievementSummary {
    slug: string;
    leader: MenuLeader | null;
    name: string;
    civilization: string;
    region: string;
    rarity: string;
    ownershipStatus: HeroOwnershipStatus;
    playable: boolean;
    achievements: EnrichedAchievement[];
    recruitAchievement: EnrichedAchievement | null;
    masteryAchievements: EnrichedAchievement[];
    winsAchievements: EnrichedAchievement[];
    unlockedCount: number;
    totalCount: number;
    earnedLaurels: number;
    totalLaurels: number;
    percent: number;
    nextAchievement: EnrichedAchievement | null;
    nextPercent: number;
    masteryRankKey: string;
    matchesPlayed: number;
    wins: number;
    kills: number;
    deaths: number;
    assists: number;
    xp: number;
}

const ALL_HEROES_FALLBACK: { slug: string; name: string }[] = [
    { slug: "caesar", name: "Caesar" },
    { slug: "cleopatra", name: "Cleopatra" },
    { slug: "ragnar", name: "Ragnar" },
    { slug: "sun_tzu", name: "Sun Tzu" },
    { slug: "alexander", name: "Alexander" },
    { slug: "genghis_khan", name: "Genghis Khan" },
    { slug: "richard_the_lionheart", name: "Richard the Lionheart" },
    { slug: "vercingetorix", name: "Vercingetorix" },
    { slug: "boudica", name: "Boudica" },
    { slug: "lady_six_sky", name: "Lady Six Sky" },
    { slug: "leonidas", name: "Leonidas" },
    { slug: "napoleon", name: "Napoleon" },
    { slug: "amina_de_zazzau", name: "Amina of Zazzau" },
    { slug: "yaa_asantewaa", name: "Yaa Asantewaa" },
    { slug: "anacaona", name: "Anacaona" },
    { slug: "lozen", name: "Lozen" },
    { slug: "tipu_sultan", name: "Tipu Sultan" },
    { slug: "william_the_conqueror", name: "William the Conqueror" },
    { slug: "hongi_hika", name: "Hongi Hika" },
    { slug: "salote_tupou_iii", name: "Sālote Tupou III" },
    { slug: "makeda", name: "Makeda" },
    { slug: "rani_durgavati", name: "Rani Durgavati" },
    { slug: "empress_gi", name: "Empress Gi" },
    { slug: "ashoka", name: "Ashoka" },
    { slug: "hannibal", name: "Hannibal" },
    { slug: "bolivar", name: "Simón Bolívar" },
    { slug: "joan_of_arc", name: "Joan of Arc" },
    { slug: "catherine_the_great", name: "Catherine the Great" },
    { slug: "isabella_of_castile", name: "Isabella I of Castile" },
    { slug: "cyrus_the_great", name: "Cyrus the Great" },
    { slug: "saladin", name: "Saladin" },
    { slug: "suleiman", name: "Suleiman the Magnificent" },
    { slug: "tomoe_gozen", name: "Tomoe Gozen" },
    { slug: "trieu_thi_trinh", name: "Lady Triệu" },
    { slug: "musa_i", name: "Mansa Musa" },
    { slug: "shaka", name: "Shaka" },
    { slug: "nzinga", name: "Nzinga" },
    { slug: "kamehameha", name: "Kamehameha I" },
    { slug: "gajah_mada", name: "Gajah Mada" },
    { slug: "solomon", name: "Solomon" },
    { slug: "sayyid_said", name: "Sayyid Said" },
    { slug: "lapu_lapu", name: "Lapu-Lapu" },
    { slug: "constantine", name: "Constantine" },
    { slug: "vlad_iii", name: "Vlad III" },
    { slug: "mary_of_burgundy", name: "Mary of Burgundy" },
    { slug: "semiramis", name: "Semiramis" },
    { slug: "shima_kalingga", name: "Shima" },
    { slug: "tomyris", name: "Tomyris" },
    { slug: "gorgo", name: "Gorgo" },
    { slug: "artigas", name: "Artigas" },
    { slug: "bernardo_ohiggins", name: "Bernardo O'Higgins" },
    { slug: "nezahualcoyotl", name: "Nezahualcóyotl" },
    { slug: "tlacaelel", name: "Tlacaelel" },
    { slug: "tenamaxtli", name: "Tenamaxtli" },
    { slug: "cosijoeza", name: "Cosijoeza" },
    { slug: "mangas_coloradas", name: "Mangas Coloradas" },
    { slug: "tomochichi", name: "Tomochichi" },
    { slug: "eight_deer", name: "Eight Deer" },
    { slug: "nemequene", name: "Nemequene" },
    { slug: "thanadelthur", name: "Thanadelthur" },
    { slug: "artemisia_i", name: "Artemisia I" },
    { slug: "shajar_al_durr", name: "Shajar al-Durr" },
    { slug: "ramesses_ii", name: "Ramesses II" },
    { slug: "qin_liangyu", name: "Qin Liangyu" },
    { slug: "lobengula", name: "Lobengula" },
    { slug: "shah_jahan", name: "Shah Jahan" },
    { slug: "xerxes_i", name: "Xerxes" },
    { slug: "marcus_aurelius", name: "Marcus Aurelius" },
    { slug: "justinian_ii_hero", name: "Justinian II" },
    { slug: "totila", name: "Totila" },
    { slug: "chilperic_i", name: "Chilperic I" },
    { slug: "lothair_i", name: "Lothair I" },
    { slug: "frederick_barbarossa", name: "Barbarossa" },
    { slug: "charles_v", name: "Charles V" },
    { slug: "wanli", name: "Wanli" },
    { slug: "jayaatu_khan", name: "Jayaatu Khan" },
    { slug: "sitting_bull", name: "Sitting Bull" },
    { slug: "matoaka", name: "Matoaka" },
    { slug: "burebista", name: "Burebista" },
    { slug: "boleslaw_ii", name: "Bolesław II" },
    { slug: "abd_al_rahman_al_ghafiqi", name: "Abd al-Rahman al-Ghafiqi" },
    { slug: "tribe_wiradjuri", name: "Windradyne" },
    { slug: "nation_yagan", name: "Yagan" },
    { slug: "peter_the_great", name: "Peter the Great" },
    { slug: "empress_suiko", name: "Empress Suiko" },
    { slug: "nebuchadnezzar_ii", name: "Nebuchadnezzar II" },
    { slug: "king_david", name: "King David" },
    { slug: "sarraounia_mangou", name: "Sarraounia Mangou" },
    { slug: "muhammad_of_ghor", name: "Muhammad of Ghor" },
    { slug: "puyi", name: "Puyi" },
    { slug: "pertinax", name: "Pertinax" },
    { slug: "ewuare", name: "Ewuare the Great" },
    { slug: "veleda", name: "Veleda" },
];

export const MASTERY_TIERS: [number, number, string][] = [
    [3, 15, "Vanguard"],
    [10, 30, "Centurion"],
    [25, 50, "Veteran"],
    [50, 75, "Champion"],
    [100, 100, "Warlord"],
    [250, 200, "Warmaster"],
    [500, 350, "Juggernaut"],
    [1000, 1000, "Ascendant"],
];

export const WIN_TIERS: [number, number, string][] = [
    [1, 25, "Victor"],
    [5, 50, "Triumph"],
    [25, 100, "Conqueror"],
    [100, 200, "Overlord"],
    [500, 500, "Imperator"],
    [1000, 1000, "Sovereign"],
];

const SPECIAL_HERO_MASTERY_TITLES: Record<string, Record<number, string>> = {
    boudica: {
        3: "Wildfire",
        10: "Fury",
        25: "Iceni",
        50: "Uprising",
        100: "Vengeance",
        250: "War Queen",
        500: "Blood Mother",
        1000: "Nemesis",
    },
    caesar: {
        3: "Tribune",
        10: "Centurion",
        25: "Legatus",
        50: "Rubicon",
        100: "Imperium",
        250: "Dictator",
        500: "Colossus",
        1000: "Veni Vidi Vici",
    },
    lady_six_sky: {
        3: "Eclipse",
        10: "Sun Priest",
        25: "Starlight",
        50: "Wikal",
        100: "Cosmos",
        250: "Serpent",
        500: "Sky Empress",
        1000: "Ascension",
    },
    alexander: {
        3: "Phaistos",
        10: "Phalanx",
        25: "Gordian",
        50: "Issus",
        100: "Alexandria",
        250: "Hegemon",
        500: "Amun",
        1000: "World's End",
    },
    genghis_khan: {
        3: "Rider",
        10: "Steppe Wolf",
        25: "Karakorum",
        50: "Golden Horde",
        100: "Tengri",
        250: "Great Khan",
        500: "Earth Shaker",
        1000: "Scourge of God",
    },
    leonidas: {
        3: "Spartiate",
        10: "Shield Wall",
        25: "Molon Labe",
        50: "Hot Gates",
        100: "Thermopylae",
        250: "Lion of Sparta",
        500: "Immortal Bane",
        1000: "Last Stand",
    },
    sun_tzu: {
        3: "Strategist",
        10: "The Art of War",
        25: "Subtle Dragon",
        50: "Silent Wind",
        100: "Formless",
        250: "Grand Tactician",
        500: "Supreme Art",
        1000: "Master of War",
    },
    cleopatra: {
        3: "Nile Lotus",
        10: "Pharaoh's Grace",
        25: "Alexandrine",
        50: "Siren of Kings",
        100: "Ptolemaic Throne",
        250: "Queen of Kings",
        500: "Isis Reborn",
        1000: "Eternal Nile",
    },
    hannibal: {
        3: "Barca",
        10: "Alpine March",
        25: "Cannae",
        50: "Double Envelopment",
        100: "Rome's Terror",
        250: "Carthaginian Lion",
        500: "Ad Portas",
        1000: "Thunderbolt",
    },
    napoleon: {
        3: "Corsican",
        10: "Artillery Officer",
        25: "Austerlitz",
        50: "Sun of Austerlitz",
        100: "Grande Armée",
        250: "First Consul",
        500: "Emperor of France",
        1000: "Eagle of Destiny",
    },
};

export function getSpecialHeroMasteryTitle(slug: string, target: number): string | null {
    return (SPECIAL_HERO_MASTERY_TITLES[slug] && SPECIAL_HERO_MASTERY_TITLES[slug][target]) || null;
}

const GLOBAL_ACHIEVEMENTS: ProfileAchievement[] = [
    // 9 Canonical Google Play / Core achievements
    { id: "first_command", category: "battles", title: "First Command", description: "Complete your first match.", points: 10, target: 1, unlocked: false, progress: 0 },
    { id: "first_victory", category: "victories", title: "First Victory", description: "Win a verified match.", points: 25, target: 1, unlocked: false, progress: 0 },
    { id: "battle_hardened", category: "battles", title: "Battle Hardened", description: "Complete 10 matches.", points: 25, target: 10, unlocked: false, progress: 0 },
    { id: "victory_march", category: "victories", title: "Victory March", description: "Win 10 verified matches.", points: 50, target: 10, unlocked: false, progress: 0 },
    { id: "laurel_hoard", category: "laurels", title: "Laurel Hoard", description: "Earn 100 achievement points.", points: 50, target: 100, unlocked: false, progress: 0 },
    { id: "commander_victorious", category: "victories", title: "Commander Victorious", description: "Win with a commander.", points: 25, target: 1, unlocked: false, progress: 0 },
    { id: "veteran_commander", category: "commanders", title: "Veteran Commander", description: "Complete 10 matches with commanders.", points: 50, target: 10, unlocked: false, progress: 0 },
    { id: "banner_collector", category: "commanders", title: "Banner Collector", description: "Command three different leaders.", points: 50, target: 3, unlocked: false, progress: 0 },
    { id: "leader_path", category: "commanders", title: "Leader Path", description: "Earn 1,000 leader experience.", points: 100, target: 1000, unlocked: false, progress: 0 },

    // Tiered Battles Matrix
    { id: "battles_3", category: "battles", title: "Vanguard", description: "Complete 3 matches.", points: 15, target: 3, unlocked: false, progress: 0 },
    { id: "battles_25", category: "battles", title: "Ironclad", description: "Complete 25 matches.", points: 50, target: 25, unlocked: false, progress: 0 },
    { id: "battles_50", category: "battles", title: "Centurion", description: "Complete 50 matches.", points: 75, target: 50, unlocked: false, progress: 0 },
    { id: "battles_100", category: "battles", title: "Warlord", description: "Complete 100 matches.", points: 100, target: 100, unlocked: false, progress: 0 },
    { id: "battles_250", category: "battles", title: "Warmaster", description: "Complete 250 matches.", points: 200, target: 250, unlocked: false, progress: 0 },
    { id: "battles_500", category: "battles", title: "Juggernaut", description: "Complete 500 matches.", points: 350, target: 500, unlocked: false, progress: 0 },
    { id: "battles_1000", category: "battles", title: "Oblivion", description: "Complete 1,000 matches.", points: 1000, target: 1000, unlocked: false, progress: 0 },

    // Tiered Victories Matrix
    { id: "victories_3", category: "victories", title: "Blood Oath", description: "Win 3 verified matches.", points: 30, target: 3, unlocked: false, progress: 0 },
    { id: "victories_25", category: "victories", title: "Triumph", description: "Win 25 verified matches.", points: 75, target: 25, unlocked: false, progress: 0 },
    { id: "victories_50", category: "victories", title: "Conqueror", description: "Win 50 verified matches.", points: 100, target: 50, unlocked: false, progress: 0 },
    { id: "victories_100", category: "victories", title: "Overlord", description: "Win 100 verified matches.", points: 150, target: 100, unlocked: false, progress: 0 },
    { id: "victories_250", category: "victories", title: "Dominion", description: "Win 250 verified matches.", points: 250, target: 250, unlocked: false, progress: 0 },
    { id: "victories_500", category: "victories", title: "Imperator", description: "Win 500 verified matches.", points: 500, target: 500, unlocked: false, progress: 0 },
    { id: "victories_1000", category: "victories", title: "Sovereign", description: "Win 1,000 verified matches.", points: 1000, target: 1000, unlocked: false, progress: 0 },

    // Combat Supremacy
    { id: "kills_10", category: "combat", title: "First Blood", description: "Defeat 10 enemy units.", points: 20, target: 10, unlocked: false, progress: 0 },
    { id: "kills_25", category: "combat", title: "Skirmisher", description: "Defeat 25 enemy units.", points: 35, target: 25, unlocked: false, progress: 0 },
    { id: "kills_50", category: "combat", title: "Reaper", description: "Defeat 50 enemy units.", points: 50, target: 50, unlocked: false, progress: 0 },
    { id: "kills_100", category: "combat", title: "Slayer", description: "Defeat 100 enemy units.", points: 75, target: 100, unlocked: false, progress: 0 },
    { id: "kills_250", category: "combat", title: "Executioner", description: "Defeat 250 enemy units.", points: 150, target: 250, unlocked: false, progress: 0 },
    { id: "kills_500", category: "combat", title: "Warbringer", description: "Defeat 500 enemy units.", points: 250, target: 500, unlocked: false, progress: 0 },
    { id: "kills_1000", category: "combat", title: "Death Incarnate", description: "Defeat 1,000 enemy units.", points: 500, target: 1000, unlocked: false, progress: 0 },
    { id: "kills_2500", category: "combat", title: "Doom of Empires", description: "Defeat 2,500 enemy units.", points: 750, target: 2500, unlocked: false, progress: 0 },
    { id: "kills_5000", category: "combat", title: "Lord of the Grave", description: "Defeat 5,000 enemy units.", points: 1000, target: 5000, unlocked: false, progress: 0 },
    { id: "assists_10", category: "combat", title: "Wingman", description: "Record 10 combat assists.", points: 15, target: 10, unlocked: false, progress: 0 },
    { id: "assists_50", category: "combat", title: "Vanguard Shield", description: "Record 50 combat assists.", points: 50, target: 50, unlocked: false, progress: 0 },
    { id: "assists_250", category: "combat", title: "Tactical Hand", description: "Record 250 combat assists.", points: 150, target: 250, unlocked: false, progress: 0 },
    { id: "assists_1000", category: "combat", title: "Master of Cohorts", description: "Record 1,000 combat assists.", points: 500, target: 1000, unlocked: false, progress: 0 },
    { id: "players_5", category: "combat", title: "Duelist Initiate", description: "Eliminate 5 rival commanders.", points: 15, target: 5, unlocked: false, progress: 0 },
    { id: "players_10", category: "combat", title: "Duelist", description: "Eliminate 10 rival commanders.", points: 30, target: 10, unlocked: false, progress: 0 },
    { id: "players_25", category: "combat", title: "Champion Slayer", description: "Eliminate 25 rival commanders.", points: 50, target: 25, unlocked: false, progress: 0 },
    { id: "players_50", category: "combat", title: "Crown Breaker", description: "Eliminate 50 rival commanders.", points: 100, target: 50, unlocked: false, progress: 0 },
    { id: "players_100", category: "combat", title: "Sovereign Hunter", description: "Eliminate 100 rival commanders.", points: 200, target: 100, unlocked: false, progress: 0 },
    { id: "players_250", category: "combat", title: "Kingmaker", description: "Eliminate 250 rival commanders.", points: 500, target: 250, unlocked: false, progress: 0 },
    { id: "empires_5", category: "combat", title: "Border Raider", description: "Overthrow 5 rival empires.", points: 25, target: 5, unlocked: false, progress: 0 },
    { id: "empires_10", category: "combat", title: "Empire Slayer", description: "Overthrow 10 rival empires.", points: 50, target: 10, unlocked: false, progress: 0 },
    { id: "empires_25", category: "combat", title: "Dynasty Fall", description: "Overthrow 25 rival empires.", points: 100, target: 25, unlocked: false, progress: 0 },
    { id: "empires_50", category: "combat", title: "World Breaker", description: "Overthrow 50 rival empires.", points: 250, target: 50, unlocked: false, progress: 0 },
    { id: "empires_100", category: "combat", title: "Scourge of Realms", description: "Overthrow 100 rival empires.", points: 500, target: 100, unlocked: false, progress: 0 },
    { id: "tribes_10", category: "combat", title: "Chieftain Bane", description: "Subjugate 10 native tribes.", points: 25, target: 10, unlocked: false, progress: 0 },
    { id: "tribes_25", category: "combat", title: "Subjugator", description: "Subjugate 25 native tribes.", points: 50, target: 25, unlocked: false, progress: 0 },
    { id: "tribes_50", category: "combat", title: "Steppe Dominator", description: "Subjugate 50 native tribes.", points: 100, target: 50, unlocked: false, progress: 0 },
    { id: "tribes_100", category: "combat", title: "Overlord of the Wild", description: "Subjugate 100 native tribes.", points: 250, target: 100, unlocked: false, progress: 0 },

    // Campaign Chronicles
    { id: "campaign_1", category: "campaign", title: "Pathfinder", description: "Complete 1 campaign episode.", points: 25, target: 1, unlocked: false, progress: 0 },
    { id: "campaign_3", category: "campaign", title: "Trailblazer", description: "Complete 3 campaign episodes.", points: 35, target: 3, unlocked: false, progress: 0 },
    { id: "campaign_5", category: "campaign", title: "Saga Victor", description: "Complete 5 campaign episodes.", points: 50, target: 5, unlocked: false, progress: 0 },
    { id: "campaign_10", category: "campaign", title: "Epic Historian", description: "Complete 10 campaign episodes.", points: 75, target: 10, unlocked: false, progress: 0 },
    { id: "campaign_15", category: "campaign", title: "Chronicles of War", description: "Complete 15 campaign episodes.", points: 100, target: 15, unlocked: false, progress: 0 },
    { id: "campaign_25", category: "campaign", title: "Grand Campaigner", description: "Complete 25 campaign episodes.", points: 150, target: 25, unlocked: false, progress: 0 },
    { id: "campaign_40", category: "campaign", title: "Master of History", description: "Complete 40 campaign episodes.", points: 250, target: 40, unlocked: false, progress: 0 },

    // Laurels & Prestige
    { id: "laurels_250", category: "laurels", title: "Gilded Crown", description: "Earn 250 achievement points.", points: 50, target: 250, unlocked: false, progress: 0 },
    { id: "laurels_500", category: "laurels", title: "Imperial Grace", description: "Earn 500 achievement points.", points: 100, target: 500, unlocked: false, progress: 0 },
    { id: "laurels_1000", category: "laurels", title: "Vault of Glory", description: "Earn 1,000 achievement points.", points: 200, target: 1000, unlocked: false, progress: 0 },
    { id: "laurels_2500", category: "laurels", title: "Golden Emperor", description: "Earn 2,500 achievement points.", points: 500, target: 2500, unlocked: false, progress: 0 },
    { id: "laurels_5000", category: "laurels", title: "Immortal Renown", description: "Earn 5,000 achievement points.", points: 1000, target: 5000, unlocked: false, progress: 0 },
    { id: "laurels_10000", category: "laurels", title: "Demigod of War", description: "Earn 10,000 achievement points.", points: 2500, target: 10000, unlocked: false, progress: 0 },

    // Hero Roster Milestones
    { id: "roster_1", category: "commanders", title: "First Recruit", description: "Command or own 1 hero.", points: 25, target: 1, unlocked: false, progress: 0 },
    { id: "roster_3", category: "commanders", title: "Cohort", description: "Command or own 3 heroes.", points: 50, target: 3, unlocked: false, progress: 0 },
    { id: "roster_5", category: "commanders", title: "Council of War", description: "Command or own 5 heroes.", points: 75, target: 5, unlocked: false, progress: 0 },
    { id: "roster_10", category: "commanders", title: "Grand Legion", description: "Command or own 10 heroes.", points: 100, target: 10, unlocked: false, progress: 0 },
    { id: "roster_25", category: "commanders", title: "High Command", description: "Command or own 25 heroes.", points: 200, target: 25, unlocked: false, progress: 0 },
    { id: "roster_50", category: "commanders", title: "Army of Legends", description: "Command or own 50 heroes.", points: 350, target: 50, unlocked: false, progress: 0 },
    { id: "roster_80", category: "commanders", title: "Pantheon of War", description: "Command or own 80 heroes.", points: 1000, target: 80, unlocked: false, progress: 0 },
];

export const WOW_PILLAR_ORDER: WowConcretePillar[] = [
    "general",
    "pvp_combat",
    "pve_campaign",
    "heroes_mastery",
    "prestige",
];

export const WOW_SUBCATEGORIES: Record<WowConcretePillar, string[]> = {
    general: ["all", "battles", "victories", "vanguard"],
    pvp_combat: ["all", "slaying", "commanders_defeated", "conquest"],
    pve_campaign: ["all", "campaign"],
    heroes_mastery: [
        "all",
        "region:europe",
        "region:asia",
        "region:africa",
        "region:americas",
        "region:oceania",
        "kind:recruit",
        "kind:mastery",
        "kind:wins",
    ],
    prestige: ["all", "laurels"],
};

export function tText(key: string, fallback: string, values?: Record<string, unknown>): string {
    if (typeof window.SOW_hasText === "function" && window.SOW_hasText(key)) {
        return SOW_t(key, values);
    }
    if (values) {
        let out = fallback;
        for (const k of Object.keys(values)) {
            out = out.replace(new RegExp("\\{" + k + "\\}", "g"), String(values[k] ?? ""));
        }
        return out;
    }
    return fallback;
}

export function achievementText(key: string, fallback: string): string {
    return typeof window.SOW_hasText === "function" && window.SOW_hasText(key) ? SOW_t(key) : fallback;
}

export function wowCategoryTitle(cat: WowCategoryId): string {
    if (cat === "summary") return tText("profile.ach_cat_summary", "Summary");
    if (cat === "general") return tText("profile.ach_cat_general", "General & Career");
    if (cat === "pvp_combat") return tText("profile.ach_cat_pvp_combat", "PvP & Combat");
    if (cat === "pve_campaign") return tText("profile.ach_cat_pve_campaign", "PvE & Campaign");
    if (cat === "heroes_mastery") return tText("profile.ach_cat_heroes_mastery", "Heroes & Mastery");
    if (cat === "prestige") return tText("profile.ach_cat_prestige", "Prestige & Laurels");
    return tText("profile.ach_all", "All Codex");
}

export function wowSubcategoryTitle(sub: string): string {
    if (sub === "all") return tText("profile.ach_sub_all", "All");
    if (sub === "battles") return tText("profile.ach_sub_battles", "Battles & Endurance");
    if (sub === "victories") return tText("profile.ach_sub_victories", "Career Victories");
    if (sub === "vanguard") return tText("profile.ach_sub_vanguard", "Vanguard & Roster");
    if (sub === "slaying") return tText("profile.ach_sub_slaying", "Eliminations & Assists");
    if (sub === "commanders_defeated") {
        return tText("profile.ach_sub_commanders_defeated", "Rival Rulers");
    }
    if (sub === "conquest") return tText("profile.ach_sub_conquest", "Empires & Tribes");
    if (sub === "campaign") return tText("profile.ach_sub_campaign", "Campaign Chronicles");
    if (sub === "laurels") return tText("profile.ach_sub_laurels", "Renown Milestones");
    if (sub.startsWith("region:")) {
        const region = sub.slice(7);
        return tText("heroes." + region, region.charAt(0).toUpperCase() + region.slice(1));
    }
    if (sub === "kind:recruit") return tText("profile.ach_sub_recruit", "Recruitment");
    if (sub === "kind:mastery") return tText("profile.ach_sub_mastery", "Mastery Tiers");
    if (sub === "kind:wins") return tText("profile.ach_sub_hero_wins", "Hero Victories");
    return sub;
}

export function getHeroOwnershipStatus(leader: MenuLeader | null | undefined): HeroOwnershipStatus {
    if (!leader) return "locked";
    if (leader.permanently_free) return "starter";
    if (leader.owned) return "owned";
    if (leader.free_rotation && leader.available !== false) return "weekly_free";
    if (leader.available !== false) return "owned";
    return "locked";
}

export function getHeroOwnershipLabel(status: HeroOwnershipStatus): string {
    if (status === "owned") return tText("heroes.status_owned", "Owned");
    if (status === "starter") return tText("heroes.status_starter", "Starter");
    if (status === "weekly_free") return tText("heroes.status_weekly_free", "Weekly Free");
    return tText("heroes.status_locked", "Locked");
}

function getHeroEntriesCatalog(): { slug: string; name: string; leader: MenuLeader | null }[] {
    const bySlug = new Map<string, { slug: string; name: string; leader: MenuLeader | null }>();
    const liveLeaders = menuUi.state && Array.isArray(menuUi.state.leaders) ? menuUi.state.leaders : [];
    for (const leader of liveLeaders) {
        if (!leader || !leader.slug) continue;
        const slug = String(leader.slug).toLowerCase().trim();
        bySlug.set(slug, {
            slug,
            name: leaderDisplayName(leader) || leader.name || slug,
            leader,
        });
    }
    for (const fallback of ALL_HEROES_FALLBACK) {
        const slug = fallback.slug.toLowerCase();
        if (!bySlug.has(slug)) {
            const matched = leaderById(slug);
            bySlug.set(slug, {
                slug,
                name: (matched && leaderDisplayName(matched)) || fallback.name,
                leader: matched && matched.slug === slug ? matched : null,
            });
        }
    }
    return Array.from(bySlug.values());
}

export function buildHeroAchievementsForSlug(
    slug: string,
    heroName: string,
    options?: {
        owned?: boolean;
        matchesPlayed?: number;
        wins?: number;
        unlockedIds?: Set<string> | null;
        serverMap?: Map<string, ProfileAchievement> | null;
    },
): ProfileAchievement[] {
    const owned = Boolean(options?.owned);
    const matchesPlayed = Math.max(0, Number(options?.matchesPlayed) || 0);
    const wins = Math.max(0, Number(options?.wins) || 0);
    const unlockedIds = options?.unlockedIds;
    const serverMap = options?.serverMap;
    const results: ProfileAchievement[] = [];

    const makeEntry = (
        id: string,
        category: string,
        fallbackTitle: string,
        fallbackDesc: string,
        points: number,
        target: number,
        computedProgress: number,
    ): ProfileAchievement => {
        const srv = serverMap ? serverMap.get(id) : undefined;
        let progress = Math.max(computedProgress, srv ? Number(srv.progress) || 0 : 0);
        const unlocked =
            Boolean(srv && srv.unlocked) ||
            Boolean(unlockedIds && unlockedIds.has(id)) ||
            progress >= target;
        if (unlocked && progress < target) progress = target;
        return {
            id,
            category,
            title: (srv && srv.title) || fallbackTitle,
            description: (srv && srv.description) || fallbackDesc,
            points,
            target,
            progress: Math.min(target, progress),
            unlocked,
        };
    };

    results.push(
        makeEntry(
            "recruit_" + slug,
            "commanders",
            "Recruit " + heroName,
            "Unlock or recruit " + heroName + " into your vanguard.",
            25,
            1,
            owned || matchesPlayed > 0 || wins > 0 ? 1 : 0,
        ),
    );

    for (const [target, points, tierTitle] of MASTERY_TIERS) {
        const special = getSpecialHeroMasteryTitle(slug, target);
        results.push(
            makeEntry(
                slug + "_mastery_" + target,
                "commanders",
                special || heroName + " " + tierTitle,
                "Command " + heroName + " in " + target + " battles.",
                points,
                target,
                matchesPlayed,
            ),
        );
    }

    for (const [target, points, tierTitle] of WIN_TIERS) {
        results.push(
            makeEntry(
                slug + "_wins_" + target,
                "victories",
                heroName + " " + tierTitle,
                "Win " + target + " battles commanding " + heroName + ".",
                points,
                target,
                wins,
            ),
        );
    }

    return results;
}

export function getFullAchievementsCatalog(): ProfileAchievement[] {
    const list: ProfileAchievement[] = GLOBAL_ACHIEVEMENTS.map(function (a) {
        return Object.assign({}, a);
    });
    for (const hero of getHeroEntriesCatalog()) {
        const entries = buildHeroAchievementsForSlug(hero.slug, hero.name);
        for (const entry of entries) list.push(entry);
    }
    return list;
}

export function parseHeroAchievementId(
    id: string,
): { slug: string; kind: "recruit" | "mastery" | "wins" } | null {
    if (id.startsWith("recruit_")) {
        return { slug: id.slice(8), kind: "recruit" };
    }
    const masteryIdx = id.indexOf("_mastery_");
    if (masteryIdx > 0) {
        return { slug: id.slice(0, masteryIdx), kind: "mastery" };
    }
    const winsIdx = id.indexOf("_wins_");
    if (winsIdx > 0) {
        return { slug: id.slice(0, winsIdx), kind: "wins" };
    }
    return null;
}

function buildUnlockedSetFromMenuState(): Set<string> | null {
    const raw = menuUi.state && menuUi.state.achievements;
    if (!Array.isArray(raw)) return null;
    const set = new Set<string>();
    for (let i = 0; i < raw.length; i++) {
        if (typeof raw[i] === "string") set.add(raw[i]);
    }
    return set;
}

function findLeaderBySlugOrName(key: string): MenuLeader | null {
    const norm = key.toLowerCase().trim();
    const leaders = menuUi.state && Array.isArray(menuUi.state.leaders) ? menuUi.state.leaders : [];
    for (const l of leaders) {
        if (
            String(l.slug || "").toLowerCase() === norm ||
            String(l.id || "").toLowerCase() === norm ||
            String(l.name || "").toLowerCase() === norm
        ) {
            return l;
        }
    }
    const byId = leaderById(key);
    if (byId && String(byId.id || "").toLowerCase() !== "random") return byId;
    return null;
}

export function resolveProfileAchievements(data: AchievementProfileInput): ProfileAchievement[] {
    const serverMap = new Map<string, ProfileAchievement>();
    if (Array.isArray(data.achievements)) {
        for (const ach of data.achievements) {
            if (ach && ach.id) serverMap.set(ach.id, ach);
        }
    }
    if (data.achievements && data.achievements.length >= 1000) {
        return data.achievements;
    }

    const matches = Math.max(0, Number(data.matches_played) || 0);
    const wins = Math.max(0, Number(data.wins) || 0);
    const kills = Math.max(0, Number(data.kills) || 0);
    const assists = Math.max(0, Number(data.assists) || 0);
    const playersDefeated = Math.max(0, Number(data.players_defeated) || 0);
    const empiresDefeated = Math.max(0, Number(data.empires_defeated) || 0);
    const tribesDefeated = Math.max(0, Number(data.tribes_defeated) || 0);
    const laurels = Math.max(0, Number(data.laurels) || 0);
    const isOwn = Boolean(menuUi.state && data.account_id && menuUi.state.account_id === data.account_id);
    const unlockedSet = isOwn ? buildUnlockedSetFromMenuState() : null;
    const campaignCompleted = isOwn && menuUi.state?.campaign?.episodes
        ? menuUi.state.campaign.episodes.filter((ep) => ep && ep.completed).length
        : 0;

    const leadersList = Array.isArray(data.leaders) ? data.leaders : [];
    const leaderMatchesMap = new Map<string, number>();
    const leaderWinsMap = new Map<string, number>();
    const ownedSet = new Set<string>();
    let totalLeaderXp = 0;

    for (const l of leadersList) {
        if (!l || !l.leader) continue;
        const matchedLeader = findLeaderBySlugOrName(l.leader);
        const slug = String((matchedLeader && matchedLeader.slug) || l.leader).toLowerCase().trim();
        const m = Math.max(0, Number(l.matches_played) || 0);
        const w = Math.max(0, Number(l.wins) || 0);
        leaderMatchesMap.set(slug, Math.max(leaderMatchesMap.get(slug) || 0, m));
        leaderWinsMap.set(slug, Math.max(leaderWinsMap.get(slug) || 0, w));
        totalLeaderXp += Number(l.xp) || 0;
        if (m > 0 || w > 0 || (Number(l.xp) || 0) > 0) {
            ownedSet.add(slug);
        }
    }

    if (isOwn && menuUi.state && Array.isArray(menuUi.state.leaders)) {
        totalLeaderXp = 0;
        for (const l of menuUi.state.leaders) {
            if (!l || !l.slug) continue;
            const slug = String(l.slug).toLowerCase().trim();
            if (l.owned || l.permanently_free) ownedSet.add(slug);
            const st = l.stats;
            if (st) {
                const m = Math.max(0, Number(st.matches_played) || 0);
                const w = Math.max(0, Number(st.wins) || 0);
                if (m > (leaderMatchesMap.get(slug) || 0)) leaderMatchesMap.set(slug, m);
                if (w > (leaderWinsMap.get(slug) || 0)) leaderWinsMap.set(slug, w);
                if (m > 0 || w > 0) ownedSet.add(slug);
            }
            totalLeaderXp += Number(l.xp) || 0;
        }
    }

    const ownedCount = Math.max(ownedSet.size, leadersList.length);
    const resolved: ProfileAchievement[] = [];
    const seenIds = new Set<string>();

    for (const def of GLOBAL_ACHIEVEMENTS) {
        const srv = serverMap.get(def.id);
        let progress = 0;
        const target = def.target;

        if (def.id === "first_command") progress = matches >= 1 ? 1 : 0;
        else if (def.id === "first_victory") progress = wins >= 1 ? 1 : 0;
        else if (def.id === "battle_hardened") progress = Math.min(10, matches);
        else if (def.id === "victory_march") progress = Math.min(10, wins);
        else if (def.id === "commander_victorious") progress = wins >= 1 ? 1 : 0;
        else if (def.id === "veteran_commander") progress = Math.min(10, matches);
        else if (def.id === "banner_collector") progress = Math.min(3, ownedCount);
        else if (def.id === "leader_path") progress = Math.min(1000, totalLeaderXp);
        else if (def.id === "laurel_hoard") progress = Math.min(100, laurels);
        else if (def.id.startsWith("battles_")) progress = Math.min(target, matches);
        else if (def.id.startsWith("victories_")) progress = Math.min(target, wins);
        else if (def.id.startsWith("kills_")) progress = Math.min(target, kills);
        else if (def.id.startsWith("assists_")) progress = Math.min(target, assists);
        else if (def.id.startsWith("players_")) progress = Math.min(target, playersDefeated);
        else if (def.id.startsWith("empires_")) progress = Math.min(target, empiresDefeated);
        else if (def.id.startsWith("tribes_")) progress = Math.min(target, tribesDefeated);
        else if (def.id.startsWith("campaign_")) {
            progress = Math.max(srv ? Number(srv.progress) || 0 : 0, campaignCompleted);
        } else if (def.id.startsWith("laurels_")) progress = Math.min(target, laurels);
        else if (def.id.startsWith("roster_")) progress = Math.min(target, ownedCount);
        else progress = srv ? Number(srv.progress) || 0 : 0;

        if (srv && Number(srv.progress) > progress) progress = Number(srv.progress);
        const unlocked =
            Boolean(srv && srv.unlocked) ||
            Boolean(unlockedSet && unlockedSet.has(def.id)) ||
            progress >= target;
        if (unlocked && progress < target) progress = target;

        seenIds.add(def.id);
        resolved.push({
            id: def.id,
            category: def.category,
            title: def.title,
            description: def.description,
            points: def.points,
            target: target,
            progress: Math.min(target, progress),
            unlocked,
        });
    }

    for (const hero of getHeroEntriesCatalog()) {
        const entries = buildHeroAchievementsForSlug(hero.slug, hero.name, {
            owned: ownedSet.has(hero.slug),
            matchesPlayed: leaderMatchesMap.get(hero.slug) || 0,
            wins: leaderWinsMap.get(hero.slug) || 0,
            unlockedIds: unlockedSet,
            serverMap,
        });
        for (const entry of entries) {
            seenIds.add(entry.id);
            resolved.push(entry);
        }
    }

    for (const [id, srv] of serverMap.entries()) {
        if (!seenIds.has(id)) resolved.push(srv);
    }

    return resolved;
}

function classifyChain(id: string): {
    chainKey: string | null;
    chainTitleKey: string | null;
    chainFallbackTitle: string | null;
} {
    if (id === "first_command" || id === "battle_hardened" || id.startsWith("battles_")) {
        return {
            chainKey: "series:battles",
            chainTitleKey: "profile.ach_sub_battles",
            chainFallbackTitle: "Battles & Endurance",
        };
    }
    if (id === "first_victory" || id === "victory_march" || id.startsWith("victories_")) {
        return {
            chainKey: "series:victories",
            chainTitleKey: "profile.ach_sub_victories",
            chainFallbackTitle: "Career Victories",
        };
    }
    if (id.startsWith("roster_")) {
        return {
            chainKey: "series:roster",
            chainTitleKey: "profile.ach_sub_vanguard",
            chainFallbackTitle: "Vanguard & Roster",
        };
    }
    if (id.startsWith("kills_")) {
        return { chainKey: "series:kills", chainTitleKey: null, chainFallbackTitle: "Combat Eliminations" };
    }
    if (id.startsWith("assists_")) {
        return { chainKey: "series:assists", chainTitleKey: null, chainFallbackTitle: "Tactical Assists" };
    }
    if (id.startsWith("players_")) {
        return {
            chainKey: "series:players",
            chainTitleKey: "profile.ach_sub_commanders_defeated",
            chainFallbackTitle: "Rival Rulers",
        };
    }
    if (id.startsWith("empires_")) {
        return { chainKey: "series:empires", chainTitleKey: null, chainFallbackTitle: "Empires Overthrown" };
    }
    if (id.startsWith("tribes_")) {
        return { chainKey: "series:tribes", chainTitleKey: null, chainFallbackTitle: "Tribes Subjugated" };
    }
    if (id.startsWith("campaign_")) {
        return {
            chainKey: "series:campaign",
            chainTitleKey: "profile.ach_sub_campaign",
            chainFallbackTitle: "Campaign Chronicles",
        };
    }
    if (id === "laurel_hoard" || id.startsWith("laurels_")) {
        return {
            chainKey: "series:laurels",
            chainTitleKey: "profile.ach_sub_laurels",
            chainFallbackTitle: "Renown Milestones",
        };
    }
    const parsedHero = parseHeroAchievementId(id);
    if (parsedHero && parsedHero.kind === "mastery") {
        return {
            chainKey: "hero_mastery:" + parsedHero.slug,
            chainTitleKey: null,
            chainFallbackTitle: null,
        };
    }
    if (parsedHero && parsedHero.kind === "wins") {
        return {
            chainKey: "hero_wins:" + parsedHero.slug,
            chainTitleKey: null,
            chainFallbackTitle: null,
        };
    }
    return { chainKey: null, chainTitleKey: null, chainFallbackTitle: null };
}

export function enrichAchievement(
    achievement: ProfileAchievement,
    options?: { isOwnProfile?: boolean; leaderMap?: Map<string, MenuLeader> },
): EnrichedAchievement {
    const progress = Math.max(0, Number(achievement.progress) || 0);
    const target = Math.max(1, Number(achievement.target) || 1);
    const unlocked = Boolean(achievement.unlocked) || progress >= target;
    const clampedProgress = unlocked ? target : Math.min(target, progress);
    const percent = Math.min(100, Math.round((clampedProgress / target) * 100));
    const isMythic = target >= 1000;
    const points = Number(achievement.points) || 0;
    const tierLevel: AchievementTierLevel =
        isMythic || points >= 200 ? "mythic" : points >= 50 ? "elite" : "standard";

    const id = achievement.id;
    const parsedHero = parseHeroAchievementId(id);
    const chainInfo = classifyChain(id);
    let heroSlug: string | null = null;
    let heroRegion: string | null = null;
    let heroKind: "recruit" | "mastery" | "wins" | null = null;
    let heroPlayable = false;

    let wowCategory: WowConcretePillar = "general";
    let wowSubcategory = "battles";

    if (parsedHero) {
        heroSlug = parsedHero.slug;
        heroKind = parsedHero.kind;
        wowCategory = "heroes_mastery";
        wowSubcategory = "kind:" + parsedHero.kind;
        const leader =
            (options?.leaderMap && options.leaderMap.get(parsedHero.slug.toLowerCase())) ||
            findLeaderBySlugOrName(parsedHero.slug);
        if (leader) {
            heroRegion = leader.region || null;
            heroPlayable =
                leader.available !== false ||
                Boolean(leader.owned) ||
                Boolean(leader.permanently_free);
        }
    } else if (id === "commander_victorious" || id === "veteran_commander" || id === "banner_collector" || id === "leader_path" || id.startsWith("roster_")) {
        wowCategory = "general";
        wowSubcategory = "vanguard";
    } else if (id === "first_command" || id === "battle_hardened" || id.startsWith("battles_")) {
        wowCategory = "general";
        wowSubcategory = "battles";
    } else if (id === "first_victory" || id === "victory_march" || id.startsWith("victories_")) {
        wowCategory = "general";
        wowSubcategory = "victories";
    } else if (id.startsWith("kills_") || id.startsWith("assists_")) {
        wowCategory = "pvp_combat";
        wowSubcategory = "slaying";
    } else if (id.startsWith("players_")) {
        wowCategory = "pvp_combat";
        wowSubcategory = "commanders_defeated";
    } else if (id.startsWith("empires_") || id.startsWith("tribes_")) {
        wowCategory = "pvp_combat";
        wowSubcategory = "conquest";
    } else if (id.startsWith("campaign_") || achievement.category === "campaign") {
        wowCategory = "pve_campaign";
        wowSubcategory = "campaign";
    } else if (id === "laurel_hoard" || id.startsWith("laurels_") || achievement.category === "laurels") {
        wowCategory = "prestige";
        wowSubcategory = "laurels";
    } else if (achievement.category === "combat") {
        wowCategory = "pvp_combat";
        wowSubcategory = "slaying";
    } else if (achievement.category === "victories") {
        wowCategory = "general";
        wowSubcategory = "victories";
    }

    const prefix = "profile.achievement_" + id;
    return Object.assign({}, achievement, {
        progress: clampedProgress,
        target,
        unlocked,
        percent,
        isMythic,
        tierLevel,
        wowCategory,
        wowSubcategory,
        chainKey: chainInfo.chainKey,
        chainTitleKey: chainInfo.chainTitleKey,
        chainFallbackTitle: chainInfo.chainFallbackTitle,
        heroSlug,
        heroRegion,
        heroKind,
        heroPlayable,
        localizedTitle: achievementText(prefix + "_title", achievement.title || id),
        localizedDescription: achievementText(prefix + "_description", achievement.description || ""),
    });
}

export function buildLeaderLookupMap(): Map<string, MenuLeader> {
    const map = new Map<string, MenuLeader>();
    const leaders = menuUi.state && Array.isArray(menuUi.state.leaders) ? menuUi.state.leaders : [];
    for (const leader of leaders) {
        if (!leader) continue;
        if (leader.slug) map.set(String(leader.slug).toLowerCase(), leader);
        if (leader.id) map.set(String(leader.id).toLowerCase(), leader);
        if (leader.name) map.set(String(leader.name).toLowerCase(), leader);
    }
    return map;
}

export function enrichAchievementsList(
    list: ProfileAchievement[],
    isOwnProfile: boolean,
): EnrichedAchievement[] {
    const leaderMap = buildLeaderLookupMap();
    return list.map(function (ach) {
        return enrichAchievement(ach, { isOwnProfile, leaderMap });
    });
}

export function computeHeroAchievementSummary(leader: MenuLeader): HeroAchievementSummary {
    const slug = String(leader.slug || leader.id || "caesar").toLowerCase().trim();
    const displayName = leaderDisplayName(leader) || leader.name || slug;
    const stats =
        leader.stats ||
        (menuUi.state?.profile_stats?.leader_stats && leader.name
            ? menuUi.state.profile_stats.leader_stats[leader.name]
            : undefined) ||
        (menuUi.state?.profile_stats?.leader_stats
            ? menuUi.state.profile_stats.leader_stats[slug]
            : undefined) ||
        {};
    const xpMap = menuUi.state?.profile_stats?.leader_xp;
    const xp =
        typeof leader.xp === "number"
            ? leader.xp
            : xpMap && typeof xpMap === "object" && leader.name
              ? Number(xpMap[leader.name] || xpMap[slug] || 0)
              : 0;

    const ownershipStatus = getHeroOwnershipStatus(leader);
    const playable = ownershipStatus !== "locked";
    const matchesPlayed = Math.max(0, Number(stats.matches_played) || 0);
    const wins = Math.max(0, Number(stats.wins) || 0);
    const kills = Math.max(0, Number(stats.kills) || 0);
    const deaths = Math.max(0, Number(stats.deaths) || 0);
    const assists = Math.max(0, Number(stats.assists) || 0);

    const unlockedSet = buildUnlockedSetFromMenuState();
    const raw = buildHeroAchievementsForSlug(slug, displayName, {
        owned: Boolean(leader.owned) || Boolean(leader.permanently_free),
        matchesPlayed,
        wins,
        unlockedIds: unlockedSet,
    });
    const leaderMap = buildLeaderLookupMap();
    const enriched = raw.map((a) => enrichAchievement(a, { isOwnProfile: true, leaderMap }));

    return summarizeEnrichedHeroAchievements(slug, leader, displayName, enriched, {
        ownershipStatus,
        playable,
        matchesPlayed,
        wins,
        kills,
        deaths,
        assists,
        xp,
    });
}

export function summarizeEnrichedHeroAchievements(
    slug: string,
    leader: MenuLeader | null,
    fallbackName: string,
    enriched: EnrichedAchievement[],
    statsOverride?: {
        ownershipStatus: HeroOwnershipStatus;
        playable: boolean;
        matchesPlayed: number;
        wins: number;
        kills: number;
        deaths: number;
        assists: number;
        xp: number;
    },
): HeroAchievementSummary {
    const recruitAchievement = enriched.find((a) => a.heroKind === "recruit") || null;
    const masteryAchievements = enriched
        .filter((a) => a.heroKind === "mastery")
        .sort((a, b) => a.target - b.target);
    const winsAchievements = enriched
        .filter((a) => a.heroKind === "wins")
        .sort((a, b) => a.target - b.target);

    let unlockedCount = 0;
    let earnedLaurels = 0;
    let totalLaurels = 0;
    for (const a of enriched) {
        totalLaurels += a.points;
        if (a.unlocked) {
            unlockedCount += 1;
            earnedLaurels += a.points;
        }
    }

    const totalCount = enriched.length || 15;
    const percent = Math.round((unlockedCount / totalCount) * 100);
    const locked = enriched.filter((a) => !a.unlocked);
    locked.sort(function (a, b) {
        const aNonZero = a.progress > 0 ? 1 : 0;
        const bNonZero = b.progress > 0 ? 1 : 0;
        if (aNonZero !== bNonZero) return bNonZero - aNonZero;
        if (b.percent !== a.percent) return b.percent - a.percent;
        const aKindRank = a.heroKind === "recruit" ? 0 : a.heroKind === "mastery" ? 1 : 2;
        const bKindRank = b.heroKind === "recruit" ? 0 : b.heroKind === "mastery" ? 1 : 2;
        if (aKindRank !== bKindRank && !(a.percent > 0 || b.percent > 0)) return aKindRank - bKindRank;
        return a.target - b.target;
    });
    const nextAchievement = locked[0] || null;
    const nextPercent = nextAchievement ? nextAchievement.percent : 100;

    let masteryRankKey = "unproven";
    if (unlockedCount >= 15) masteryRankKey = "immortal";
    else if (unlockedCount >= 11) masteryRankKey = "conqueror";
    else if (unlockedCount >= 7) masteryRankKey = "centurion";
    else if (unlockedCount >= 4) masteryRankKey = "veteran";
    else if (unlockedCount >= 1) masteryRankKey = "initiate";

    const st = leader?.stats;
    return {
        slug,
        leader,
        name: (leader && leaderDisplayName(leader)) || fallbackName,
        civilization: (leader && leaderCivilization(leader)) || "",
        region: (leader && leader.region) || "",
        rarity: (leader && leader.rarity) || "common",
        ownershipStatus: statsOverride ? statsOverride.ownershipStatus : getHeroOwnershipStatus(leader),
        playable: statsOverride ? statsOverride.playable : leader ? leader.available !== false : false,
        achievements: enriched,
        recruitAchievement,
        masteryAchievements,
        winsAchievements,
        unlockedCount,
        totalCount,
        earnedLaurels,
        totalLaurels,
        percent,
        nextAchievement,
        nextPercent,
        masteryRankKey,
        matchesPlayed: statsOverride ? statsOverride.matchesPlayed : Math.max(0, Number(st?.matches_played) || 0),
        wins: statsOverride ? statsOverride.wins : Math.max(0, Number(st?.wins) || 0),
        kills: statsOverride ? statsOverride.kills : Math.max(0, Number(st?.kills) || 0),
        deaths: statsOverride ? statsOverride.deaths : Math.max(0, Number(st?.deaths) || 0),
        assists: statsOverride ? statsOverride.assists : Math.max(0, Number(st?.assists) || 0),
        xp: statsOverride ? statsOverride.xp : Math.max(0, Number(leader?.xp) || 0),
    };
}

export function getHeroMasteryRankLabel(rankKey: string): string {
    if (rankKey === "immortal") return tText("heroes.mastery_rank_immortal", "Immortal");
    if (rankKey === "conqueror") return tText("heroes.mastery_rank_conqueror", "Conqueror");
    if (rankKey === "centurion") return tText("heroes.mastery_rank_centurion", "Centurion");
    if (rankKey === "veteran") return tText("heroes.mastery_rank_veteran", "Veteran");
    if (rankKey === "initiate") return tText("heroes.mastery_rank_initiate", "Initiate");
    return tText("heroes.mastery_rank_unproven", "Unproven");
}

function toRomanNumeral(num: number): string {
    const map: [number, string][] = [
        [10, "X"],
        [9, "IX"],
        [5, "V"],
        [4, "IV"],
        [1, "I"],
    ];
    let n = Math.max(1, num);
    let out = "";
    for (const [val, sym] of map) {
        while (n >= val) {
            out += sym;
            n -= val;
        }
    }
    return out;
}

function renderAchievementBadgeIcon(ach: EnrichedAchievement): string {
    if (ach.heroSlug) {
        const heroArtSlug = leaderArtSlug(ach.heroSlug);
        return (
            "<img " +
            avatarImageAttribute(asset("gameplay/avatars/" + heroArtSlug + ".webp")) +
            " alt='' width='36' height='36' class='sow-profile__ach-hero-avatar' loading='lazy' />"
        );
    }
    const category = ach.category || "commanders";
    if (category === "battles") {
        return (
            "<svg class='sow-profile__ach-icon' viewBox='0 0 24 24' fill='currentColor' aria-hidden='true'>" +
            "<path d='M6.92 5L5 6.92l4.95 4.95-1.41 1.41L5 9.75 3.59 11.16l4.24 4.24 1.41-1.41-3.53-3.54 1.41-1.41 4.95 4.95L13.99 12 6.92 5zm11.75 1.58L17.25 5 12.3 9.95l1.41 1.41 3.54-3.53 1.41 1.41-4.95 4.95 1.92 1.92 7.07-7.07-4.23-4.24-1.41 1.41 3.53 3.53-1.41 1.41zM3 19.5L4.5 21l3.5-3.5-1.5-1.5L3 19.5zm16.5 0l-3.5-3.5-1.5 1.5 3.5 3.5 1.5-1.5z'/>" +
            "</svg>"
        );
    }
    if (category === "victories") {
        return (
            "<svg class='sow-profile__ach-icon' viewBox='0 0 24 24' fill='currentColor' aria-hidden='true'>" +
            "<path d='M19 5h-2V3H7v2H5c-1.1 0-2 .9-2 2v1c0 2.55 1.92 4.63 4.39 4.94A5.01 5.01 0 0011 15.9V19H7v2h10v-2h-4v-3.1a5.01 5.01 0 003.61-2.96C19.08 12.63 21 10.55 21 8V7c0-1.1-.9-2-2-2zM5 8V7h2v3.82C5.84 10.4 5 9.3 5 8zm14 0c0 1.3-.84 2.4-2 2.82V7h2v1z'/>" +
            "</svg>"
        );
    }
    if (category === "combat") {
        return (
            "<svg class='sow-profile__ach-icon' viewBox='0 0 24 24' fill='currentColor' aria-hidden='true'>" +
            "<path d='M12 2C6.49 2 2 6.49 2 12s4.49 10 10 10 10-4.49 10-10S17.51 2 12 2zm1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z'/>" +
            "</svg>"
        );
    }
    if (category === "campaign") {
        return (
            "<svg class='sow-profile__ach-icon' viewBox='0 0 24 24' fill='currentColor' aria-hidden='true'>" +
            "<path d='M20.5 3l-.16.03L15 5.1 9 3 3.36 4.9c-.21.07-.36.25-.36.48V20.5c0 .28.22.5.5.5l.16-.03L9 18.9l6 2.1 5.64-1.9c.21-.07.36-.25.36-.48V3.5c0-.28-.22-.5-.5-.5zM15 19l-6-2.11V5l6 2.11V19z'/>" +
            "</svg>"
        );
    }
    if (category === "laurels") {
        return (
            "<img src='" +
            esc(currencyAsset("laurel")) +
            "' alt='' width='28' height='28' class='sow-profile__ach-laurel-badge' />"
        );
    }
    return (
        "<svg class='sow-profile__ach-icon' viewBox='0 0 24 24' fill='currentColor' aria-hidden='true'>" +
        "<path d='M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 3c1.66 0 3 1.34 3 3s-1.34 3-3 3-3-1.34-3-3 1.34-3 3-3zm0 14.2c-2.5 0-4.71-1.28-6-3.22.03-1.99 4-3.08 6-3.08 1.99 0 5.97 1.09 6 3.08-1.29 1.94-3.5 3.22-6 3.22z'/>" +
        "</svg>"
    );
}

export function renderAchievementCard(
    ach: EnrichedAchievement,
    options?: { compact?: boolean; tagText?: string },
): string {
    const compact = Boolean(options?.compact);
    const completedLabel = tText("profile.achievement_completed", "Completed");
    const mythicLabel = tText("profile.achievement_mythic", "Mythic");
    const laurelsWord = tText("profile.laurels", "Laurels");

    return (
        "<article class='sow-profile__achievement" +
        (ach.unlocked ? " is-unlocked" : " is-locked") +
        (ach.isMythic ? " is-mythic" : "") +
        (compact ? " is-compact" : "") +
        "' data-ach-id='" +
        esc(ach.id) +
        "'>" +
        "<div class='sow-profile__achievement-header'>" +
        "<div class='sow-profile__achievement-badge-frame" +
        (ach.unlocked ? " is-unlocked" : "") +
        (ach.isMythic ? " is-mythic" : "") +
        "'>" +
        renderAchievementBadgeIcon(ach) +
        (ach.unlocked ? "<span class='sow-profile__achievement-check' aria-hidden='true'>✓</span>" : "") +
        "</div>" +
        "<div class='sow-profile__achievement-info'>" +
        "<div class='sow-profile__achievement-top-row'>" +
        "<div class='sow-profile__achievement-title-wrap'>" +
        "<strong>" +
        esc(ach.localizedTitle) +
        "</strong>" +
        (ach.isMythic ? "<span class='sow-profile__achievement-mythic-pill'>" + esc(mythicLabel) + "</span>" : "") +
        (options?.tagText
            ? "<span class='sow-profile__achievement-pillar-tag'>" + esc(options.tagText) + "</span>"
            : "") +
        "</div>" +
        "<span class='sow-profile__achievement-reward-pill" +
        (ach.isMythic ? " is-mythic" : "") +
        "'>" +
        "<img src='" +
        esc(currencyAsset("laurel")) +
        "' alt='' width='14' height='14' />" +
        "<span>+" +
        esc(ach.points) +
        "</span>" +
        "</span>" +
        "</div>" +
        "<span>" +
        esc(ach.localizedDescription) +
        "</span>" +
        "</div>" +
        "</div>" +
        "<div class='sow-profile__achievement-progress-wrap'>" +
        "<div class='sow-profile__achievement-track" +
        (ach.isMythic ? " is-mythic" : "") +
        "' role='progressbar' aria-valuenow='" +
        esc(ach.progress) +
        "' aria-valuemax='" +
        esc(ach.target) +
        "'>" +
        "<i style='width: " +
        esc(ach.percent) +
        "%'></i>" +
        "</div>" +
        "<div class='sow-profile__achievement-meta'>" +
        "<span class='sow-profile__achievement-status" +
        (ach.unlocked ? " is-done" : "") +
        "'>" +
        esc(ach.unlocked ? completedLabel : ach.percent + "%") +
        "</span>" +
        "<b>" +
        esc(ach.progress) +
        "/" +
        esc(ach.target) +
        " · " +
        esc(ach.points) +
        " " +
        esc(laurelsWord) +
        "</b>" +
        "</div>" +
        "</div>" +
        "</article>"
    );
}

export function renderAchievementChainCard(
    chainKey: string,
    seriesItems: EnrichedAchievement[],
    selectedTierId?: string | null,
): string {
    const sorted = seriesItems.slice().sort((a, b) => a.target - b.target);
    const unlockedCount = sorted.filter((a) => a.unlocked).length;
    const totalTiers = sorted.length;
    const defaultActive = sorted.find((a) => !a.unlocked) || sorted[sorted.length - 1];
    const active =
        (selectedTierId && sorted.find((a) => a.id === selectedTierId)) || defaultActive;
    const activeIdx = Math.max(0, sorted.indexOf(active));
    const seriesTitle = active.chainTitleKey
        ? tText(active.chainTitleKey, active.chainFallbackTitle || active.localizedTitle)
        : active.chainFallbackTitle || active.localizedTitle;
    const totalSeriesLaurels = sorted.reduce((acc, a) => acc + a.points, 0);
    const earnedSeriesLaurels = sorted.reduce((acc, a) => acc + (a.unlocked ? a.points : 0), 0);
    const completedLabel = tText("profile.achievement_completed", "Completed");
    const mythicLabel = tText("profile.achievement_mythic", "Mythic");
    const laurelsWord = tText("profile.laurels", "Laurels");

    const stepsHtml = sorted
        .map(function (step, idx) {
            const isActive = step.id === active.id;
            return (
                "<button type='button' class='sow-ach-chain__step" +
                (step.unlocked ? " is-done" : "") +
                (isActive ? " is-active" : "") +
                "' data-command='select_chain_tier' data-chain-key='" +
                esc(chainKey) +
                "' data-tier-id='" +
                esc(step.id) +
                "' title='" +
                esc(step.localizedTitle + " (" + step.progress + "/" + step.target + ")") +
                "'>" +
                (step.unlocked ? "✓ " : "") +
                esc(toRomanNumeral(idx + 1)) +
                "</button>"
            );
        })
        .join("");

    return (
        "<article class='sow-profile__achievement sow-ach-chain" +
        (active.unlocked ? " is-unlocked" : " is-locked") +
        (active.isMythic ? " is-mythic" : "") +
        "'>" +
        "<div class='sow-ach-chain__kicker'>" +
        "<span class='sow-ach-chain__series-name'>" +
        esc(seriesTitle) +
        "</span>" +
        "<span class='sow-ach-chain__tier-count'>" +
        esc(
            unlockedCount === totalTiers
                ? tText("profile.ach_all_tiers_done", "All Tiers Mastered")
                : tText("profile.ach_tier_step", "Tier {current}/{total}", {
                      current: activeIdx + 1,
                      total: totalTiers,
                  }),
        ) +
        " · " +
        esc(earnedSeriesLaurels + "/" + totalSeriesLaurels + " " + laurelsWord) +
        "</span>" +
        "</div>" +
        "<div class='sow-profile__achievement-header'>" +
        "<div class='sow-profile__achievement-badge-frame" +
        (active.unlocked ? " is-unlocked" : "") +
        (active.isMythic ? " is-mythic" : "") +
        "'>" +
        renderAchievementBadgeIcon(active) +
        (active.unlocked ? "<span class='sow-profile__achievement-check' aria-hidden='true'>✓</span>" : "") +
        "</div>" +
        "<div class='sow-profile__achievement-info'>" +
        "<div class='sow-profile__achievement-top-row'>" +
        "<div class='sow-profile__achievement-title-wrap'>" +
        "<strong>" +
        esc(active.localizedTitle) +
        "</strong>" +
        (active.isMythic ? "<span class='sow-profile__achievement-mythic-pill'>" + esc(mythicLabel) + "</span>" : "") +
        "</div>" +
        "<span class='sow-profile__achievement-reward-pill" +
        (active.isMythic ? " is-mythic" : "") +
        "'>" +
        "<img src='" +
        esc(currencyAsset("laurel")) +
        "' alt='' width='14' height='14' />" +
        "<span>+" +
        esc(active.points) +
        "</span>" +
        "</span>" +
        "</div>" +
        "<span>" +
        esc(active.localizedDescription) +
        "</span>" +
        "</div>" +
        "</div>" +
        "<div class='sow-profile__achievement-progress-wrap'>" +
        "<div class='sow-profile__achievement-track" +
        (active.isMythic ? " is-mythic" : "") +
        "' role='progressbar' aria-valuenow='" +
        esc(active.progress) +
        "' aria-valuemax='" +
        esc(active.target) +
        "'>" +
        "<i style='width: " +
        esc(active.percent) +
        "%'></i>" +
        "</div>" +
        "<div class='sow-profile__achievement-meta'>" +
        "<span class='sow-profile__achievement-status" +
        (active.unlocked ? " is-done" : "") +
        "'>" +
        esc(active.unlocked ? completedLabel : active.percent + "%") +
        "</span>" +
        "<b>" +
        esc(active.progress) +
        "/" +
        esc(active.target) +
        " · " +
        esc(active.points) +
        " " +
        esc(laurelsWord) +
        "</b>" +
        "</div>" +
        "</div>" +
        "<div class='sow-ach-chain__steps' role='group' aria-label='" +
        esc(seriesTitle) +
        "'>" +
        stepsHtml +
        "</div>" +
        "</article>"
    );
}

function renderMiniTrack(
    label: string,
    items: EnrichedAchievement[],
    singleMode: boolean,
): string {
    const done = items.filter((a) => a.unlocked).length;
    const next = items.find((a) => !a.unlocked) || items[items.length - 1];
    const progressPct = singleMode
        ? next?.unlocked
            ? 100
            : 0
        : next
          ? next.percent
          : 0;
    const nextMeta = next
        ? next.unlocked
            ? tText("profile.achievement_completed", "Completed")
            : next.progress + "/" + next.target + " (+" + next.points + ")"
        : "";
    return (
        "<div class='sow-ach-hero-card__track-row'>" +
        "<div class='sow-ach-hero-card__track-head'>" +
        "<span>" +
        esc(label) +
        " <b>" +
        esc(done + "/" + items.length) +
        "</b></span>" +
        "<small>" +
        esc(nextMeta) +
        "</small>" +
        "</div>" +
        "<div class='sow-profile__achievement-track'><i style='width:" +
        esc(progressPct) +
        "%'></i></div>" +
        "</div>"
    );
}

export function renderHeroMasteryProfileCard(
    summary: HeroAchievementSummary,
    isOwnProfile: boolean,
): string {
    const avatarUrl = asset("gameplay/avatars/" + leaderArtSlug(summary.slug) + ".webp");
    const ownershipBadge = isOwnProfile
        ? "<span class='sow-ach-hero-card__own is-" +
          esc(summary.ownershipStatus) +
          "'>" +
          esc(getHeroOwnershipLabel(summary.ownershipStatus)) +
          "</span>"
        : "";
    const regionLabel = summary.region
        ? tText("heroes." + summary.region, summary.region)
        : "";
    const rankLabel = getHeroMasteryRankLabel(summary.masteryRankKey);

    return (
        "<article class='sow-ach-hero-card" +
        (summary.unlockedCount === summary.totalCount ? " is-complete" : "") +
        (summary.unlockedCount > 0 ? " has-progress" : "") +
        "'>" +
        "<header class='sow-ach-hero-card__head'>" +
        "<img " +
        avatarImageAttribute(avatarUrl) +
        " alt='' width='48' height='48' class='sow-ach-hero-card__avatar' loading='lazy' />" +
        "<div class='sow-ach-hero-card__id'>" +
        "<div class='sow-ach-hero-card__title-row'>" +
        "<strong>" +
        esc(summary.name) +
        "</strong>" +
        ownershipBadge +
        "</div>" +
        "<div class='sow-ach-hero-card__sub'>" +
        (regionLabel ? "<span>" + esc(regionLabel) + "</span>" : "") +
        "<span class='sow-ach-hero-card__rank'>" +
        esc(rankLabel) +
        "</span>" +
        "<span>" +
        esc(summary.unlockedCount + "/" + summary.totalCount) +
        " · " +
        esc(summary.earnedLaurels + "/" + summary.totalLaurels) +
        " " +
        esc(tText("profile.laurels", "Laurels")) +
        "</span>" +
        "</div>" +
        "</div>" +
        (summary.leader
            ? "<button type='button' class='sow-ach-hero-card__dossier-btn' data-command='open_hero_info' data-leader-id='" +
              esc(summary.leader.id) +
              "' data-hero-info-tab='achievements'>" +
              esc(tText("profile.ach_inspect_hero", "Hero Dossier")) +
              "</button>"
            : "") +
        "</header>" +
        "<div class='sow-profile__achievement-track sow-ach-hero-card__overall'><i style='width:" +
        esc(summary.percent) +
        "%'></i></div>" +
        "<div class='sow-ach-hero-card__tracks'>" +
        (summary.recruitAchievement
            ? renderMiniTrack(
                  tText("profile.ach_recruit_short", "Recruit"),
                  [summary.recruitAchievement],
                  true,
              )
            : "") +
        (summary.masteryAchievements.length
            ? renderMiniTrack(
                  tText("profile.ach_mastery_short", "Mastery"),
                  summary.masteryAchievements,
                  false,
              )
            : "") +
        (summary.winsAchievements.length
            ? renderMiniTrack(
                  tText("profile.ach_wins_short", "Victories"),
                  summary.winsAchievements,
                  false,
              )
            : "") +
        "</div>" +
        "</article>"
    );
}

export function matchesAchievementFilters(
    ach: EnrichedAchievement,
    filters: {
        search?: string;
        status?: string;
        tier?: string;
        playableOnly?: boolean;
        isOwnProfile?: boolean;
    },
): boolean {
    if (filters.status === "unlocked" && !ach.unlocked) return false;
    if (filters.status === "in_progress" && (ach.unlocked || ach.progress <= 0)) return false;
    if (filters.status === "locked" && (ach.unlocked || ach.progress > 0)) return false;
    if (filters.tier && filters.tier !== "all" && ach.tierLevel !== filters.tier) return false;
    if (filters.playableOnly && filters.isOwnProfile && ach.heroSlug && !ach.heroPlayable) {
        return false;
    }
    const q = (filters.search || "").trim().toLowerCase();
    if (q) {
        const heroMeta = ach.heroSlug ? leaderById(ach.heroSlug) : null;
        const heroDisplay = heroMeta ? leaderDisplayName(heroMeta).toLowerCase() : "";
        const hay =
            ach.localizedTitle.toLowerCase() +
            " " +
            ach.localizedDescription.toLowerCase() +
            " " +
            ach.id.toLowerCase() +
            " " +
            (ach.heroRegion || "").toLowerCase() +
            " " +
            heroDisplay;
        if (!hay.includes(q)) return false;
    }
    return true;
}

export function sortAchievementsList(
    list: EnrichedAchievement[],
    sortBy: string,
): EnrichedAchievement[] {
    const copy = list.slice();
    if (sortBy === "closest") {
        copy.sort(function (a, b) {
            if (a.unlocked !== b.unlocked) return a.unlocked ? 1 : -1;
            if (b.percent !== a.percent) return b.percent - a.percent;
            return b.points - a.points;
        });
        return copy;
    }
    if (sortBy === "points_desc") {
        copy.sort(function (a, b) {
            if (b.points !== a.points) return b.points - a.points;
            return b.percent - a.percent;
        });
        return copy;
    }
    if (sortBy === "alpha_asc") {
        copy.sort(function (a, b) {
            return a.localizedTitle.localeCompare(b.localizedTitle);
        });
        return copy;
    }
    // "smart": active in-progress (>0%) first by %, then ready/locked by lowest target, then completed
    copy.sort(function (a, b) {
        const aBucket = !a.unlocked && a.progress > 0 ? 0 : !a.unlocked ? 1 : 2;
        const bBucket = !b.unlocked && b.progress > 0 ? 0 : !b.unlocked ? 1 : 2;
        if (aBucket !== bBucket) return aBucket - bBucket;
        if (aBucket === 0 && b.percent !== a.percent) return b.percent - a.percent;
        if (a.target !== b.target) return a.target - b.target;
        return b.points - a.points;
    });
    return copy;
}

/** Compact Featured Hero Status & Achievements ribbon inside Hero Selection. */
export function renderHeroSelectionAchievementStrip(leader: MenuLeader): string {
    const summary = computeHeroAchievementSummary(leader);
    const statusLabel = getHeroOwnershipLabel(summary.ownershipStatus);
    const matches = summary.matchesPlayed;
    const wins = summary.wins;
    const winRate = matches > 0 ? Math.round((wins / matches) * 100) : 0;
    const next = summary.nextAchievement;
    const nextText = next
        ? next.localizedTitle + " (" + next.progress + "/" + next.target + " · +" + next.points + ")"
        : tText("heroes.all_hero_achievements_done", "All {total} hero achievements completed!", {
              total: summary.totalCount,
          });
    const nextPct = next ? next.percent : 100;

    return (
        "<div class='sow-heroes__ach-strip' data-hero-achievement-strip>" +
        "<div class='sow-heroes__ach-strip-meta'>" +
        "<span class='sow-heroes__ach-own is-" +
        esc(summary.ownershipStatus) +
        "'>" +
        esc(statusLabel) +
        "</span>" +
        "<span class='sow-heroes__ach-record'>" +
        esc(
            matches > 0
                ? matches + " " + tText("heroes.stat_matches", "Matches") + " · " + wins + "W (" + winRate + "%)"
                : tText("heroes.mastery_rank_" + summary.masteryRankKey, getHeroMasteryRankLabel(summary.masteryRankKey)),
        ) +
        "</span>" +
        "<span class='sow-heroes__ach-xp'>" +
        esc(summary.xp + " XP") +
        "</span>" +
        "</div>" +
        "<button type='button' class='sow-heroes__ach-btn' data-command='open_hero_info' data-leader-id='" +
        esc(leader.id) +
        "' data-hero-info-tab='achievements'>" +
        "<div class='sow-heroes__ach-btn-row'>" +
        "<strong class='sow-heroes__ach-count'>★ " +
        esc(summary.unlockedCount + "/" + summary.totalCount) +
        " " +
        esc(tText("heroes.hero_achievements", "Hero Achievements")) +
        "</strong>" +
        "<span class='sow-heroes__ach-next-label'>" +
        esc(next ? tText("heroes.next_milestone", "Next") + ": " + nextText : nextText) +
        "</span>" +
        "<span class='sow-heroes__ach-cta'>›</span>" +
        "</div>" +
        "<div class='sow-heroes__ach-bar'><i style='width:" +
        esc(next ? nextPct : summary.percent) +
        "%'></i></div>" +
        "</button>" +
        "</div>"
    );
}

/** 3rd Tab inside the Hero Dossier Modal: Full Hero Status + Filterable Achievements. */
export function renderHeroDossierAchievementsTab(
    leader: MenuLeader,
    controls: {
        category: "all" | "recruit" | "mastery" | "victories";
        status: "all" | "in_progress" | "unlocked" | "locked";
        sort: "tier" | "closest" | "points";
    },
): string {
    const summary = computeHeroAchievementSummary(leader);
    const matches = summary.matchesPlayed;
    const wins = summary.wins;
    const winRate = matches > 0 ? Math.round((wins / matches) * 100) + "%" : "0%";
    const rankLabel = getHeroMasteryRankLabel(summary.masteryRankKey);
    const statusLabel = getHeroOwnershipLabel(summary.ownershipStatus);

    const catCounts = {
        all: summary.achievements.length,
        recruit: summary.recruitAchievement ? 1 : 0,
        mastery: summary.masteryAchievements.length,
        victories: summary.winsAchievements.length,
    };

    const filtered = summary.achievements.filter(function (a) {
        if (controls.category !== "all" && a.heroKind !== controls.category) return false;
        if (controls.status === "unlocked" && !a.unlocked) return false;
        if (controls.status === "in_progress" && (a.unlocked || a.progress <= 0)) return false;
        if (controls.status === "locked" && (a.unlocked || a.progress > 0)) return false;
        return true;
    });

    const sorted = filtered.slice().sort(function (a, b) {
        if (controls.sort === "closest") {
            if (a.unlocked !== b.unlocked) return a.unlocked ? 1 : -1;
            if (b.percent !== a.percent) return b.percent - a.percent;
            return a.target - b.target;
        }
        if (controls.sort === "points") {
            if (b.points !== a.points) return b.points - a.points;
            return b.percent - a.percent;
        }
        // "tier": Recruit -> Mastery tiers -> Wins tiers
        const orderA = a.heroKind === "recruit" ? 0 : a.heroKind === "mastery" ? 1 : 2;
        const orderB = b.heroKind === "recruit" ? 0 : b.heroKind === "mastery" ? 1 : 2;
        if (orderA !== orderB) return orderA - orderB;
        return a.target - b.target;
    });

    const headerCard =
        "<div class='sow-hero-info__ach-summary'>" +
        "<div class='sow-hero-info__ach-status-row'>" +
        "<span class='sow-ach-hero-card__own is-" +
        esc(summary.ownershipStatus) +
        "'>" +
        esc(statusLabel) +
        "</span>" +
        "<span class='sow-ach-hero-card__rank'>" +
        esc(rankLabel) +
        "</span>" +
        "<span class='sow-hero-info__ach-laurels'>" +
        "<img src='" +
        esc(currencyAsset("laurel")) +
        "' alt='' width='14' height='14' /> " +
        esc(tText("heroes.achievements_earned_laurels", "{earned}/{total} Laurels", {
            earned: summary.earnedLaurels,
            total: summary.totalLaurels,
        })) +
        "</span>" +
        "</div>" +
        "<div class='sow-hero-info__ach-overall'>" +
        "<div class='sow-hero-info__ach-overall-head'>" +
        "<strong>" +
        esc(tText("heroes.achievements_progress", "{unlocked}/{total} Achievements", {
            unlocked: summary.unlockedCount,
            total: summary.totalCount,
        })) +
        "</strong>" +
        "<span>" +
        esc(summary.percent) +
        "% · " +
        esc(matches) +
        " " +
        esc(tText("heroes.stat_matches", "Matches")) +
        " · " +
        esc(wins) +
        " " +
        esc(tText("heroes.stat_wins", "Wins")) +
        " (" +
        esc(winRate) +
        ") · " +
        esc(summary.xp) +
        " XP</span>" +
        "</div>" +
        "<div class='sow-profile__achievement-track'><i style='width:" +
        esc(summary.percent) +
        "%'></i></div>" +
        "</div>" +
        "</div>";

    const catPills: { id: "all" | "recruit" | "mastery" | "victories"; label: string }[] = [
        { id: "all", label: tText("heroes.ach_cat_all", "All") + " (" + catCounts.all + ")" },
        { id: "recruit", label: tText("heroes.ach_cat_recruit", "Recruit") + " (" + catCounts.recruit + ")" },
        { id: "mastery", label: tText("heroes.ach_cat_mastery", "Mastery") + " (" + catCounts.mastery + ")" },
        { id: "victories", label: tText("heroes.ach_cat_victories", "Victories") + " (" + catCounts.victories + ")" },
    ];

    const statusPills: { id: "all" | "in_progress" | "unlocked" | "locked"; label: string }[] = [
        { id: "all", label: tText("heroes.ach_status_all", "All") },
        { id: "in_progress", label: tText("heroes.ach_status_active", "In Progress") },
        { id: "unlocked", label: tText("heroes.ach_status_done", "Completed") },
        { id: "locked", label: tText("heroes.ach_status_locked", "Upcoming") },
    ];

    const sortPills: { id: "tier" | "closest" | "points"; label: string }[] = [
        { id: "tier", label: tText("heroes.ach_sort_tier", "Tier Order") },
        { id: "closest", label: tText("heroes.ach_sort_closest", "Closest %") },
        { id: "points", label: tText("heroes.ach_sort_laurels", "Laurels") },
    ];

    const controlsHtml =
        "<div class='sow-hero-info__ach-controls'>" +
        "<div class='sow-hero-info__ach-pill-row' role='group' aria-label='" +
        esc(tText("heroes.hero_achievements", "Hero Achievements")) +
        "'>" +
        catPills
            .map(
                (p) =>
                    "<button type='button' class='sow-hero-info__ach-pill" +
                    (controls.category === p.id ? " is-active" : "") +
                    "' data-command='hero_info_ach_category' data-category='" +
                    esc(p.id) +
                    "'>" +
                    esc(p.label) +
                    "</button>",
            )
            .join("") +
        "</div>" +
        "<div class='sow-hero-info__ach-subcontrols'>" +
        "<div class='sow-hero-info__ach-pill-row sow-hero-info__ach-pill-row--sm'>" +
        statusPills
            .map(
                (p) =>
                    "<button type='button' class='sow-hero-info__ach-pill" +
                    (controls.status === p.id ? " is-active" : "") +
                    "' data-command='hero_info_ach_status' data-status='" +
                    esc(p.id) +
                    "'>" +
                    esc(p.label) +
                    "</button>",
            )
            .join("") +
        "</div>" +
        "<div class='sow-hero-info__ach-pill-row sow-hero-info__ach-pill-row--sm'>" +
        sortPills
            .map(
                (p) =>
                    "<button type='button' class='sow-hero-info__ach-pill" +
                    (controls.sort === p.id ? " is-active" : "") +
                    "' data-command='hero_info_ach_sort' data-sort='" +
                    esc(p.id) +
                    "'>" +
                    esc(p.label) +
                    "</button>",
            )
            .join("") +
        "</div>" +
        "</div>" +
        "</div>";

    const listHtml = sorted.length
        ? "<div class='sow-hero-info__ach-grid'>" +
          sorted.map((a) => renderAchievementCard(a, { compact: true })).join("") +
          "</div>"
        : "<p class='sow-hero-info__bio-status'>" +
          esc(tText("heroes.ach_empty_filtered", "No hero achievements match these filters.")) +
          "</p>";

    return "<div class='sow-hero-info__ach'>" + headerCard + controlsHtml + listHtml + "</div>";
}
