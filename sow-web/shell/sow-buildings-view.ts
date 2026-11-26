import { buildingLine, buildingPageText, buildingRole, type FoundationCardCopy } from "./building-lines.ts";
import {
  type BuildingAtlasManifest,
  escapeAttr,
  renderBuildingAtlasSprite,
  renderHudIcon,
  renderStatusIcon,
  renderUnitIcon,
} from "./sow-icons.ts";

export const VISIBLE_BUILDING_KINDS = [
  "City",
  "Farm",
  "TradingPost",
  "Port",
  "Tower",
  "MilitaryCamp",
] as const;

export type VisibleBuildingKind = (typeof VISIBLE_BUILDING_KINDS)[number];

export interface BuildingMetaEntry {
  cartBonusPct: number;
  isDistrict: boolean;
  coastalOnly: boolean;
  maxLevel: number;
}

export interface CityFoundationsMeta {
  baseRadius: number;
  maxRadius: number;
  radiusStep: number;
  minExclusion: number;
  maxExclusion: number;
  districtCaps: number[];
}

export interface BuildingCodexParams {
  locale: string;
  translate: (key: string) => string;
  atlas: BuildingAtlasManifest | null;
  atlasUrl: string;
  badgeAtlasUrl: string;
  homeHref: string;
  playHref: string;
  buildingPrices?: Record<string, number[]>;
  buildingEffects?: Record<string, Record<string, number>[]>;
  buildingAlternatives?: Record<string, number[]>;
  buildingMeta?: Record<string, BuildingMetaEntry>;
  buildingDurations?: Record<string, number[]>;
  cityFoundations?: CityFoundationsMeta;
}

const DEFAULT_META: Record<VisibleBuildingKind, BuildingMetaEntry> = {
  City: { cartBonusPct: 100, isDistrict: false, coastalOnly: false, maxLevel: 8 },
  Farm: { cartBonusPct: 75, isDistrict: true, coastalOnly: false, maxLevel: 8 },
  TradingPost: { cartBonusPct: 150, isDistrict: true, coastalOnly: false, maxLevel: 8 },
  Port: { cartBonusPct: 125, isDistrict: true, coastalOnly: true, maxLevel: 8 },
  Tower: { cartBonusPct: 25, isDistrict: true, coastalOnly: false, maxLevel: 8 },
  MilitaryCamp: { cartBonusPct: 50, isDistrict: true, coastalOnly: false, maxLevel: 8 },
};

const DEFAULT_FOUNDATIONS: CityFoundationsMeta = {
  baseRadius: 8,
  maxRadius: 22,
  radiusStep: 2,
  minExclusion: 16,
  maxExclusion: 44,
  districtCaps: [1, 2, 3],
};

const TRANSLATION_KEY_BY_KIND: Record<VisibleBuildingKind, string> = {
  City: "city",
  Farm: "farm",
  TradingPost: "trading_post",
  Port: "port",
  Tower: "tower",
  MilitaryCamp: "military_camp",
};

const STAT_DISPLAY_ORDER = [
  "gold",
  "trade",
  "troops",
  "reserve",
  "radius",
  "district_limit",
  "capture",
  "travel",
  "range",
  "defense",
  "resistance",
  "fleet",
  "docks",
  "repair",
  "speed",
  "charges",
  "fire_rate",
] as const;

function interpolateTokens(template: string, values: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (match, key) =>
    key in values ? String(values[key]) : match,
  );
}

function lineLabel(kind: VisibleBuildingKind, translate: (key: string) => string): string {
  return translate("hud.map_action_" + TRANSLATION_KEY_BY_KIND[kind]);
}

