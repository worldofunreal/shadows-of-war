const lines: Record<string, { en: string[]; es: string[] }> = {
  City: {
    en: ["Settlement", "Village", "Town", "Walled City", "Capital", "Industrial City", "Metropolis", "Megacity"],
    es: ["Asentamiento", "Aldea", "Pueblo", "Ciudad amurallada", "Capital", "Ciudad industrial", "Metrópolis", "Megaciudad"],
  },
  Farm: {
    en: ["Military Camp", "Barracks", "Garrison", "Stronghold", "Military Base", "Mobilization Center", "Command Center", "Strategic Command"],
    es: ["Campamento militar", "Cuartel", "Guarnición", "Bastión militar", "Base militar", "Centro de movilización", "Centro de mando", "Mando estratégico"],
  },
  Factory: {
    en: ["Trading Post", "Market", "Trade Square", "Merchant Quarter", "Trade Depot", "Freight Terminal", "Logistics Hub", "Trade Complex"],
    es: ["Puesto comercial", "Mercado", "Plaza comercial", "Barrio mercantil", "Depósito comercial", "Terminal de carga", "Centro logístico", "Complejo comercial"],
  },
  Tower: {
    en: ["Watchtower", "Guard Tower", "Stone Tower", "Keep", "Bastion", "Artillery Bastion", "Defense Station", "Sentinel Tower"],
    es: ["Atalaya", "Torre de guardia", "Torre de piedra", "Torreón", "Bastión", "Bastión de artillería", "Estación defensiva", "Torre centinela"],
  },
  Port: {
    en: ["Fishing Pier", "Trading Dock", "Harbor", "Fortified Harbor", "Naval Yard", "Industrial Shipyard", "Fleet Base", "Fleet Command"],
    es: ["Muelle pesquero", "Muelle comercial", "Puerto", "Puerto fortificado", "Astillero naval", "Astillero industrial", "Base naval", "Mando naval"],
  },
  SAM: {
    en: ["SAM Battery I", "SAM Battery II", "SAM Battery III", "SAM Battery IV"],
    es: ["Batería SAM I", "Batería SAM II", "Batería SAM III", "Batería SAM IV"],
  },
  MissileSilo: {
    en: ["Missile Silo I", "Missile Silo II", "Missile Silo III", "Missile Silo IV"],
    es: ["Silo de misiles I", "Silo de misiles II", "Silo de misiles III", "Silo de misiles IV"],
  },
};

const eras = {
  en: ["Tribal", "Agrarian", "Classical", "Medieval", "Early Modern", "Industrial", "Contemporary", "Future"],
  es: ["Tribal", "Agraria", "Clásica", "Medieval", "Moderna temprana", "Industrial", "Contemporánea", "Futura"],
};

export const BUILDING_KINDS = Object.keys(lines);

export function buildingLine(kind: string, locale: string) {
  const language = locale.toLowerCase().startsWith("es") ? "es" : "en";
  return lines[kind]?.[language].map((name, index) => ({
    name,
    era: eras[language][lines[kind].en.length === 4 ? Math.min(7, index + 5) : index],
  })) ?? [];
}
