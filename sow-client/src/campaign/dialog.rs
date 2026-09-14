//! The campaign **dialog database** — one home for every scripted line (tutorial + future
//! episodes), so content grows here without touching UI code. Each line is rendered through the
//! bottom-panel takeover modal (a `BottomDialog` painted by `paint_dialog_contents`); add freely.
//!
//! Lines go through the emoji atlas pipeline, so emoji render fine; keep lines short — the panel
//! is compact.
//!
//! The speaker's *visual* (avatar / tribe emoji / empire disc) is built from the live snapshot at
//! render time, not stored here — this file is just the words.

/// One line of scripted speech (owned, so a generic line can fold in the faction name).
pub struct Line {
    pub speaker: String,
    pub title: String,
    pub body: String,
}

impl Line {
    fn new(speaker: &str, title: &str, body: &str) -> Line {
        Line {
            speaker: speaker.into(),
            title: title.into(),
            body: body.into(),
        }
    }
}

/// A faction's self-introduction, shown the first time the player's territory reaches it.
/// Always returns a line so **every** border contact prompts a greeting; named factions get a
/// written one, the rest a generic line (add specifics here as the campaign grows).
pub fn first_contact(faction: &str) -> Line {
    first_contact_for(crate::campaign::CampaignId::Boudica, faction)
}

/// Same as [`first_contact`] for a specific campaign. Six Sky lines respect the
/// written record (stelae, dates, names); where the record is silent the copy
/// says so instead of inventing facts.
pub fn first_contact_for(campaign: crate::campaign::CampaignId, faction: &str) -> Line {
    use crate::campaign::CampaignId as C;
    match campaign {
        C::Boudica => first_contact_boudica(faction),
        C::SixSkyEp1 | C::SixSkyEp2 | C::SixSkyEp3 => first_contact_six_sky(faction),
    }
}

fn first_contact_six_sky(faction: &str) -> Line {
    let (title, body) = match faction {
        // — Kin: the queen's own people (Team Red) —
        "Sa'aal Court" => (
            "Your court",
            "Sa'aal stands with you, queen from Dos Pilas. Naranjo is yours to rebuild — the old king fell to Caracol, and the court waited for you.",
        ),
        "Dos Pilas Honor Guard" => (
            "Father's guard",
            "We marched with you from Dos Pilas, daughter of B'alaj Chan K'awiil. The Calakmul alliance holds — while we stand.",
        ),
        "Naranjo Loyalists" => (
            "Sworn ally",
            "Loyalists of Naranjo. We kept the fires lit since the defeat. Command us, Lady Six Sky.",
        ),
        "K'ak' Tiliw Guard" => (
            "Your son's guard",
            "We guard the young king K'ak' Tiliw, born to you in 688. He is five years old — the city looks to you, regent.",
        ),
        "Dos Pilas Veterans" => (
            "Father's guard",
            "Veterans of Dos Pilas. Your father's house has not forgotten you — nor has Calakmul.",
        ),
        "Moon Priestesses" => (
            "Keepers of the count",
            "We keep the moon count with you, my lady. In 726 you will wear the goddess herself — but first, the war.",
        ),
        "Sa'aal Veterans" => (
            "Your court",
            "Veterans of Sa'aal. We burned with you, we bled with you. One more campaign, queen.",
        ),
        // — First-blood villages (independent, gray) —
        "Uaxactun Villagers" | "Uaxactun Rebels" => (
            "Lowland village",
            "Uaxactun minds its milpas. We are few, queen — be swift, and be merciful if you can.",
        ),
        "Yaxha Fishers" | "Yaxha Lookouts" | "Yaxha Rebels" => (
            "Lakeside people",
            "Yaxha fishes the lake and pays whoever holds the shore. Today that could be you.",
        ),
        "Tayasal Farmers" | "Tayasal Rebels" => (
            "Lowland village",
            "Tayasal plants and harvests. We have no quarrel with Naranjo — unless you bring one.",
        ),
        "Seibal Traders" | "Seibal Rebels" => (
            "River traders",
            "Seibal trades up and down the river. Spare our canoes and we will remember it.",
        ),
        "Topoxte" | "Topoxte Fishers" => (
            "Lakeside people",
            "Topoxte keeps to its island. The lake has fed us longer than any dynasty — do not drain it.",
        ),
        "Lamanai Fishers" => (
            "Lakeside people",
            "Lamanai endures, as always. Pass through, queen, and leave our nets alone.",
        ),
        "Altun Ha Traders" => (
            "River traders",
            "Altun Ha trades jade and salt. War is bad for business — but business adapts.",
        ),
        // — Vassals and neighbors (Team Blue, passive) —
        "Caracol Outpost" => (
            "Caracol's reach",
            "Caracol broke Naranjo once before. This outpost reminds you of it. Tear it down if you can.",
        ),
        "Tikal Tribute Post" => (
            "Tikal's reach",
            "Tikal collects from everyone, even in defeat. This post flies the great city's banner.",
        ),
        "Piedras Negras Garrison" => (
            "River garrison",
            "Piedras Negras holds the river crossing. We did not come for your war — but we will finish it.",
        ),
        "Copan Delegation" => (
            "Southern envoys",
            "Copan sends envoys, not armies. The south watches to see if a woman can hold Naranjo.",
        ),
        "Calakmul Emissaries" | "Calakmul Honor Guard" => (
            "The great ally",
            "Calakmul, head of the alliance your father serves. Prove Naranjo worthy and the serpent stands behind you.",
        ),
        "Palenque Envoys" => (
            "Western envoys",
            "Palenque sends words from the west. Words can become warriors — choose yours well.",
        ),
        "Yaxchilan Allies" => (
            "River allies",
            "Yaxchilan holds the upper river. Our canoes can carry warriors as easily as maize.",
        ),
        // — Bosses (Team Blue, expanding) —
        "Rival Claimant" => (
            "The old blood",
            "I am of the old line of Naranjo, of K'ahk' Xiiw Chan Chaahk. You are a stranger from Dos Pilas — this city is mine.",
        ),
        "Caracol" | "Caracol War Host" => (
            "The breaker",
            "Caracol, who left Naranjo kingless. You rebuilt what we broke, woman — we have come to break it again.",
        ),
        "Tikal Vanguard" => (
            "The great rival",
            "Tikal's vanguard. Calakmul's puppets do not impress the great city. Come and be buried, stranger queen.",
        ),
        "Tikal" | "Tikal Coalition" => (
            "The great city",
            "I am Tikal. Empires older than your dynasty have knelt to us. Burn your little fires, regent — the jungle remembers who endures.",
        ),
        "K'inichil Kab" => (
            "The unknown",
            "K'inichil Kab. The scribes argue over where we rule from — but Stela 24 will show you under the queen's feet all the same.",
        ),
        "Xunantunich Rebels" => (
            "Hill rebels",
            "Xunantunich bows to no regent. Our hill has stood before Naranjo and will stand after.",
        ),
        "Quirigua" => (
            "Southern watchers",
            "Quirigua watches the passes. We have survived greater queens than you — and lesser ones too.",
        ),
        other => {
            return Line {
                speaker: other.into(),
                title: "A city of the lowlands".into(),
                body: format!(
                    "The {other} watch your rising, queen of Naranjo. This is not yet our fight."
                ),
            };
        }
    };
    Line::new(faction, title, body)
}

