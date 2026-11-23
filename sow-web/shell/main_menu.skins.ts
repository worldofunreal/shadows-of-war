import { asset, esc, menuUi, root } from "./main_menu.core.ts";
import type { MenuSkinOffer, MenuState } from "./main_menu.state.ts";

export function skinName(skin: MenuSkinOffer) {
    const key = "store.skin_" + skin.id;
    return window.SOW_hasText?.(key) ? SOW_t(key) : skin.name;
}

export function skinIconAttributes(id: string, url: string): string {
    return "src='" + esc(url) + "' data-skin-icon='" + esc(id) + "'";
}

export function syncMenuSkinTheme(state: MenuState | null): void {
    const skin = state?.store?.skins?.find((item) => item.owned && item.id === state.selected_skin);
    const html = document.documentElement;
    const hud = document.getElementById("sow-hud");
    if (skin) {
        root.dataset.skin = skin.id;
        if (hud) hud.dataset.skin = skin.id;
        if (html.dataset.sowSkin !== skin.id) {
            const url = new URL(asset(skin.asset_path), document.baseURI).href;
            html.style.setProperty("--sow-skin-artwork", `url(${JSON.stringify(url)})`);
            html.dataset.sowSkin = skin.id;
        }
    } else {
        delete root.dataset.skin;
        if (hud) delete hud.dataset.skin;
        if (html.dataset.sowSkin !== undefined) {
            delete html.dataset.sowSkin;
            html.style.removeProperty("--sow-skin-artwork");
        }
    }
}

export function renderSkinsButton() {
    return (
        "<button class='sow-heroes__skins sow-store__buy' type='button' data-command='open_skin_picker'>" +
        esc(SOW_t("store.skins")) +
        "</button>"
    );
}

export function renderSkinCard(skin: MenuSkinOffer, lockedAction = "", discount = 0) {
    const equipped = skin.owned && menuUi.state?.selected_skin === skin.id;
    const discountBadge = !skin.owned && discount > 0;
    const action = skin.owned
        ? "<button class='sow-store__buy" + (equipped ? " sow-store__buy--equipped" : "") +
          "' type='button' data-command='" + (equipped ? "unequip_skin" : "equip_skin") +
          "' data-skin-id='" + esc(skin.id) +
          "' aria-pressed='" + String(equipped) + "'" + (menuUi.state?.store_busy ? " disabled" : "") +
          ">" + esc(SOW_t(equipped ? "store.equipped" : "store.equip")) +
          (equipped ? "<span aria-hidden='true'>×</span>" : "") + "</button>"
        : lockedAction;
    return (
        "<article class='sow-store__skin sow-store__skin--pattern" +
        (skin.owned ? " is-owned" : "") +
        (equipped ? " is-equipped" : "") +
        (discountBadge ? " is-on-sale" : "") +
        "' data-skin-card='" + esc(skin.id) +
        "'><div class='sow-store__skin-art'><img " + skinIconAttributes(skin.id, asset(skin.asset_path)) +
        " alt='' width='96' height='96' loading='lazy'>" +
        (discountBadge ? "<b class='sow-store__discount'>-" + discount + "%</b>" : "") +
        "</div><div class='sow-store__skin-body'><h3>" + esc(skinName(skin)) +
        "</h3>" + action + "</div></article>"
    );
}

export function renderSkinPicker(renderCard?: (skin: MenuSkinOffer) => string) {
    if (!menuUi.skinPickerOpen) return "";
    const skins = menuUi.state?.store?.skins || [];
    const cards = renderCard ? skins.map(renderCard) : skins.filter((skin) => skin.owned).map((skin) => renderSkinCard(skin));
    const title = esc(SOW_t("store.skins"));
    return (
        "<div class='sow-menu__overlay' data-menu-overlay='skin-picker'><section class='sow-menu__modal sow-skin-picker' role='dialog' aria-modal='true' aria-label='" +
        title + "'><div class='sow-menu__modal-head'><h2>" + title +
        "</h2><button class='sow-menu__icon-button' type='button' data-command='close_skin_picker' aria-label='" +
        esc(SOW_t("menu.close")) + "'>×</button></div><div class='sow-store__skin-grid sow-store__skin-grid--patterns'>" +
        (cards.join("") || "<p class='sow-menu__empty'>" + esc(SOW_t("store.offers_unavailable")) + "</p>") +
        "</div></section></div>"
    );
}
