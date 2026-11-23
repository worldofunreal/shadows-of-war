/**
 * Sound Studio — Declarative Catalog Metadata for all Procedural Audio Cues.
 *
 * Imports `AudioCue`, `CUE_REGISTRY`, and subtype enums directly from
 * `sow-web/shell/audio/` so the tool is always in lockstep with the game engine.
 * Any future `AudioCue` added to `types.ts` automatically receives a fallback
 * catalog entry so it can be auditioned immediately.
 */

import {
  AbilityAudioFamily,
  ArsenalActionSubtype,
  AudioCue,
  BuildingAudioKind,
  BuildingModeSubtype,
  CombatAudioSubtype,
  CUE_REGISTRY,
  EmoteAudioKind,
  FleetOrderSubtype,
  MatchPhaseAudioKind,
  NukeAudioLevel,
  ResourceAudioSubtype,
  UiBattleLaunchSubtype,
  UiHeroBrowseSubtype,
  UiHoverSubtype,
  UiInfoTabSubtype,
  UiPressSubtype,
  UiPurchaseActionSubtype,
  type AudioCategory,
  type AudioCueId,
  type CueDefinition,
} from "../../../sow-web/shell/audio/index.ts";

export interface CueVariantSpec {
  readonly label: string;
  readonly subtype: number;
  /** Optional intensity override when auditioning this variant (uses inspector slider if omitted). */
  readonly intensity?: number;
}

export interface CueCatalogEntry {
  readonly id: AudioCueId;
  readonly name: string;
  readonly category: AudioCategory;
  readonly def: CueDefinition;
  readonly description: string;
  readonly recipeFile: string;
  readonly variants: readonly CueVariantSpec[];
  readonly defaultIntensity: number;
  readonly disabledInGame?: boolean;
  /** Interval (ms) between steps when pressing the Burst / Sequence test button. */
  readonly burstIntervalMs: number;
  /** Number of hits in a burst when the cue has only 1 variant. */
  readonly burstCount: number;
}

interface RawCueMeta {
  readonly description: string;
  readonly recipeFile: string;
  readonly variants?: readonly CueVariantSpec[];
  readonly defaultIntensity?: number;
  readonly disabledInGame?: boolean;
  readonly burstIntervalMs?: number;
  readonly burstCount?: number;
}

export const CATEGORY_LABELS: Record<AudioCategory, string> = {
  ui: "UI / Menús",
  combat: "Combate",
  building: "Edificios",
  naval: "Naval",
  nuke: "Nuclear / SAM",
  diplomacy: "Diplomacia",
  match: "Partida",
};

export const CATEGORY_ORDER: readonly AudioCategory[] = [
  "ui",
  "combat",
  "building",
  "naval",
  "nuke",
  "diplomacy",
  "match",
];

const BUILDING_VARIANTS: readonly CueVariantSpec[] = [
  { label: "City", subtype: BuildingAudioKind.City },
  { label: "Tower", subtype: BuildingAudioKind.Tower },
  { label: "TradingPost", subtype: BuildingAudioKind.TradingPost },
  { label: "Port", subtype: BuildingAudioKind.Port },
  { label: "MilitaryCamp", subtype: BuildingAudioKind.MilitaryCamp },
  { label: "SAM", subtype: BuildingAudioKind.SAM },
  { label: "MissileSilo", subtype: BuildingAudioKind.MissileSilo },
];

const COMBAT_ARCHETYPE_VARIANTS: readonly CueVariantSpec[] = [
  { label: "Wilderness", subtype: CombatAudioSubtype.Wilderness },
  { label: "Tribe", subtype: CombatAudioSubtype.Tribe },
  { label: "Nation", subtype: CombatAudioSubtype.Nation },
  { label: "Human", subtype: CombatAudioSubtype.Human },
  { label: "Counter", subtype: CombatAudioSubtype.Counter },
];

const NUKE_LEVEL_VARIANTS: readonly CueVariantSpec[] = [
  { label: "Atom", subtype: NukeAudioLevel.Atom },
  { label: "Hydrogen", subtype: NukeAudioLevel.Hydrogen },
  { label: "MIRV Carrier", subtype: NukeAudioLevel.MirvCarrier },
  { label: "MIRV Warhead", subtype: NukeAudioLevel.MirvWarhead },
];