fn first_contact_boudica(faction: &str) -> Line {
    let (title, body) = match faction {
        // — Independent clans (Caesar's 54 BC submitters): the first-blood targets —
        "Cassi" => (
            "Independent clan",
            "We are the Cassi, who knelt to Caesar a hundred years past. We are few, queen of the Iceni — be swift.",
        ),
        "Bibroci" => (
            "Independent clan",
            "Bibroci, we are. We pay Rome's grain-tithe to keep our hearths warm. We have no stomach for your war.",
        ),
        "Ancalites" => (
            "Independent clan",
            "The Ancalites feast with Rome's tax-men. You will find us soft, daughter of the Iceni.",
        ),
        "Segontiaci" => (
            "Independent clan",
            "Segontiaci — the last of the kneelers. Take us, and all the east marches under your banner.",
        ),
        // — Kin: Boudica's own people (Team Red) —
        "Trinovantes" => (
            "Sworn ally",
            "The Trinovantes stand with you, Boudica. Rome seized our land for Camulodunum — we have not forgotten.",
        ),
        "Venta Icenorum" => (
            "Iceni capital",
            "Venta Icenorum, your own seat, my queen. The Iceni rally to your standard.",
        ),
        "Snettisham" => (
            "Iceni sanctuary",
            "Snettisham, keepers of the Iceni gold. Our torcs and our spears are yours.",
        ),
        "Thetford" => (
            "Iceni stronghold",
            "Thetford, hearth of the Iceni. Lead us against the eagle.",
        ),
        "Stonea" => (
            "Iceni fort",
            "Stonea, where we first defied Rome's disarmament. We rise again with you.",
        ),
        // — Rome's vassals (client tribes, Team Blue) —
        "Catuvellauni" => (
            "Client of Rome",
            "The Catuvellauni serve Rome now. We will not throw away what the eagle has given us.",
        ),
        "Atrebates" => (
            "Client of Rome",
            "Atrebates, friends of Rome under good king Cogidubnus. We want no part of your rebellion.",
        ),
        "Cantiaci" => (
            "Client of Rome",
            "Cantiaci of Kent, the gateway to Rome. We stand with the legions.",
        ),
        // — City bosses + Rome (Team Blue) —
        "Camulodunum" => (
            "Roman colony",
            "I am Camulodunum, where the Temple of Claudius stands. You would not dare, savage.",
        ),
        "Londinium" => (
            "Roman city",
            "Londinium, jewel of Roman trade. Turn back, Iceni, or burn with us.",
        ),
        "Verulamium" => (
            "Roman city",
            "Verulamium, loyal to Rome. Your rebellion ends at our walls.",
        ),
        "Rome" => (
            "The Empire",
            "I am Rome. I have swallowed greater kingdoms than yours, woman. Kneel, or be crushed.",
        ),
        // — Generic: any other faction still greets you —
        other => {
            return Line {
                speaker: other.into(),
                title: "A people of Britain".into(),
                body: format!(
                    "The {other} watch your rising, queen of the Iceni. This is not yet our fight."
                ),
            };
        }
    };
    Line::new(faction, title, body)
}