function statIconHtml(
  statKey: string,
  kind: VisibleBuildingKind,
  badgeAtlasUrl: string,
): string {
  const cls = "sow-bcodex__ico sow-bcodex__ico--sm";
  switch (statKey) {
    case "gold":
    case "trade":
      return renderStatusIcon("gold", cls, undefined, badgeAtlasUrl);
    case "troops":
      return renderStatusIcon("troops", cls, undefined, badgeAtlasUrl);
    case "reserve":
      return renderHudIcon("troops", cls);
    case "radius":
      return renderHudIcon("range", cls);
    case "district_limit":
      return renderHudIcon("ring_build", cls);
    case "capture":
      return renderHudIcon("ring_attack", cls);
    case "travel":
    case "speed":
      return renderStatusIcon("speed", cls, undefined, badgeAtlasUrl);
    case "range":
      return renderStatusIcon("range", cls, undefined, badgeAtlasUrl);
    case "defense":
      return renderStatusIcon("shield", cls, undefined, badgeAtlasUrl);
    case "resistance":
      return renderHudIcon("defense", cls);
    case "fleet":
      return renderUnitIcon("warship", cls);
    case "docks":
      return renderHudIcon("anchor", cls);
    case "repair":
      return renderHudIcon("construction", cls);
    case "charges":
      return kind === "Tower"
        ? renderUnitIcon("sam-missile", cls)
        : renderHudIcon("ring_nuke", cls);
    case "fire_rate":
      return renderHudIcon("speed", cls);
    default:
      return renderHudIcon("upgrade", cls);
  }
}

function formatStatValue(
  statKey: string,
  value: number,
  format: Intl.NumberFormat,
  tilesUnit: string,
  perKindUnit: string,
  isDelta = false,
): string {
  const num = format.format(value);
  const prefix = isDelta ? "+" : "";
  switch (statKey) {
    case "capture":
    case "speed":
      return `+${num}%`;
    case "defense":
      return `${prefix}${num}%`;
    case "travel":
      return `${prefix}${num}×`;
    case "radius":
    case "range":
      return isDelta ? `${prefix}${num}` : `${num} ${tilesUnit}`;
    case "district_limit":
      return isDelta ? `${prefix}${num}` : `${num} ${perKindUnit}`;
    default:
      return `${prefix}${num}`;
  }
}

function tierMilestoneTone(
  kind: VisibleBuildingKind,
  level: number,
): "gold" | "cyan" | "crimson" | "emerald" | null {
  if (kind === "City" && (level === 4 || level === 8)) return "emerald";
  if (kind === "TradingPost" && (level === 5 || level === 6)) return "gold";
  if (kind === "Tower" && level === 5) return "cyan";
  if (kind === "MilitaryCamp" && level >= 5 && level <= 7) return "crimson";
  return null;
}