const CUE_META: Partial<Record<AudioCueId, RawCueMeta>> = {
  // ── Combat & Land (0–10) ─────────────────────────────────────────────
  [AudioCue.DeployTroops]: {
    description: "Elegir posición inicial en fase Spawning o desplegar refuerzos.",
    recipeFile: "recipes/combat.ts",
    defaultIntensity: 0.7,
    burstIntervalMs: 260,
  },
  [AudioCue.AttackOrder]: {
    description:
      "Tap/clic para atacar: 4 arquetipos (Wilderness, Tribe, Nation, Human) + variación única por objetivo + combo ascendente en taps rápidos.",
    recipeFile: "recipes/combat.ts",
    defaultIntensity: 0.65,
    burstIntervalMs: 170,
    variants: [
      { label: "Wilderness · A", subtype: (0 << 2) | CombatAudioSubtype.Wilderness, intensity: 0.55 },
      { label: "Wilderness · B", subtype: (3 << 2) | CombatAudioSubtype.Wilderness, intensity: 0.7 },
      { label: "Tribe · A", subtype: (1 << 2) | CombatAudioSubtype.Tribe, intensity: 0.65 },
      { label: "Tribe · B", subtype: (5 << 2) | CombatAudioSubtype.Tribe, intensity: 0.75 },
      { label: "Nation · A", subtype: (2 << 2) | CombatAudioSubtype.Nation, intensity: 0.7 },
      { label: "Nation · B", subtype: (6 << 2) | CombatAudioSubtype.Nation, intensity: 0.85 },
      { label: "Human · A", subtype: (0 << 2) | CombatAudioSubtype.Human, intensity: 0.75 },
      { label: "Human · B", subtype: (4 << 2) | CombatAudioSubtype.Human, intensity: 0.9 },
    ],
  },
  [AudioCue.AttackStalled]: {
    description: "Un ataque propio pierde impulso o es frenado sin avanzar.",
    recipeFile: "recipes/combat.ts",
    defaultIntensity: 0.6,
  },
  [AudioCue.UnderAttack]: {
    description: "Alerta crítica cuando un enemigo ataca directamente tu territorio.",
    recipeFile: "recipes/combat.ts",
    defaultIntensity: 0.85,
  },
  [AudioCue.LandExpand]: {
    description: "Avance continuo de casillas al expandir territorio (desactivado en partida).",
    recipeFile: "recipes/combat.ts",
    disabledInGame: true,
    defaultIntensity: 0.5,
    burstIntervalMs: 120,
    burstCount: 6,
  },
  [AudioCue.LandCombat]: {
    description: "Choque continuo casilla a casilla durante ataque (desactivado en partida).",
    recipeFile: "recipes/combat.ts",
    disabledInGame: true,
    defaultIntensity: 0.65,
    burstIntervalMs: 150,
    variants: COMBAT_ARCHETYPE_VARIANTS,
  },
  [AudioCue.CounterAttack]: {
    description: "Activar contraataque ofensivo desde mapa o menú radial.",
    recipeFile: "recipes/combat.ts",
    defaultIntensity: 0.8,
  },
  [AudioCue.AttackCancelled]: {
    description: "Cancelar o retirar una orden de ataque en curso.",
    recipeFile: "recipes/combat.ts",
    defaultIntensity: 0.6,
  },
  [AudioCue.CascadeStep]: {
    description: "Escala ascendente D Major9 al capturar edificios o activar Tactical Surge.",
    recipeFile: "recipes/combat.ts",
    defaultIntensity: 0.75,
    burstIntervalMs: 155,
    variants: [
      { label: "Paso 1 (D4)", subtype: 1 },
      { label: "Paso 2 (E4)", subtype: 2 },
      { label: "Paso 3 (F#4)", subtype: 3 },
      { label: "Paso 4 (A4)", subtype: 4 },
      { label: "Paso 5 (B4)", subtype: 5 },
      { label: "Paso 6 (D5)", subtype: 6 },
      { label: "Paso 7 (E5)", subtype: 7 },
      { label: "Paso 8 (F#5 Jackpot)", subtype: 8 },
    ],
  },
  [AudioCue.KillStreak]: {
    description: "Racha progresiva de eliminaciones o conquistas seguidas.",
    recipeFile: "recipes/combat.ts",
    defaultIntensity: 0.8,
    burstIntervalMs: 360,
    variants: [
      { label: "Racha ×1", subtype: 1 },
      { label: "Racha ×2", subtype: 2 },
      { label: "Racha ×3", subtype: 3 },
      { label: "Racha ×4", subtype: 4 },
      { label: "Racha ×5", subtype: 5 },
      { label: "Racha ×6", subtype: 6 },
    ],
  },
  [AudioCue.PlayerEliminated]: {
    description: "Caída completa de una facción o jugador en el mapa.",
    recipeFile: "recipes/combat.ts",
    defaultIntensity: 0.85,
    burstIntervalMs: 480,
    variants: [
      { label: "Tribe", subtype: CombatAudioSubtype.Tribe },
      { label: "Nation", subtype: CombatAudioSubtype.Nation },
      { label: "Human", subtype: CombatAudioSubtype.Human },
    ],
  },

  // ── Naval & Trade (11–20) ────────────────────────────────────────────
  [AudioCue.TransportLaunch]: {
    description: "Despacho de tropas por mar (barco de transporte anfibio).",
    recipeFile: "recipes/naval.ts",
    defaultIntensity: 0.7,
  },
  [AudioCue.TransportLanded]: {
    description: "Desembarco de tropas en la costa objetivo.",
    recipeFile: "recipes/naval.ts",
    defaultIntensity: 0.75,
  },
  [AudioCue.FleetOrder]: {
    description: "Orden directa a una flota naval.",
    recipeFile: "recipes/naval.ts",
    defaultIntensity: 0.65,
    burstIntervalMs: 320,
    variants: [
      { label: "Move", subtype: FleetOrderSubtype.Move },
      { label: "Patrol", subtype: FleetOrderSubtype.Patrol },
    ],
  },
  [AudioCue.FleetRecallOrScuttle]: {
    description: "Retirar a puerto o hundir voluntariamente una flota propia.",
    recipeFile: "recipes/naval.ts",
    defaultIntensity: 0.7,
    burstIntervalMs: 380,
    variants: [
      { label: "Recall", subtype: FleetOrderSubtype.Recall },
      { label: "Scuttle", subtype: FleetOrderSubtype.Scuttle },
    ],
  },
  [AudioCue.WarshipSpawned]: {
    description: "Botadura de un nuevo buque de guerra en un Port.",
    recipeFile: "recipes/naval.ts",
    defaultIntensity: 0.8,
  },
  [AudioCue.NavalShot]: {
    description: "Disparo de cañón de buque de guerra en combate naval.",
    recipeFile: "recipes/naval.ts",
    defaultIntensity: 0.75,
    burstIntervalMs: 210,
    burstCount: 4,
  },
  [AudioCue.NavalImpact]: {
    description: "Impacto de artillería naval en agua o casco enemigo.",
    recipeFile: "recipes/naval.ts",
    defaultIntensity: 0.75,
    burstIntervalMs: 240,
    burstCount: 4,
  },
  [AudioCue.NavalSink]: {
    description: "Hundimiento de un buque en batalla naval.",
    recipeFile: "recipes/naval.ts",
    defaultIntensity: 0.85,
  },
  [AudioCue.TradeDelivered]: {
    description: "Llegada de barco mercante a puerto con entrega de oro.",
    recipeFile: "recipes/naval.ts",
    defaultIntensity: 0.75,
    burstIntervalMs: 360,
    variants: [
      { label: "Entrega chica", subtype: 0, intensity: 0.35 },
      { label: "Entrega grande", subtype: 0, intensity: 0.95 },
    ],
  },
  [AudioCue.TradeCaptured]: {
    description: "Intercepción y saqueo de un barco mercante en alta mar.",
    recipeFile: "recipes/naval.ts",
    defaultIntensity: 0.8,
  },

  // ── Buildings & Workflow (21–25, 57–59) ──────────────────────────────
  [AudioCue.BuildingPlace]: {
    description: "Colocar cimientos de obra nueva (firma física distinta por edificio).",
    recipeFile: "recipes/buildings.ts",
    defaultIntensity: 0.75,
    burstIntervalMs: 320,
    variants: BUILDING_VARIANTS,
  },
  [AudioCue.BuildingUpgrade]: {
    description: "Iniciar mejora de nivel de un edificio existente.",
    recipeFile: "recipes/buildings.ts",
    defaultIntensity: 0.75,
  },
  [AudioCue.BuildingComplete]: {
    description: "Construcción completada (timbre característico para cada edificio).",
    recipeFile: "recipes/buildings.ts",
    defaultIntensity: 0.8,
    burstIntervalMs: 420,
    variants: BUILDING_VARIANTS,
  },
  [AudioCue.ArsenalQueued]: {
    description: "Encolar producción militar o cancelar cola en TradingPost/Silo/SAM.",
    recipeFile: "recipes/buildings.ts",
    defaultIntensity: 0.7,
    burstIntervalMs: 280,
    variants: [
      { label: "Queue Warship", subtype: ArsenalActionSubtype.QueueWarship },
      { label: "Queue Nuke", subtype: ArsenalActionSubtype.QueueNuke },
      { label: "Queue AntiNuke", subtype: ArsenalActionSubtype.QueueAntiNuke },
      { label: "Cancel", subtype: ArsenalActionSubtype.Cancel },
    ],
  },
  [AudioCue.BuildingDemolished]: {
    description: "Destrucción o demolición completa de una estructura.",
    recipeFile: "recipes/buildings.ts",
    defaultIntensity: 0.85,
  },
  [AudioCue.BuildingUpgradeProgress]: {
    description: "Pulso rítmico singleton mientras >=1 edificio propio está en mejora.",
    recipeFile: "recipes/buildings.ts",
    defaultIntensity: 0.5,
    burstIntervalMs: 420,
    variants: [
      { label: "Progreso 20%", subtype: 0, intensity: 0.2 },
      { label: "Progreso 50%", subtype: 0, intensity: 0.5 },
      { label: "Progreso 85%", subtype: 0, intensity: 0.85 },
      { label: "Múltiples obras (2+)", subtype: 2, intensity: 0.65 },
    ],
  },
  [AudioCue.BuildingModeEnter]: {
    description: "Entrar, cambiar edificio seleccionado o salir del modo construcción.",
    recipeFile: "recipes/buildings.ts",
    defaultIntensity: 0.65,
    burstIntervalMs: 280,
    variants: [
      { label: "Enter", subtype: BuildingModeSubtype.Enter },
      { label: "Switch", subtype: BuildingModeSubtype.Switch },
      { label: "Exit", subtype: BuildingModeSubtype.Exit },
    ],
  },
  [AudioCue.BuildingSelect]: {
    description: "Seleccionar o inspeccionar un edificio propio en el mapa.",
    recipeFile: "recipes/buildings.ts",
    defaultIntensity: 0.65,
    burstIntervalMs: 280,
    variants: BUILDING_VARIANTS,
  },

  // ── Nuclear & SAM Defense (26–30) ────────────────────────────────────
  [AudioCue.NukeArmed]: {
    description: "Armar / seleccionar ojiva nuclear en el silo antes de lanzar.",
    recipeFile: "recipes/nukes.ts",
    defaultIntensity: 0.8,
    burstIntervalMs: 380,
    variants: NUKE_LEVEL_VARIANTS.slice(0, 3),
  },
  [AudioCue.NukeLaunch]: {
    description: "Ignición y lanzamiento de misil nuclear desde el silo.",
    recipeFile: "recipes/nukes.ts",
    defaultIntensity: 0.85,
    burstIntervalMs: 520,
    variants: NUKE_LEVEL_VARIANTS.slice(0, 3),
  },
  [AudioCue.MirvSplit]: {
    description: "Separación en vuelo de un portador MIRV en múltiples ojivas.",
    recipeFile: "recipes/nukes.ts",
    defaultIntensity: 0.85,
  },
  [AudioCue.SamIntercept]: {
    description: "Intercepción aérea exitosa de un misil entrante por lanzador SAM.",
    recipeFile: "recipes/nukes.ts",
    defaultIntensity: 0.8,
  },
  [AudioCue.NukeImpact]: {
    description: "Detonación nuclear (escala de sub-bass y onda expansiva por nivel).",
    recipeFile: "recipes/nukes.ts",
    defaultIntensity: 0.9,
    burstIntervalMs: 650,
    variants: NUKE_LEVEL_VARIANTS,
  },

  // ── Diplomacy, Abilities & Emotes (31–37) ────────────────────────────
  [AudioCue.AllianceRequest]: {
    description: "Enviar o recibir una solicitud diplomática de alianza.",
    recipeFile: "recipes/diplomacy.ts",
    defaultIntensity: 0.7,
  },
  [AudioCue.AllianceFormed]: {
    description: "Sellar y confirmar una nueva alianza.",
    recipeFile: "recipes/diplomacy.ts",
    defaultIntensity: 0.8,
  },
  [AudioCue.AllianceEnded]: {
    description: "Expirar, rechazar o terminar una alianza existente.",
    recipeFile: "recipes/diplomacy.ts",
    defaultIntensity: 0.65,
  },
  [AudioCue.Betrayal]: {
    description: "Traicionar a un aliado (alta tensión dramática con ducking).",
    recipeFile: "recipes/diplomacy.ts",
    defaultIntensity: 0.9,
  },
  [AudioCue.ResourceTransfer]: {
    description: "Envío, recepción, solicitud, rechazo o pedido de auxilio de recursos.",
    recipeFile: "recipes/diplomacy.ts",
    defaultIntensity: 0.7,
    burstIntervalMs: 360,
    variants: [
      { label: "Sent", subtype: ResourceAudioSubtype.Sent },
      { label: "Received", subtype: ResourceAudioSubtype.Received },
      { label: "Requested", subtype: ResourceAudioSubtype.Requested },
      { label: "Rejected", subtype: ResourceAudioSubtype.Rejected },
      { label: "HelpCall", subtype: ResourceAudioSubtype.HelpCall },
    ],
  },
  [AudioCue.HeroAbility]: {
    description: "Activación de habilidad de héroe (9 firmas armónicas por familia).",
    recipeFile: "recipes/diplomacy.ts",
    defaultIntensity: 0.8,
    burstIntervalMs: 380,
    variants: [
      { label: "Strength", subtype: AbilityAudioFamily.Strength },
      { label: "Conquest", subtype: AbilityAudioFamily.Conquest },
      { label: "Income", subtype: AbilityAudioFamily.Income },
      { label: "Tower", subtype: AbilityAudioFamily.Tower },
      { label: "City", subtype: AbilityAudioFamily.City },
      { label: "Losses", subtype: AbilityAudioFamily.Losses },
      { label: "Alliance", subtype: AbilityAudioFamily.Alliance },
      { label: "Retreat", subtype: AbilityAudioFamily.Retreat },
      { label: "Naval", subtype: AbilityAudioFamily.Naval },
    ],
  },
  [AudioCue.Emote]: {
    description: "Enviar o recibir un emote diplomático sobre el mapa.",
    recipeFile: "recipes/diplomacy.ts",
    defaultIntensity: 0.7,
    burstIntervalMs: 340,
    variants: [
      { label: "Greet", subtype: EmoteAudioKind.Greet },
      { label: "Help", subtype: EmoteAudioKind.Help },
      { label: "Threat", subtype: EmoteAudioKind.Threat },
      { label: "Celebrate", subtype: EmoteAudioKind.Celebrate },
    ],
  },

  // ── Match Lifecycle & Outcomes (38–41) ───────────────────────────────
  [AudioCue.MatchPhaseStart]: {
    description: "Inicio de fase de despliegue (Spawning) y comienzo real de partida.",
    recipeFile: "recipes/match.ts",
    defaultIntensity: 0.85,
    burstIntervalMs: 520,
    variants: [
      { label: "Spawning", subtype: MatchPhaseAudioKind.Spawning },
      { label: "Playing", subtype: MatchPhaseAudioKind.Playing },
    ],
  },
  [AudioCue.ActionError]: {
    description: "Acción inválida (sin oro, casilla ilegal o sin capacidad).",
    recipeFile: "recipes/match.ts",
    defaultIntensity: 0.65,
  },
  [AudioCue.Victory]: {
    description: "Fanfarria completa de victoria al ganar la partida.",
    recipeFile: "recipes/match.ts",
    defaultIntensity: 0.95,
  },
  [AudioCue.Defeat]: {
    description: "Cierre solemne de derrota al ser eliminado.",
    recipeFile: "recipes/match.ts",
    defaultIntensity: 0.9,
  },

  // ── GUI / UX-UI & Subtle AAA Micro-Interactions (42–56, 60–65) ───────
  [AudioCue.UiHover]: {
    description: "Hover sutil al pasar el cursor sobre botones, CTAs y tarjetas (con glide de tono).",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.5,
    burstIntervalMs: 110,
    variants: [
      { label: "Standard", subtype: UiHoverSubtype.Standard, intensity: 0.5 },
      { label: "Primary CTA", subtype: UiHoverSubtype.Primary, intensity: 0.5 },
      { label: "Card (inicio)", subtype: UiHoverSubtype.Card, intensity: 0.0 },
      { label: "Card (medio)", subtype: UiHoverSubtype.Card, intensity: 0.5 },
      { label: "Card (final)", subtype: UiHoverSubtype.Card, intensity: 1.0 },
    ],
  },
  [AudioCue.UiPress]: {
    description: "Retén táctil amortiguado al bajar el botón (pointerdown), antes de soltar clic.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.5,
    burstIntervalMs: 160,
    variants: [
      { label: "Standard", subtype: UiPressSubtype.Standard },
      { label: "Primary", subtype: UiPressSubtype.Primary },
      { label: "Soft", subtype: UiPressSubtype.Soft },
    ],
  },
  [AudioCue.UiClick]: {
    description: "Clic principal de botones en menú y diálogos.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.6,
    burstIntervalMs: 210,
    variants: [
      { label: "Default", subtype: 0 },
      { label: "Primary CTA", subtype: 1 },
      { label: "Back / Cancel", subtype: 2 },
    ],
  },
  [AudioCue.UiBattleLaunch]: {
    description: "Menú de Batalla: entrar a submenús, lanzar partida o pasos de mapa/semilla.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.7,
    burstIntervalMs: 260,
    variants: [
      { label: "Submenu", subtype: UiBattleLaunchSubtype.Submenu },
      { label: "Launch (Play/Join)", subtype: UiBattleLaunchSubtype.Launch },
      { label: "TactileStep", subtype: UiBattleLaunchSubtype.TactileStep },
    ],
  },
  [AudioCue.UiHeroBrowse]: {
    description: "Explorar tarjetas de héroes o episodios de campaña.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.5,
    burstIntervalMs: 190,
    variants: [
      { label: "Available (izq)", subtype: UiHeroBrowseSubtype.Available, intensity: 0.15 },
      { label: "Available (der)", subtype: UiHeroBrowseSubtype.Available, intensity: 0.85 },
      { label: "Locked", subtype: UiHeroBrowseSubtype.Locked, intensity: 0.5 },
      { label: "Random", subtype: UiHeroBrowseSubtype.Random, intensity: 0.5 },
    ],
  },
  [AudioCue.UiInfoTab]: {
    description: "Pestañas del panel de Info de Héroe (Resumen/Bio/Logros), chips de filtro y switches.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.5,
    burstIntervalMs: 210,
    variants: [
      { label: "Tab · Resumen", subtype: UiInfoTabSubtype.Tab, intensity: 0.0 },
      { label: "Tab · Biografía", subtype: UiInfoTabSubtype.Tab, intensity: 0.5 },
      { label: "Tab · Logros", subtype: UiInfoTabSubtype.Tab, intensity: 1.0 },
      { label: "Chip / Filtro", subtype: UiInfoTabSubtype.Chip, intensity: 0.5 },
      { label: "Switch ON", subtype: UiInfoTabSubtype.Switch, intensity: 0.85 },
      { label: "Switch OFF", subtype: UiInfoTabSubtype.Switch, intensity: 0.15 },
    ],
  },
  [AudioCue.UiPurchaseAction]: {
    description: "Flujo de compra en Héroes y Tienda (abrir confirmación, confirmar gasto o denegado).",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.7,
    burstIntervalMs: 320,
    variants: [
      { label: "Open (abrir modal)", subtype: UiPurchaseActionSubtype.Open },
      { label: "Confirm (comprar)", subtype: UiPurchaseActionSubtype.Confirm },
      { label: "Denied (insuficiente)", subtype: UiPurchaseActionSubtype.Denied },
    ],
  },
  [AudioCue.UiPurchaseSuccess]: {
    description: "Celebración dorada al completar una compra y revelar el ítem.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.9,
  },
  [AudioCue.UiHeroEquip]: {
    description: "Confirmar y equipar un héroe o skin.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.75,
  },
  [AudioCue.UiTabSwitch]: {
    description: "Transición entre pantallas principales (Batalla, Héroes, Tienda, Perfil).",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.6,
    burstIntervalMs: 260,
    variants: [
      { label: "Neutral", subtype: 0 },
      { label: "Forward", subtype: 1 },
      { label: "Back", subtype: 2 },
    ],
  },
  [AudioCue.UiModalOpen]: {
    description: "Abrir modales, panel de Info de Héroe, ajustes o paneles del HUD.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.65,
  },
  [AudioCue.UiModalClose]: {
    description: "Cerrar modales o paneles del HUD.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.6,
  },
  [AudioCue.UiRadialOpen]: {
    description: "Desplegar el menú radial sobre el mapa en partida.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.65,
  },
  [AudioCue.UiRadialClose]: {
    description: "Cerrar el menú radial sobre el mapa.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.6,
  },
  [AudioCue.UiSliderTick]: {
    description: "Paso táctil al arrastrar sliders de volumen, Crear Partida o ratio de ataque.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.5,
    burstIntervalMs: 95,
    variants: [
      { label: "10%", subtype: 0, intensity: 0.1 },
      { label: "35%", subtype: 0, intensity: 0.35 },
      { label: "60%", subtype: 0, intensity: 0.6 },
      { label: "85%", subtype: 0, intensity: 0.85 },
      { label: "100%", subtype: 0, intensity: 1.0 },
    ],
  },
  [AudioCue.UiToast]: {
    description: "Notificaciones toast del HUD escaladas por prioridad (1..4).",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.7,
    burstIntervalMs: 360,
    variants: [
      { label: "Prioridad 1", subtype: 1 },
      { label: "Prioridad 2", subtype: 2 },
      { label: "Prioridad 3", subtype: 3 },
      { label: "Prioridad 4 (VIP)", subtype: 4 },
    ],
  },
  [AudioCue.UiMatchFound]: {
    description: "Alerta épica cuando matchmaking encuentra rival/partida.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.9,
  },
  [AudioCue.UiCampaignStep]: {
    description: "Avanzar escena o diálogo en campaña y tutorial.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.6,
  },
  [AudioCue.UiCelebrationFanfare]: {
    description: "Inicio de ceremonia de recompensa (misiones/niveles).",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.9,
  },
  [AudioCue.UiCelebrationFly]: {
    description: "Vuelo de monedas/gemas hacia los contadores en celebraciones.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.7,
    burstIntervalMs: 110,
    burstCount: 5,
  },
  [AudioCue.UiCelebrationImpact]: {
    description: "Impacto final de recompensa sobre el contador de coronas/gemas.",
    recipeFile: "recipes/ui.ts",
    defaultIntensity: 0.8,
    burstIntervalMs: 130,
    burstCount: 4,
  },
};