function renderFoundationsOverview(params: {
  copy: ReturnType<typeof buildingPageText>;
  foundations: CityFoundationsMeta;
  metaMap: Record<VisibleBuildingKind, BuildingMetaEntry>;
  badgeAtlasUrl: string;
  translate: (key: string) => string;
}): string {
  const { copy, foundations, metaMap, badgeAtlasUrl, translate } = params;
  const tokens = {
    baseRadius: foundations.baseRadius,
    maxRadius: foundations.maxRadius,
    radiusStep: foundations.radiusStep,
    minExclusion: foundations.minExclusion,
    maxExclusion: foundations.maxExclusion,
    c1: foundations.districtCaps[0] ?? 1,
    c2: foundations.districtCaps[1] ?? 2,
    c3: foundations.districtCaps[2] ?? 3,
  };

  const pillars: Array<{ icon: string; card: FoundationCardCopy }> = [
    { icon: "city", card: copy.codex.foundations.cities },
    { icon: "ring_build", card: copy.codex.foundations.districts },
    { icon: "ring_send", card: copy.codex.foundations.trade },
    { icon: "ring_upgrade", card: copy.codex.foundations.evolutions },
  ];

  const pillarsHtml = pillars
    .map(
      ({ icon, card }) =>
        `<article class="sow-bcodex__pillar">` +
        `<div class="sow-bcodex__pillar-head">` +
        `<span class="sow-bcodex__pillar-crest" aria-hidden="true">${renderHudIcon(icon, "sow-bcodex__ico")}</span>` +
        `<div class="sow-bcodex__pillar-titles">` +
        `<h2>${escapeAttr(card.title)}</h2>` +
        `<span class="sow-bcodex__pillar-badge">${escapeAttr(interpolateTokens(card.badge, tokens))}</span>` +
        `</div>` +
        `</div>` +
        `<p class="sow-bcodex__pillar-body">${escapeAttr(interpolateTokens(card.body, tokens))}</p>` +
        `</article>`,
    )
    .join("");

  const sortedStops = [...VISIBLE_BUILDING_KINDS].sort(
    (a, b) => metaMap[b].cartBonusPct - metaMap[a].cartBonusPct,
  );
  const maxPct = Math.max(150, ...sortedStops.map((k) => metaMap[k].cartBonusPct));
  const cartStopsHtml = sortedStops
    .map((kind) => {
      const pct = metaMap[kind].cartBonusPct;
      const width = Math.max(12, Math.round((pct / maxPct) * 100));
      const label = lineLabel(kind, translate);
      return (
        `<a class="sow-bcodex__cart-stop" href="#${kind.toLowerCase()}" data-tier-pct="${pct}">` +
        `<div class="sow-bcodex__cart-stop-top">` +
        `<span class="sow-bcodex__cart-stop-name">${renderHudIcon(kind.toLowerCase(), "sow-bcodex__ico sow-bcodex__ico--sm")}<span>${escapeAttr(label)}</span></span>` +
        `<strong class="sow-bcodex__cart-stop-pct">${pct}%</strong>` +
        `</div>` +
        `<span class="sow-bcodex__cart-bar-track" aria-hidden="true"><span class="sow-bcodex__cart-bar-fill" style="width:${width}%"></span></span>` +
        `</a>`
      );
    })
    .join("");

  return (
    `<section class="sow-bcodex__foundations-wrap" aria-label="${escapeAttr(copy.codex.foundationsHeading)}">` +
    `<p class="sow-bcodex__section-kicker">${renderHudIcon("ring_build", "sow-bcodex__ico sow-bcodex__ico--sm")}<span>${escapeAttr(copy.codex.foundationsHeading)}</span></p>` +
    `<div class="sow-bcodex__foundations">${pillarsHtml}</div>` +
    `<div class="sow-bcodex__cart-meter">` +
    `<div class="sow-bcodex__cart-meter-head">` +
    `<span class="sow-bcodex__cart-meter-title">${renderStatusIcon("gold", "sow-bcodex__ico sow-bcodex__ico--sm", undefined, badgeAtlasUrl)}<span>${escapeAttr(copy.codex.cartStopBonus)}</span></span>` +
    `</div>` +
    `<div class="sow-bcodex__cart-meter-items">${cartStopsHtml}</div>` +
    `</div>` +
    `</section>`
  );
}

function directoryTagsHtml(
  kind: VisibleBuildingKind,
  copy: ReturnType<typeof buildingPageText>,
): string {
  const tags: string[] = [];
  if (kind === "City") {
    tags.push(`<span class="sow-bcodex__tag sow-bcodex__tag--emerald">${escapeAttr(copy.codex.coreAnchor)}</span>`);
  } else if (kind === "Port") {
    tags.push(`<span class="sow-bcodex__tag sow-bcodex__tag--cyan">${escapeAttr(copy.codex.roleTagCoastal)}</span>`);
  } else if (kind === "TradingPost") {
    tags.push(`<span class="sow-bcodex__tag sow-bcodex__tag--gold">${escapeAttr(copy.codex.roleTagFactory)}</span>`);
  } else if (kind === "Tower") {
    tags.push(`<span class="sow-bcodex__tag sow-bcodex__tag--cyan">${escapeAttr(copy.codex.roleTagSam)}</span>`);
  } else if (kind === "MilitaryCamp") {
    tags.push(`<span class="sow-bcodex__tag sow-bcodex__tag--crimson">${escapeAttr(copy.codex.roleTagNukes)}</span>`);
  }
  return tags.length ? `<div class="sow-bcodex__dir-tags">${tags.join("")}</div>` : "";
}

function renderHierarchyDirectory(params: {
  locale: string;
  copy: ReturnType<typeof buildingPageText>;
  metaMap: Record<VisibleBuildingKind, BuildingMetaEntry>;
  atlas: BuildingAtlasManifest | null;
  atlasUrl: string;
  translate: (key: string) => string;
}): string {
  const { locale, copy, metaMap, atlas, atlasUrl, translate } = params;

  const renderDirCard = (kind: VisibleBuildingKind, isCore: boolean) => {
    const label = lineLabel(kind, translate);
    const spriteHtml = renderBuildingAtlasSprite(kind, 1, atlas, atlasUrl, "sow-bcodex__dir-sprite");
    const roleIcon = renderHudIcon(kind.toLowerCase(), "sow-bcodex__ico sow-bcodex__ico--sm");
    const cartPct = metaMap[kind].cartBonusPct;
    return (
      `<a class="sow-bcodex__dir-card${isCore ? " sow-bcodex__dir-card--core" : ""}" href="#${kind.toLowerCase()}">` +
      `<span class="sow-bcodex__dir-pedestal" aria-hidden="true">${spriteHtml}</span>` +
      `<span class="sow-bcodex__dir-copy">` +
      `<span class="sow-bcodex__dir-title-row">` +
      `<span class="sow-bcodex__dir-title">${roleIcon}<span>${escapeAttr(label)}</span></span>` +
      `<span class="sow-bcodex__dir-cart" data-sow-tooltip="${escapeAttr(copy.codex.cartStopBonus + ": " + cartPct + "%")}">${cartPct}%</span>` +
      `</span>` +
      `<p class="sow-bcodex__dir-role">${escapeAttr(buildingRole(kind, locale))}</p>` +
      directoryTagsHtml(kind, copy) +
      `</span>` +
      `</a>`
    );
  };

  const districtCards = VISIBLE_BUILDING_KINDS.filter((k) => k !== "City")
    .map((k) => renderDirCard(k, false))
    .join("");

  return (
    `<nav class="sow-bcodex__directory" aria-label="${escapeAttr(copy.nav)}">` +
    `<div class="sow-bcodex__dir-group">` +
    `<span class="sow-bcodex__dir-label">${renderHudIcon("city", "sow-bcodex__ico sow-bcodex__ico--sm")}<span>${escapeAttr(copy.codex.coreAnchor)}</span></span>` +
    renderDirCard("City", true) +
    `</div>` +
    `<div class="sow-bcodex__dir-group">` +
    `<span class="sow-bcodex__dir-label">${renderHudIcon("ring_build", "sow-bcodex__ico sow-bcodex__ico--sm")}<span>${escapeAttr(copy.codex.cityDistrict)}</span></span>` +
    `<div class="sow-bcodex__dir-districts">${districtCards}</div>` +
    `</div>` +
    `</nav>`
  );
}

function renderFilterBar(
  copy: ReturnType<typeof buildingPageText>,
  translate: (key: string) => string,
): string {
  const buttons = [
    `<button class="sow-bcodex__filter-btn" type="button" data-codex-filter="all" aria-pressed="true">` +
      `${renderHudIcon("globe", "sow-bcodex__ico sow-bcodex__ico--sm")}<span>${escapeAttr(copy.codex.allLines)}</span></button>`,
    ...VISIBLE_BUILDING_KINDS.map(
      (kind) =>
        `<button class="sow-bcodex__filter-btn" type="button" data-codex-filter="${kind.toLowerCase()}" aria-pressed="false">` +
        `${renderHudIcon(kind.toLowerCase(), "sow-bcodex__ico sow-bcodex__ico--sm")}<span>${escapeAttr(lineLabel(kind, translate))}</span></button>`,
    ),
  ];

  return (
    `<div class="sow-bcodex__filter-bar" role="toolbar" aria-label="${escapeAttr(copy.nav)}">` +
    buttons.join("") +
    `</div>`
  );
}