function defaultRecipeFileForCategory(category: AudioCategory): string {
  switch (category) {
    case "combat":
      return "recipes/combat.ts";
    case "naval":
      return "recipes/naval.ts";
    case "building":
      return "recipes/buildings.ts";
    case "nuke":
      return "recipes/nukes.ts";
    case "diplomacy":
      return "recipes/diplomacy.ts";
    case "match":
      return "recipes/match.ts";
    case "ui":
      return "recipes/ui.ts";
  }
}

export function buildSoundCatalog(): CueCatalogEntry[] {
  const entries: CueCatalogEntry[] = [];
  const pairs = Object.entries(AudioCue) as [string, AudioCueId][];
  for (const [name, id] of pairs) {
    const def = CUE_REGISTRY[id];
    if (!def) continue;
    const meta = CUE_META[id];
    entries.push({
      id,
      name,
      category: def.category,
      def,
      description: meta?.description ?? `Cue procedural #${id} (${def.category}).`,
      recipeFile: meta?.recipeFile ?? defaultRecipeFileForCategory(def.category),
      variants: meta?.variants ?? [{ label: "Default", subtype: 0 }],
      defaultIntensity: meta?.defaultIntensity ?? 0.7,
      disabledInGame: meta?.disabledInGame ?? false,
      burstIntervalMs: meta?.burstIntervalMs ?? 200,
      burstCount: meta?.burstCount ?? 4,
    });
  }
  const catRank = (c: AudioCategory) => CATEGORY_ORDER.indexOf(c);
  entries.sort((a, b) => catRank(a.category) - catRank(b.category) || a.id - b.id);
  return entries;
}