function renderStatPill(params: {
  statKey: string;
  value: number;
  prevValue: number;
  level: number;
  kind: VisibleBuildingKind;
  wide: boolean;
  copy: ReturnType<typeof buildingPageText>;
  format: Intl.NumberFormat;
  badgeAtlasUrl: string;
}): string {
  const { statKey, value, prevValue, level, kind, wide, copy, format, badgeAtlasUrl } = params;
  const label = copy.stats[statKey];
  const valStr = formatStatValue(statKey, value, format, copy.codex.tilesUnit, copy.codex.perKindUnit);
  const delta = value - prevValue;
  const deltaHtml =
    level >= 2 && delta > 0.0001
      ? `<span class="sow-bcard__stat-delta">${escapeAttr(
          formatStatValue(statKey, delta, format, copy.codex.tilesUnit, copy.codex.perKindUnit, true),
        )}</span>`
      : "";
  const tooltipText =
    level >= 2 && delta > 0.0001
      ? `${label}: ${valStr} (${formatStatValue(statKey, delta, format, copy.codex.tilesUnit, copy.codex.perKindUnit, true)} ${copy.thisLevel})`
      : `${label}: ${valStr}`;

  return (
    `<li class="sow-bcard__stat${wide ? " sow-bcard__stat--wide" : ""}" data-sow-tooltip="${escapeAttr(tooltipText)}">` +
    `<span class="sow-bcard__stat-top">` +
    statIconHtml(statKey, kind, badgeAtlasUrl) +
    `<strong class="sow-bcard__stat-val">${escapeAttr(valStr)}</strong>` +
    deltaHtml +
    `</span>` +
    `<span class="sow-bcard__stat-label">${escapeAttr(label)}</span>` +
    `</li>`
  );
}

function renderMilestoneChips(params: {
  kind: VisibleBuildingKind;
  level: number;
  current: Record<string, number>;
  copy: ReturnType<typeof buildingPageText>;
  translate: (key: string) => string;
  format: Intl.NumberFormat;
  badgeAtlasUrl: string;
}): string {
  const { kind, level, current, copy, translate, format, badgeAtlasUrl } = params;
  const chips: string[] = [];
  const icoCls = "sow-bcodex__ico sow-bcodex__ico--sm";

  const reinforcement = Number(current.reinforcements ?? 0);
  if (reinforcement > 0) {
    chips.push(
      `<span class="sow-bcard__milestone">` +
        `${renderStatusIcon("troops", icoCls, undefined, badgeAtlasUrl)}` +
        `<span>+${escapeAttr(format.format(reinforcement))} ${escapeAttr(copy.immediate)}</span>` +
        `</span>`,
    );
  }

  if (kind === "City" && (level === 4 || level === 8)) {
    const cap = Number(current.district_limit ?? (level === 8 ? 3 : 2));
    chips.push(
      `<span class="sow-bcard__milestone sow-bcard__milestone--emerald">` +
        `${renderHudIcon("ring_build", icoCls)}` +
        `<span>${escapeAttr(interpolateTokens(copy.codex.districtCapUnlock, { n: cap }))}</span>` +
        `</span>`,
    );
  } else if (kind === "TradingPost" && level === 1) {
    chips.push(
      `<span class="sow-bcard__milestone sow-bcard__milestone--gold">` +
        `${renderHudIcon("trading_post", icoCls)}` +
        `<span>${escapeAttr(copy.route)}</span>` +
        `</span>`,
    );
  } else if (kind === "TradingPost" && level === 5) {
    chips.push(
      `<span class="sow-bcard__milestone sow-bcard__milestone--gold">` +
        `${renderHudIcon("trading_post", icoCls)}` +
        `<span>${escapeAttr(copy.codex.factoryUnlock)}</span>` +
        `</span>`,
    );
  } else if (kind === "TradingPost" && level === 6) {
    chips.push(
      `<span class="sow-bcard__milestone sow-bcard__milestone--gold">` +
        `${renderStatusIcon("speed", icoCls, undefined, badgeAtlasUrl)}` +
        `<span>${escapeAttr(copy.railway)}</span>` +
        `</span>`,
    );
  } else if (kind === "Tower" && level === 5) {
    chips.push(
      `<span class="sow-bcard__milestone sow-bcard__milestone--cyan">` +
        `${renderUnitIcon("sam-missile", "sow-bcard__unit-ico")}` +
        `<span>${escapeAttr(copy.codex.samUnlock)}</span>` +
        `</span>`,
    );
  } else if (kind === "MilitaryCamp" && level === 5) {
    chips.push(
      `<span class="sow-bcard__milestone sow-bcard__milestone--crimson">` +
        `${renderUnitIcon("atom-bomb", "sow-bcard__unit-ico")}` +
        `<span>${escapeAttr(translate("hud.nuclear_atombomb"))}</span>` +
        `</span>`,
    );
  } else if (kind === "MilitaryCamp" && level === 6) {
    chips.push(
      `<span class="sow-bcard__milestone sow-bcard__milestone--crimson">` +
        `${renderUnitIcon("hydrogen-bomb", "sow-bcard__unit-ico")}` +
        `<span>${escapeAttr(translate("hud.nuclear_hydrogenbomb"))}</span>` +
        `</span>`,
    );
  } else if (kind === "MilitaryCamp" && level === 7) {
    chips.push(
      `<span class="sow-bcard__milestone sow-bcard__milestone--crimson">` +
        `${renderUnitIcon("mirv", "sow-bcard__unit-ico")}` +
        `<span>${escapeAttr(translate("hud.nuclear_mirv"))}</span>` +
        `</span>`,
    );
  }

  return chips.length ? `<div class="sow-bcard__milestones">${chips.join("")}</div>` : "";
}

function renderBuildingTierCard(params: {
  kind: VisibleBuildingKind;
  level: number;
  name: string;
  era: string;
  stepPrice: number;
  cumulativePrice: number;
  alternativeCount: number;
  durationSec: number;
  current: Record<string, number>;
  previous: Record<string, number>;
  atlas: BuildingAtlasManifest | null;
  atlasUrl: string;
  badgeAtlasUrl: string;
  copy: ReturnType<typeof buildingPageText>;
  translate: (key: string) => string;
  format: Intl.NumberFormat;
}): string {
  const {
    kind,
    level,
    name,
    era,
    stepPrice,
    cumulativePrice,
    alternativeCount,
    durationSec,
    current,
    previous,
    atlas,
    atlasUrl,
    badgeAtlasUrl,
    copy,
    translate,
    format,
  } = params;

  const tone = tierMilestoneTone(kind, level);
  const milestoneAttrs = tone ? ` data-milestone="true" data-milestone-tone="${tone}"` : "";
  const spriteHtml = renderBuildingAtlasSprite(kind, level, atlas, atlasUrl, "sow-bcard__sprite");
  const actionLabel = level === 1 ? copy.build : copy.upgrade;
  const actionGlyph = renderHudIcon(level === 1 ? "ring_build" : "ring_upgrade", "sow-bcodex__ico sow-bcodex__ico--sm");
  const timeHtml =
    durationSec > 0
      ? `<span class="sow-bcard__cost-time" data-sow-tooltip="${escapeAttr(copy.stats.build_time + ": " + format.format(durationSec) + copy.codex.secondsUnit)}">${renderHudIcon("clock", "sow-bcodex__ico sow-bcodex__ico--sm")}<span>${escapeAttr(format.format(durationSec))}${escapeAttr(copy.codex.secondsUnit)}</span></span>`
      : "";
  const altHtml =
    level > 1 && alternativeCount > 0
      ? `<div class="sow-bcard__cost-alt" data-sow-tooltip="${escapeAttr(interpolateTokens(copy.codex.sameGoldTooltip, { n: format.format(alternativeCount) }))}" data-sow-tooltip-tone="gold"><span>${escapeAttr(copy.sameGold)}</span><strong>N1 × ${escapeAttr(format.format(alternativeCount))}</strong></div>`
      : "";

  const visibleKeys = STAT_DISPLAY_ORDER.filter(
    (key) => copy.stats[key] && Number(current[key] ?? 0) > 0,
  );
  const statsHtml = visibleKeys.length
    ? `<ul class="sow-bcard__stats">` +
      visibleKeys
        .map((key, idx) =>
          renderStatPill({
            statKey: key,
            value: Number(current[key] ?? 0),
            prevValue: Number(previous[key] ?? 0),
            level,
            kind,
            wide: visibleKeys.length % 2 === 1 && idx === visibleKeys.length - 1,
            copy,
            format,
            badgeAtlasUrl,
          }),
        )
        .join("") +
      `</ul>`
    : "";

  const milestonesHtml = renderMilestoneChips({
    kind,
    level,
    current,
    copy,
    translate,
    format,
    badgeAtlasUrl,
  });

  return (
    `<li class="sow-bcard"${milestoneAttrs}>` +
    `<article class="sow-bcard__surface">` +
    `<div class="sow-bcard__top">` +
    `<div class="sow-bcard__stage">` +
    `<span class="sow-bcard__level">N${level}</span>` +
    spriteHtml +
    `</div>` +
    `<div class="sow-bcard__head">` +
    `<div class="sow-bcard__title-block">` +
    `<h3 class="sow-bcard__title">${escapeAttr(name)}</h3>` +
    `<span class="sow-bcard__era">${escapeAttr(era)}</span>` +
    `</div>` +
    `<div class="sow-bcard__cost">` +
    `<div class="sow-bcard__cost-row">` +
    `<span class="sow-bcard__cost-action">${actionGlyph}<span>${escapeAttr(actionLabel)}</span></span>` +
    timeHtml +
    `</div>` +
    `<div class="sow-bcard__cost-row">` +
    `<strong class="sow-bcard__cost-gold">${renderHudIcon("gold", "sow-bcodex__ico sow-bcodex__ico--sm")}<span>${escapeAttr(format.format(stepPrice))}</span></strong>` +
    `<span class="sow-bcard__cost-total" data-sow-tooltip="${escapeAttr(copy.invested + ": " + format.format(cumulativePrice) + " " + copy.gold)}">Σ ${escapeAttr(format.format(cumulativePrice))}</span>` +
    `</div>` +
    altHtml +
    `</div>` +
    `</div>` +
    `</div>` +
    statsHtml +
    milestonesHtml +
    `</article>` +
    `</li>`
  );
}

function renderBuildingLineSection(params: {
  kind: VisibleBuildingKind;
  meta: BuildingMetaEntry;
  codexParams: BuildingCodexParams;
  copy: ReturnType<typeof buildingPageText>;
  format: Intl.NumberFormat;
}): string {
  const { kind, meta, codexParams, copy, format } = params;
  const { locale, translate, atlas, atlasUrl, badgeAtlasUrl } = codexParams;
  const tiers = buildingLine(kind, locale);
  const lineName = lineLabel(kind, translate);
  const idSlug = kind.toLowerCase();

  let cumulativePrice = 0;
  const cardsHtml = tiers
    .map(({ name, era }, index) => {
      const level = index + 1;
      const stepPrice = codexParams.buildingPrices?.[kind]?.[index] ?? 0;
      cumulativePrice += stepPrice;
      return renderBuildingTierCard({
        kind,
        level,
        name,
        era,
        stepPrice,
        cumulativePrice,
        alternativeCount: codexParams.buildingAlternatives?.[kind]?.[index] ?? 0,
        durationSec: codexParams.buildingDurations?.[kind]?.[index] ?? 0,
        current: codexParams.buildingEffects?.[kind]?.[index] ?? {},
        previous: codexParams.buildingEffects?.[kind]?.[index - 1] ?? {},
        atlas,
        atlasUrl,
        badgeAtlasUrl,
        copy,
        translate,
        format,
      });
    })
    .join("");

  const categoryTag = meta.isDistrict ? copy.codex.cityDistrict : copy.codex.coreAnchor;
  const categoryTone = meta.isDistrict ? "gold" : "emerald";
  const placementRule = !meta.isDistrict
    ? copy.codex.placementCity
    : meta.coastalOnly
      ? copy.codex.placementCoastal
      : copy.codex.placementDistrict;

  return (
    `<section class="sow-bcodex__line" id="${idSlug}" data-codex-section="${idSlug}" aria-labelledby="building-title-${idSlug}">` +
    `<header class="sow-bcodex__line-header">` +
    `<div class="sow-bcodex__line-identity">` +
    `<span class="sow-bcodex__line-emblem" aria-hidden="true">${renderBuildingAtlasSprite(kind, 5, atlas, atlasUrl, "sow-bcodex__line-sprite")}</span>` +
    `<div class="sow-bcodex__line-copy">` +
    `<h2 class="sow-bcodex__line-title" id="building-title-${idSlug}">` +
    `${renderHudIcon(kind.toLowerCase(), "sow-bcodex__ico sow-bcodex__ico--lg")}` +
    `<span>${escapeAttr(lineName)}</span>` +
    `<span class="sow-bcodex__tag sow-bcodex__tag--${categoryTone}">${escapeAttr(categoryTag)}</span>` +
    `</h2>` +
    `<p>${escapeAttr(buildingRole(kind, locale))}</p>` +
    `</div>` +
    `</div>` +
    `<div class="sow-bcodex__line-meta">` +
    `<span class="sow-bcodex__rule-pill sow-bcodex__rule-pill--cart">${renderStatusIcon("gold", "sow-bcodex__ico sow-bcodex__ico--sm", undefined, badgeAtlasUrl)}<span>${escapeAttr(copy.codex.cartStopBonus)}: ${meta.cartBonusPct}%</span></span>` +
    `<span class="sow-bcodex__rule-pill">${escapeAttr(placementRule)}</span>` +
    `<span class="sow-bcodex__rule-pill">${escapeAttr(copy.codex.maxLevel)}</span>` +
    `</div>` +
    `</header>` +
    `<ol class="sow-bcodex__grid">${cardsHtml}</ol>` +
    `</section>`
  );
}

export function renderBuildingCodex(codexParams: BuildingCodexParams): string {
  const { locale, translate, atlas, atlasUrl, badgeAtlasUrl, homeHref, playHref } = codexParams;
  const copy = buildingPageText(locale);
  const format = new Intl.NumberFormat(locale, { maximumFractionDigits: 2 });
  const metaMap = {
    City: codexParams.buildingMeta?.City ?? DEFAULT_META.City,
    Farm: codexParams.buildingMeta?.Farm ?? DEFAULT_META.Farm,
    TradingPost: codexParams.buildingMeta?.TradingPost ?? DEFAULT_META.TradingPost,
    Port: codexParams.buildingMeta?.Port ?? DEFAULT_META.Port,
    Tower: codexParams.buildingMeta?.Tower ?? DEFAULT_META.Tower,
    MilitaryCamp: codexParams.buildingMeta?.MilitaryCamp ?? DEFAULT_META.MilitaryCamp,
  };
  const foundations = codexParams.cityFoundations ?? DEFAULT_FOUNDATIONS;

  const breadcrumbs =
    `<nav class="guide-breadcrumbs" aria-label="Breadcrumb">` +
    `<a href="${escapeAttr(homeHref)}">${escapeAttr(translate("site.nav_home"))}</a>` +
    `<span aria-hidden="true">/</span>` +
    `<span aria-current="page">${escapeAttr(translate("site.buildings"))}</span>` +
    `</nav>`;

  const hero =
    `<header class="sow-bcodex__hero">` +
    `<div class="sow-bcodex__hero-copy">` +
    `<p class="eyebrow">${escapeAttr(copy.codex.eyebrow)}</p>` +
    `<h1>${escapeAttr(translate("site.buildings"))}</h1>` +
    `<p class="sow-bcodex__lede">${escapeAttr(copy.intro)}</p>` +
    `</div>` +
    `<a class="sow-bcodex__cta" href="${escapeAttr(playHref)}">` +
    `${renderHudIcon("ring_attack", "sow-bcodex__ico sow-bcodex__ico--sm")}` +
    `<span>${escapeAttr(translate("site.play_now"))}</span>` +
    `</a>` +
    `</header>`;

  const foundationsHtml = renderFoundationsOverview({
    copy,
    foundations,
    metaMap,
    badgeAtlasUrl,
    translate,
  });

  const directoryHtml = renderHierarchyDirectory({
    locale,
    copy,
    metaMap,
    atlas,
    atlasUrl,
    translate,
  });

  const filterBarHtml = renderFilterBar(copy, translate);

  const linesHtml = VISIBLE_BUILDING_KINDS.map((kind) =>
    renderBuildingLineSection({
      kind,
      meta: metaMap[kind],
      codexParams,
      copy,
      format,
    }),
  ).join("");

  return (
    `<div class="sow-bcodex" data-sow-buildings-codex>` +
    breadcrumbs +
    hero +
    foundationsHtml +
    directoryHtml +
    filterBarHtml +
    `<div class="sow-bcodex__lines">${linesHtml}</div>` +
    `<p class="sow-bcodex__footnote">${escapeAttr(copy.note)}</p>` +
    `</div>`
  );
}
