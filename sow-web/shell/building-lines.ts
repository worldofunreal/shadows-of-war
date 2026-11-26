const lines: Record<string, { en: string[]; es: string[] }> = {
  City: {
    en: ["Settlement", "Village", "Town", "Walled City", "Capital", "Industrial City", "Metropolis", "Megacity"],
    es: ["Asentamiento", "Aldea", "Pueblo", "Ciudad amurallada", "Capital", "Ciudad industrial", "Metrópolis", "Megaciudad"],
  },
  MilitaryCamp: {
    en: ["Military Camp", "Barracks", "Garrison", "Stronghold", "Missile Silo", "Hydrogen Command", "MIRV Complex", "Strategic Command"],
    es: ["Campamento militar", "Cuartel", "Guarnición", "Bastión militar", "Silo de misiles", "Mando de hidrógeno", "Complejo MIRV", "Mando estratégico"],
  },
  Farm: {
    en: ["Homestead", "Farm", "Granary", "Estate", "Agri-District", "Collectivized Farms", "Agro-Combine", "Arcology Farm"],
    es: ["Granja", "Cultivo", "Granero", "Hacienda", "Distrito agrario", "Cooperativa agraria", "Complejo agrícola", "Arcología agraria"],
  },
  TradingPost: {
    en: ["Trading Post", "Market", "Blacksmith", "Merchant Quarter", "Factory", "Freight Terminal", "Logistics Hub", "Industrial Complex"],
    es: ["Puesto comercial", "Mercado", "Herrería", "Barrio mercantil", "Fábrica", "Terminal de carga", "Centro logístico", "Complejo industrial"],
  },
  Tower: {
    en: ["Watchtower", "Guard Tower", "Stone Tower", "Keep", "SAM Bastion", "Artillery SAM", "Defense Station", "Sentinel SAM"],
    es: ["Atalaya", "Torre de guardia", "Torre de piedra", "Torreón", "Bastión SAM", "Batería SAM", "Estación defensiva", "Torre centinela SAM"],
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

type BuildingKind = keyof typeof lines;

const localizedTiers: Record<string, Partial<Record<BuildingKind, string[]>>> = {
  ar: {
    City: ["مستوطنة", "قرية", "بلدة", "مدينة مسوّرة", "عاصمة", "مدينة صناعية", "حاضرة", "مدينة كبرى"],
    MilitaryCamp: ["معسكر عسكري", "ثكنة", "حامية", "حصن", "صومعة صواريخ", "قيادة الهيدروجين", "مجمع MIRV", "قيادة استراتيجية"],
    Farm: ["عزبة", "مزرعة", "مخزن حبوب", "ضيعة", "حي زراعي", "مزارع جماعية", "مجمع زراعي", "مزرعة بيئية"],
    TradingPost: ["مركز تجاري", "سوق", "حدّاد", "حي التجار", "مصنع", "محطة شحن", "مركز لوجستي", "مجمع صناعي"],
    Tower: ["برج مراقبة", "برج حراسة", "برج حجري", "قلعة", "حصن SAM", "مدفعية SAM", "محطة دفاع", "برج الحارس SAM"],
    Port: ["رصيف صيد", "رصيف تجاري", "ميناء", "ميناء محصن", "ترسانة بحرية", "حوض بناء سفن صناعي", "قاعدة الأسطول", "قيادة الأسطول"],
    SAM: ["بطارية SAM I", "بطارية SAM II", "بطارية SAM III", "بطارية SAM IV"],
    MissileSilo: ["صومعة صواريخ I", "صومعة صواريخ II", "صومعة صواريخ III", "صومعة صواريخ IV"],
  },
  de: {
    City: ["Siedlung", "Dorf", "Kleinstadt", "Mauerstadt", "Hauptstadt", "Industriestadt", "Metropole", "Megastadt"],
    MilitaryCamp: ["Militärlager", "Kaserne", "Garnison", "Festung", "Raketensilo", "Wasserstoff-Kommando", "MIRV-Komplex", "Strategisches Oberkommando"],
    Farm: ["Gehöft", "Bauernhof", "Kornspeicher", "Landgut", "Agrarbezirk", "Kollektivfarmen", "Agrarkombinat", "Arkologie-Farm"],
    TradingPost: ["Handelsposten", "Markt", "Schmiede", "Kaufmannsviertel", "Fabrik", "Frachtterminal", "Logistikzentrum", "Industriekomplex"],
    Tower: ["Wachturm", "Wachtturm", "Steinturm", "Bergfried", "SAM-Bastion", "SAM-Artillerie", "Verteidigungsstation", "SAM-Wächterturm"],
    Port: ["Fischersteg", "Handelskai", "Hafen", "Festungshafen", "Marinewerft", "Industrielle Werft", "Flottenstützpunkt", "Flottenkommando"],
    SAM: ["SAM-Batterie I", "SAM-Batterie II", "SAM-Batterie III", "SAM-Batterie IV"],
    MissileSilo: ["Raketensilo I", "Raketensilo II", "Raketensilo III", "Raketensilo IV"],
  },
  fil: {
    City: ["Pamayanan", "Nayon", "Bayan", "Napapaderang Lungsod", "Kabisera", "Lungsod Industriyal", "Kalakhang Lungsod", "Megasiyudad"],
    MilitaryCamp: ["Kampong Militar", "Kuwartel", "Garnison", "Muog", "Silo ng Misayl", "Utos ng Hidroheno", "Kompleks ng MIRV", "Estratikong Punong-himpilan"],
    Farm: ["Sakahan ng Pamilya", "Bukid", "Bodega ng Butil", "Hasyenda", "Distrito ng Agrikultura", "Kooperatibang Bukid", "Sentro ng Agro-Industriya", "Arkolohiyang Bukid"],
    TradingPost: ["Pamilihang Pangkalakalan", "Palengke", "Pandayan", "Distrito ng Mangangalakal", "Pabrika", "Terminal ng Kargamento", "Sentro ng Lohistika", "Kompleks Industriyal"],
    Tower: ["Tore ng Bantay", "Tore ng Guwardiya", "Tore na Bato", "Kuta", "Bastyon ng SAM", "Artileryang SAM", "Himpilan ng Depensa", "Tore ng Sentinel SAM"],
    Port: ["Pantalan ng Pangingisda", "Pantalan ng Kalakalan", "Daungan", "Pinatibay na Daungan", "Pagawaan ng Barko", "Pagawaan ng Barkong Industriyal", "Base ng Plota", "Punong-himpilan ng Plota"],
    SAM: ["Bateryang SAM I", "Bateryang SAM II", "Bateryang SAM III", "Bateryang SAM IV"],
    MissileSilo: ["Silo ng Misayl I", "Silo ng Misayl II", "Silo ng Misayl III", "Silo ng Misayl IV"],
  },
  fr: {
    City: ["Établissement", "Village", "Bourg", "Cité fortifiée", "Capitale", "Ville industrielle", "Métropole", "Mégapole"],
    MilitaryCamp: ["Camp militaire", "Caserne", "Garnison", "Place forte", "Silo de missiles", "Commandement hydrogène", "Complexe MIRV", "Commandement stratégique"],
    Farm: ["Ferme isolée", "Ferme", "Grenier", "Domaine agricole", "Quartier agricole", "Fermes collectivisées", "Combinat agricole", "Ferme arcologique"],
    TradingPost: ["Comptoir commercial", "Marché", "Forge", "Quartier marchand", "Usine", "Terminal de fret", "Centre logistique", "Complexe industriel"],
    Tower: ["Tour de guet", "Tour de garde", "Tour de pierre", "Donjon", "Bastion SAM", "Artillerie SAM", "Station défensive", "Sentinelle SAM"],
    Port: ["Jetée de pêche", "Quai marchand", "Port", "Port fortifié", "Chantier naval", "Chantier naval industriel", "Base navale", "Commandement de la flotte"],
    SAM: ["Batterie SAM I", "Batterie SAM II", "Batterie SAM III", "Batterie SAM IV"],
    MissileSilo: ["Silo de missiles I", "Silo de missiles II", "Silo de missiles III", "Silo de missiles IV"],
  },
  id: {
    City: ["Permukiman", "Desa", "Kota kecil", "Kota bertembok", "Ibu kota", "Kota industri", "Metropolis", "Megakota"],
    MilitaryCamp: ["Kamp Militer", "Barak", "Garnisun", "Benteng", "Silo Rudal", "Komando Hidrogen", "Kompleks MIRV", "Komando Strategis"],
    Farm: ["Ladang", "Pertanian", "Lumbung", "Perkebunan", "Distrik Tani", "Pertanian Kolektif", "Agro-Kombinat", "Pertanian Arkologi"],
    TradingPost: ["Pos Dagang", "Pasar", "Pandai Besi", "Kawasan Pedagang", "Pabrik", "Terminal Kargo", "Pusat Logistik", "Kompleks Industri"],
    Tower: ["Menara Pengawas", "Menara Jaga", "Menara Batu", "Benteng Dalam", "Bastion SAM", "Artileri SAM", "Pos Pertahanan", "Sentinel SAM"],
    Port: ["Dermaga Nelayan", "Dermaga Dagang", "Pelabuhan", "Pelabuhan Berbenteng", "Galangan Angkatan Laut", "Galangan Industri", "Pangkalan Armada", "Komando Armada"],
    SAM: ["Baterai SAM I", "Baterai SAM II", "Baterai SAM III", "Baterai SAM IV"],
    MissileSilo: ["Silo Rudal I", "Silo Rudal II", "Silo Rudal III", "Silo Rudal IV"],
  },
  it: {
    City: ["Insediamento", "Villaggio", "Borgo", "Città murata", "Capitale", "Città industriale", "Metropoli", "Megalopoli"],
    MilitaryCamp: ["Campo militare", "Caserma", "Guarnigione", "Roccaforte", "Silo missilistico", "Comando idrogeno", "Complesso MIRV", "Comando strategico"],
    Farm: ["Casale", "Fattoria", "Granaio", "Tenuta", "Distretto agricolo", "Fattorie collettive", "Complesso agricolo", "Fattoria arcologica"],
    TradingPost: ["Avamposto commerciale", "Mercato", "Fucina", "Quartiere mercantile", "Fabbrica", "Terminal merci", "Centro logistico", "Complesso industriale"],
    Tower: ["Torre di vedetta", "Torre di guardia", "Torre di pietra", "Mastio", "Bastione SAM", "Artiglieria SAM", "Stazione difensiva", "Sentinella SAM"],
    Port: ["Molo da pesca", "Molo commerciale", "Porto", "Porto fortificato", "Arsenale navale", "Cantiere industriale", "Base della flotta", "Comando della flotta"],
    SAM: ["Batteria SAM I", "Batteria SAM II", "Batteria SAM III", "Batteria SAM IV"],
    MissileSilo: ["Silo missilistico I", "Silo missilistico II", "Silo missilistico III", "Silo missilistico IV"],
  },
  ja: {
    City: ["開拓地", "村", "町", "城壁都市", "首都", "工業都市", "大都市", "メガシティ"],
    MilitaryCamp: ["軍事キャンプ", "兵舎", "守備隊", "要塞", "ミサイルサイロ", "水爆司令部", "MIRV複合施設", "戦略司令部"],
    Farm: ["開拓農場", "農場", "穀物倉", "荘園", "農業地区", "集団農場", "農業コンビナート", "アコロジー農場"],
    TradingPost: ["交易所", "市場", "鍛冶場", "商業地区", "工場", "貨物ターミナル", "物流拠点", "工業複合施設"],
    Tower: ["見張り台", "警備塔", "石塔", "天守", "SAM堡塁", "SAM砲台", "防衛拠点", "センチネルSAM"],
    Port: ["漁港桟橋", "交易桟橋", "港", "要塞港", "海軍工廠", "工業造船所", "艦隊基地", "艦隊司令部"],
    SAM: ["SAM砲台 I", "SAM砲台 II", "SAM砲台 III", "SAM砲台 IV"],
    MissileSilo: ["ミサイルサイロ I", "ミサイルサイロ II", "ミサイルサイロ III", "ミサイルサイロ IV"],
  },
  ko: {
    City: ["정착지", "마을", "소도시", "성벽 도시", "수도", "산업 도시", "대도시", "메가시티"],
    MilitaryCamp: ["군사 캠프", "병영", "주둔지", "요새", "미사일 사일로", "수소폭탄 사령부", "MIRV 단지", "전략 사령부"],
    Farm: ["개척 농가", "농장", "곡물 창고", "대농장", "농업 지구", "집단 농장", "농업 콤비나트", "아콜로지 농장"],
    TradingPost: ["교역소", "시장", "대장간", "상인 지구", "공장", "화물 터미널", "물류 허브", "산업 단지"],
    Tower: ["감시탑", "경비탑", "석탑", "성루", "SAM 보루", "SAM 포대", "방어 기지", "센티널 SAM"],
    Port: ["어업 부두", "교역 부두", "항구", "요새 항구", "해군 조선소", "산업 조선소", "함대 기지", "함대 사령부"],
    SAM: ["SAM 포대 I", "SAM 포대 II", "SAM 포대 III", "SAM 포대 IV"],
    MissileSilo: ["미사일 사일로 I", "미사일 사일로 II", "미사일 사일로 III", "미사일 사일로 IV"],
  },
  "pt-br": {
    City: ["Povoado", "Vila", "Cidade", "Cidade Murada", "Capital", "Cidade Industrial", "Metrópole", "Megacidade"],
    MilitaryCamp: ["Acampamento Militar", "Quartel", "Guarnição", "Fortaleza", "Silo de Mísseis", "Comando de Hidrogênio", "Complexo MIRV", "Comando Estratégico"],
    Farm: ["Sítio", "Fazenda", "Celeiro", "Herdade", "Distrito Agrário", "Fazendas Coletivas", "Agrocomplexo", "Fazenda Arcológica"],
    TradingPost: ["Entreposto Comercial", "Mercado", "Ferraria", "Bairro Mercantil", "Fábrica", "Terminal de Carga", "Centro Logístico", "Complexo Industrial"],
    Tower: ["Torre de Vigia", "Torre de Guarda", "Torre de Pedra", "Fortaleza", "Bastião SAM", "Artilharia SAM", "Estação de Defesa", "Sentinela SAM"],
    Port: ["Cais de Pesca", "Cais Comercial", "Porto", "Porto Fortificado", "Estaleiro Naval", "Estaleiro Industrial", "Base da Frota", "Comando da Frota"],
    SAM: ["Bateria SAM I", "Bateria SAM II", "Bateria SAM III", "Bateria SAM IV"],
    MissileSilo: ["Silo de Mísseis I", "Silo de Mísseis II", "Silo de Mísseis III", "Silo de Mísseis IV"],
  },
  ru: {
    City: ["Поселение", "Деревня", "Городок", "Город за стеной", "Столица", "Промышленный город", "Мегаполис", "Мегагород"],
    MilitaryCamp: ["Военный лагерь", "Казармы", "Гарнизон", "Крепость", "Ракетная шахта", "Водородное командование", "Комплекс MIRV", "Стратегическое командование"],
    Farm: ["Хутор", "Ферма", "Амбар", "Поместье", "Агрорайон", "Колхоз", "Агрокомбинат", "Аркологическая ферма"],
    TradingPost: ["Торговый пост", "Рынок", "Кузница", "Купеческий квартал", "Завод", "Грузовой терминал", "Логистический центр", "Промышленный комплекс"],
    Tower: ["Сторожевая башня", "Караульная башня", "Каменная башня", "Донжон", "Бастион SAM", "Батарея SAM", "Оборонительный пункт", "Часовой SAM"],
    Port: ["Рыбацкий причал", "Торговый причал", "Порт", "Укреплённый порт", "Военная верфь", "Промышленная верфь", "База флота", "Командование флота"],
    SAM: ["Батарея SAM I", "Батарея SAM II", "Батарея SAM III", "Батарея SAM IV"],
    MissileSilo: ["Ракетная шахта I", "Ракетная шахта II", "Ракетная шахта III", "Ракетная шахта IV"],
  },
  tr: {
    City: ["Yerleşim", "Köy", "Kasaba", "Surlu Şehir", "Başkent", "Sanayi Şehri", "Metropol", "Mega Şehir"],
    MilitaryCamp: ["Askerî Kamp", "Kışla", "Garnizon", "Hisar", "Füze Silosu", "Hidrojen Komutanlığı", "MIRV Kompleksi", "Stratejik Komutanlık"],
    Farm: ["Çiftlik Evi", "Çiftlik", "Tahıl Ambarı", "Malikane", "Tarım Bölgesi", "Kolektif Çiftlikler", "Tarım Kombinası", "Arkolojik Çiftlik"],
    TradingPost: ["Ticaret Karakolu", "Pazar", "Demirci", "Tüccar Mahallesi", "Fabrika", "Yük Terminali", "Lojistik Merkezi", "Sanayi Kompleksi"],
    Tower: ["Gözetleme Kulesi", "Muhafız Kulesi", "Taş Kule", "İç Kale", "SAM Tabyası", "SAM Topçusu", "Savunma İstasyonu", "SAM Nöbetçisi"],
    Port: ["Balıkçı İskelesi", "Ticaret İskelesi", "Liman", "Tahkimli Liman", "Donanma Tersanesi", "Sanayi Tersanesi", "Filo Üssü", "Filo Komutanlığı"],
    SAM: ["SAM Bataryası I", "SAM Bataryası II", "SAM Bataryası III", "SAM Bataryası IV"],
    MissileSilo: ["Füze Silosu I", "Füze Silosu II", "Füze Silosu III", "Füze Silosu IV"],
  },
  vi: {
    City: ["Khu định cư", "Làng", "Thị trấn", "Thành phố có tường", "Thủ đô", "Thành phố công nghiệp", "Đô thị lớn", "Siêu đô thị"],
    MilitaryCamp: ["Trại quân sự", "Doanh trại", "Đồn trú", "Thành trì", "Hầm tên lửa", "Bộ tư lệnh Hydro", "Tổ hợp MIRV", "Bộ chỉ huy chiến lược"],
    Farm: ["Trang trại nhỏ", "Nông trại", "Kho thóc", "Điền trang", "Khu nông nghiệp", "Nông trường tập thể", "Liên hợp nông nghiệp", "Nông trại sinh thái"],
    TradingPost: ["Trạm giao thương", "Chợ", "Lò rèn", "Khu thương nhân", "Nhà máy", "Bến hàng hóa", "Trung tâm hậu cần", "Tổ hợp công nghiệp"],
    Tower: ["Tháp canh", "Tháp gác", "Tháp đá", "Tháp thành", "Pháo đài SAM", "Pháo binh SAM", "Trạm phòng thủ", "Tháp canh SAM"],
    Port: ["Cầu cá", "Cầu cảng thương mại", "Cảng", "Cảng kiên cố", "Xưởng hải quân", "Xưởng đóng tàu công nghiệp", "Căn cứ hạm đội", "Bộ chỉ huy hạm đội"],
    SAM: ["Trận địa SAM I", "Trận địa SAM II", "Trận địa SAM III", "Trận địa SAM IV"],
    MissileSilo: ["Hầm tên lửa I", "Hầm tên lửa II", "Hầm tên lửa III", "Hầm tên lửa IV"],
  },
  "zh-cn": {
    City: ["定居点", "村庄", "城镇", "城墙城市", "首都", "工业城市", "大都会", "特大城市"],
    MilitaryCamp: ["军事营地", "兵营", "驻军营地", "要塞", "导弹发射井", "氢弹司令部", "MIRV综合体", "战略司令部"],
    Farm: ["农庄", "农场", "粮仓", "庄园", "农业区", "集体农场", "农业联合体", "生态建筑农场"],
    TradingPost: ["贸易站", "市场", "铁匠铺", "商人街区", "工厂", "货运站", "物流中心", "工业综合体"],
    Tower: ["瞭望塔", "警卫塔", "石塔", "主堡", "SAM堡垒", "SAM炮兵", "防御站", "哨卫SAM"],
    Port: ["渔业码头", "贸易码头", "港口", "设防港口", "海军船坞", "工业船厂", "舰队基地", "舰队司令部"],
    SAM: ["SAM防空炮台 I", "SAM防空炮台 II", "SAM防空炮台 III", "SAM防空炮台 IV"],
    MissileSilo: ["导弹发射井 I", "导弹发射井 II", "导弹发射井 III", "导弹发射井 IV"],
  },
};

export type FoundationCardCopy = {
  title: string;
  badge: string;
  body: string;
};

export type CodexCopy = {
  eyebrow: string;
  allLines: string;
  coreAnchor: string;
  cityDistrict: string;
  coastalOnly: string;
  cartStopBonus: string;
  foundationsHeading: string;
  tilesUnit: string;
  secondsUnit: string;
  perKindUnit: string;
  factoryUnlock: string;
  samUnlock: string;
  districtCapUnlock: string;
  placementCity: string;
  placementDistrict: string;
  placementCoastal: string;
  sameGoldTooltip: string;
  investedShort: string;
  maxLevel: string;
  roleTagCoastal: string;
  roleTagSam: string;
  roleTagNukes: string;
  roleTagFactory: string;
  foundations: {
    cities: FoundationCardCopy;
    districts: FoundationCardCopy;
    trade: FoundationCardCopy;
    evolutions: FoundationCardCopy;
  };
};

type PageLocale = {
  roles: Partial<Record<BuildingKind, string>>;
  eras: string[];
  stats: Record<string, string>;
  intro: string;
  note: string;
  level: string;
  era: string;
  build: string;
  upgrade: string;
  invested: string;
  thisLevel: string;
  sameGold: string;
  immediate: string;
  route: string;
  railway: string;
  industrial: string;
  nav: string;
  codex: CodexCopy;
};

const enCodex: CodexCopy = {
  eyebrow: "Strategic Codex",
  allLines: "All Structures",
  coreAnchor: "Core Anchor",
  cityDistrict: "City District",
  coastalOnly: "Coastal Only",
  cartStopBonus: "Cart Stop Bonus",
  foundationsHeading: "City & District Foundations",
  tilesUnit: "tiles",
  secondsUnit: "s",
  perKindUnit: "/ kind",
  factoryUnlock: "Factory Era",
  samUnlock: "SAM Anti-Missile",
  districtCapUnlock: "{n}× Districts / kind",
  placementCity: "Global · 2× radius spacing",
  placementDistrict: "Inside City · Lv ≤ City · 1–3 / kind",
  placementCoastal: "Coastal inside City · Lv ≤ City · 1–3 / kind",
  sameGoldTooltip: "Same budget as building {n} new N1 structures.",
  investedShort: "Total",
  maxLevel: "Max N8",
  roleTagCoastal: "Coastal",
  roleTagSam: "SAM N5+",
  roleTagNukes: "Nukes N5–7",
  roleTagFactory: "Factory N5+",
  foundations: {
    cities: {
      title: "Cities & Territory Radius",
      badge: "{baseRadius}–{maxRadius} tiles",
      body: "Found Cities anywhere with proportional spacing (2× construction radius of nearby Cities: {minExclusion}–{maxExclusion} tiles). Upgrading adds +{radiusStep} tiles of build radius per level, core troop capacity, and instant reinforcements.",
    },
    districts: {
      title: "Districts, Caps & Capture",
      badge: "1 / 2 / 3 per kind",
      body: "Build 5 specialized Districts freely inside an owned City's radius. Two limits apply: District Lv ≤ City Lv, and per-kind caps expand with City level ({c1} at N1–3, {c2} at N4–7, {c3} at N8). Cities and districts occupy separate tiles.",
    },
    trade: {
      title: "Connected Cart & Rail Trade",
      badge: "25%–150% stop",
      body: "Every new structure links into roads and railways. Passing carts and trains award gold scaled by each destination's Cart Stop Bonus—prioritizing Trading Posts (150%) and Ports (125%) for inland and coastal wealth.",
    },
    evolutions: {
      title: "Era Pivots & Superweapons",
      badge: "N5–N7 unlocks",
      body: "Structures transform at higher eras: Trading Posts evolve into Factories (N5) and Railways (N6), Defense Towers activate SAM anti-missile interception from N5 using the same radius, and Military Camps unlock Atomic (N5), Hydrogen (N6), and MIRV (N7).",
    },
  },
};

const esCodex: CodexCopy = {
  eyebrow: "Códice estratégico",
  allLines: "Todos",
  coreAnchor: "Núcleo central",
  cityDistrict: "Distrito urbano",
  coastalOnly: "Solo costa",
  cartStopBonus: "Bono de parada",
  foundationsHeading: "Fundamentos de Ciudades y Distritos",
  tilesUnit: "casillas",
  secondsUnit: "s",
  perKindUnit: "/ tipo",
  factoryUnlock: "Era de Fábrica",
  samUnlock: "SAM antimisil",
  districtCapUnlock: "{n}× Distritos / tipo",
  placementCity: "Global · separación 2× radio",
  placementDistrict: "En Ciudad · Nv ≤ Ciudad · 1–3 / tipo",
  placementCoastal: "Costa en Ciudad · Nv ≤ Ciudad · 1–3 / tipo",
  sameGoldTooltip: "El costo equivale a construir {n} estructuras N1 nuevas.",
  investedShort: "Total",
  maxLevel: "Máx. N8",
  roleTagCoastal: "Costero",
  roleTagSam: "SAM N5+",
  roleTagNukes: "Nucleares N5–7",
  roleTagFactory: "Fábrica N5+",
  foundations: {
    cities: {
      title: "Ciudades y radio de control",
      badge: "{baseRadius}–{maxRadius} casillas",
      body: "Funda Ciudades en terreno propio respetando el doble del radio de ciudades cercanas ({minExclusion}–{maxExclusion} casillas). Cada mejora añade +{radiusStep} casillas de construcción, máxima capacidad de tropas y refuerzos inmediatos.",
    },
    districts: {
      title: "Distritos, límites y conquista",
      badge: "1 / 2 / 3 por tipo",
      body: "Construye los 5 Distritos libremente dentro del radio de una Ciudad propia. Reglas: Nivel del distrito ≤ Nivel de la Ciudad, y el límite por tipo sube con la Ciudad ({c1} en N1–3, {c2} en N4–7, {c3} en N8). Cada casilla se conquista por separado.",
    },
    trade: {
      title: "Red conectada de carretas y rieles",
      badge: "25%–150% parada",
      body: "Cada nueva estructura se conecta a la red de caminos y ferrocarriles. Las carretas y trenes pagan oro según el Bono de parada de cada destino, premiando Puestos comerciales/Fábricas (150%) y Puertos (125%).",
    },
    evolutions: {
      title: "Saltos de era y superarmas",
      badge: "Desbloqueos N5–N7",
      body: "En niveles altos cambian de rol: Puesto comercial evoluciona a Fábrica (N5) y ferrocarril (N6), la Torre activa defensa antimisil SAM desde N5 con su mismo radio, y el Campamento Militar desbloquea Atómica (N5), Hidrógeno (N6) y MIRV (N7).",
    },
  },
};

function makeCodex(partial: Partial<Omit<CodexCopy, "foundations">> & {
  foundations?: Partial<CodexCopy["foundations"]>;
}): CodexCopy {
  return {
    ...enCodex,
    ...partial,
    foundations: {
      cities: partial.foundations?.cities ?? enCodex.foundations.cities,
      districts: partial.foundations?.districts ?? enCodex.foundations.districts,
      trade: partial.foundations?.trade ?? enCodex.foundations.trade,
      evolutions: partial.foundations?.evolutions ?? enCodex.foundations.evolutions,
    },
  };
}

const pageLocales: Record<string, PageLocale> = {
  en: {
    roles: {
      City: "Primary troop housing, build radius (+2 tiles/lv), instant upgrade reinforcements, and district level cap.",
      Farm: "Balanced troop output, steady gold income, housing, and agricultural support inside City territory.",
      TradingPost: "Maximum gold income and connected trade routes; evolves into Blacksmith (N3), Factory (N5), and Rail Hubs (N6+).",
      Port: "Warships, fleet capacity, ship repair, global maritime speed, and top coastal cart trade bonuses.",
      Tower: "Area troop defense, capture resistance, and automatic SAM anti-missile interception from N5+ sharing its radius.",
      MilitaryCamp: "Peak troop production, housing, faster land conquest, and nuclear launches (Atom N5, Hydrogen N6, MIRV N7).",
      SAM: "Automatic interception of incoming nuclear weapons.",
      MissileSilo: "Launch capacity for atomic, hydrogen and MIRV weapons.",
    },
    eras: ["Tribal", "Agrarian", "Classical", "Medieval", "Early Modern", "Industrial", "Contemporary", "Future"],
    stats: {
      reserve: "max troops",
      troops: "troops/s",
      gold: "gold/s",
      radius: "build radius",
      district_limit: "districts/kind",
      reinforcements: "reinforcements",
      capture: "conquest speed",
      trade: "trade gold/s",
      travel: "route speed",
      range: "defense range",
      resistance: "capture resist",
      defense: "loss reduction",
      fleet: "fleet slots",
      docks: "active boats",
      repair: "hull repair/s",
      speed: "maritime speed",
      charges: "ordnance salvos",
      fire_rate: "interceptors/min",
      build_time: "build time",
    },
    intro: "Found Cities across the map to claim territory, then specialize each urban radius with 5 Districts linked into one connected trade and war economy.",
    note: "Values show single-structure prices, build times, cumulative totals at Normal speed, and upgrade deltas added at each tier.",
    level: "Level", era: "Era", build: "Build", upgrade: "Upgrade", invested: "invested", thisLevel: "this level", sameGold: "Same budget", immediate: "Instant troops",
    route: "Connected land routes from N1", railway: "Railways unlock at N6", industrial: "Factory (N5) · SAM (N5) · Nukes (N5–N7)", nav: "Building lines",
    codex: enCodex,
  },
  es: {
    roles: {
      City: "Máximo alojamiento de tropas, radio de construcción (+2 casillas/nv), refuerzos al mejorar y límite de nivel para distritos.",
      Farm: "Balance de tropas, oro moderado y alojamiento dentro del territorio urbano.",
      TradingPost: "Máxima generación de oro y rutas comerciales conectadas; evoluciona a Herrería (N3), Fábrica (N5) y ferrocarril (N6+).",
      Port: "Buques de guerra, flota simultánea, reparación naval, velocidad marítima global y gran bono a carretas.",
      Tower: "Defensa de área y resistencia a captura; desde N5+ activa intercepción antimisil SAM usando el mismo radio.",
      MilitaryCamp: "Máxima producción de tropas, alojamiento, velocidad de conquista y lanzamientos nucleares (Atómica N5, Hidrógeno N6, MIRV N7).",
      SAM: "Intercepción automática de armas nucleares entrantes.",
      MissileSilo: "Capacidad para lanzar armas atómicas, de hidrógeno y MIRV.",
    },
    eras: ["Tribal", "Agraria", "Clásica", "Medieval", "Moderna temprana", "Industrial", "Contemporánea", "Futura"],
    stats: {
      reserve: "tropas máx.",
      troops: "tropas/s",
      gold: "oro/s",
      radius: "radio urbano",
      district_limit: "distritos/tipo",
      reinforcements: "refuerzos",
      capture: "vel. conquista",
      trade: "oro comercial/s",
      travel: "vel. de ruta",
      range: "alcance",
      resistance: "resist. captura",
      defense: "reducción bajas",
      fleet: "plazas flota",
      docks: "barcos simult.",
      repair: "reparación/s",
      speed: "vel. marítima",
      charges: "cargas",
      fire_rate: "interceptores/min",
      build_time: "tiempo",
    },
    intro: "Funda Ciudades para expandir tu dominio y especializa su radio con 5 Distritos conectados a una red viva de caminos, ferrocarriles y comercio.",
    note: "Los precios muestran el costo por estructura a velocidad Normal, tiempo de obra, total acumulado y la mejora neta de cada nivel.",
    level: "Nivel", era: "Era", build: "Construir", upgrade: "Mejorar", invested: "invertido", thisLevel: "este nivel", sameGold: "Mismo oro", immediate: "Refuerzo inmediato",
    route: "Rutas terrestres conectadas desde N1", railway: "Ferrocarriles en N6", industrial: "Fábrica (N5) · SAM (N5) · Nucleares (N5–N7)", nav: "Líneas de edificios",
    codex: esCodex,
  },
  ar: {
    roles: {
      City: "سعة القوات الأساسية ونطاق البناء (+2 خانة/مستوى) وتعزيزات الترقية وسقف مستوى المقاطعات.",
      Farm: "توازن بين إنتاج القوات ودخل الذهب وسعة القوات داخل نطاق المدينة.",
      TradingPost: "أعلى دخل ذهب وشبكة طرق متصلة؛ يتطور إلى مصنع في المستوى 5 وسكك حديد في 6.",
      Port: "سفن حربية وسعة الأسطول وإصلاح السفن وسرعة بحرية شاملة ومكافأة قوافل عالية.",
      Tower: "دفاع المنطقة ومقاومة الاستيلاء؛ يفعل اعتراض SAM المضاد للصواريخ من المستوى 5+ بنفس النطاق.",
      MilitaryCamp: "أعلى إنتاج للقوات وسرعة فتح بري وإطلاق نووي من المستوى 5+ (ذرية N5، هيدروجين N6، MIRV N7).",
      SAM: "اعتراض تلقائي للأسلحة النووية القادمة.",
      MissileSilo: "إطلاق الأسلحة الذرية والهيدروجينية وMIRV.",
    },
    eras: ["قبلية", "زراعية", "كلاسيكية", "العصور الوسطى", "حديثة مبكرة", "صناعية", "معاصرة", "مستقبلية"],
    stats: {
      reserve: "أقصى قوات", troops: "قوات/ث", gold: "ذهب/ث", radius: "نطاق البناء", district_limit: "مقاطعة/نوع",
      reinforcements: "تعزيزات", capture: "سرعة الاستيلاء", trade: "ذهب/ث (طريق)", travel: "سرعة القافلة",
      range: "المدى", resistance: "مقاومة الاستيلاء", defense: "تقليل الخسائر", fleet: "سعة الأسطول",
      docks: "سفن متزامنة", repair: "إصلاح/ث", speed: "سرعة بحرية", charges: "شحنات", fire_rate: "اعتراض/دقيقة", build_time: "المدة",
    },
    intro: "أسس المدن للسيطرة على الأراضي، ثم ابنِ 5 مقاطعات داخل نطاقها متصلة بشبكة طرق وسكك حديدية وتجارة حية.",
    note: "الأسعار لمبنى واحد بالسرعة العادية: تكلفة الإنشاء والترقية والوقت والإجمالي التراكمي.",
    level: "المستوى", era: "العصر", build: "إنشاء", upgrade: "ترقية", invested: "الإجمالي", thisLevel: "هذا المستوى", sameGold: "بنفس الميزانية", immediate: "تعزيز فوري",
    route: "شبكة طرق متصلة من N1", railway: "سكك الحديد عند N6", industrial: "مصنع N5 · SAM N5 · نووي N5–N7", nav: "خطوط المباني",
    codex: makeCodex({
      eyebrow: "الموسوعة الاستراتيجية", allLines: "الكل", coreAnchor: "النواة الأساسية", cityDistrict: "مقاطعة حضرية", coastalOnly: "ساحلي فقط",
      cartStopBonus: "مكافأة توقف القافلة", foundationsHeading: "أسس المدن والمقاطعات", tilesUnit: "خانات", secondsUnit: "ث", perKindUnit: "/ نوع",
      factoryUnlock: "عصر المصنع", samUnlock: "دفاع SAM", districtCapUnlock: "{n}× مقاطعة / نوع",
    }),
  },
  de: {
    roles: {
      City: "Primäre Truppenkapazität, Bauradius (+2 Felder/Stufe), Sofortverstärkung und Stufenlimit für Bezirke.",
      Farm: "Ausgewogene Truppenproduktion, leichtes Goldeinkommen und Unterkünfte im Stadtradius.",
      TradingPost: "Maximales Goldeinkommen und Straßennetz; entwickelt sich zu Schmiede (N3), Fabrik (N5) und Eisenbahn (N6+).",
      Port: "Kriegsschiffe, Flottenkapazität, Reparatur, globales Seetempo und hoher Karawanen-Bonus.",
      Tower: "Gebietsverteidigung und Eroberungsschutz; aktiviert ab N5+ SAM-Raketenabwehr mit gleichem Radius.",
      MilitaryCamp: "Maximale Truppenproduktion, Unterkünfte, Eroberungstempo und Atomwaffen (Atom N5, Wasserstoff N6, MIRV N7).",
      SAM: "Automatisches Abfangen eingehender Atomwaffen.",
      MissileSilo: "Startkapazität für Atom-, Wasserstoff- und MIRV-Waffen.",
    },
    eras: ["Stammeszeit", "Agrarzeit", "Antike", "Mittelalter", "Frühe Neuzeit", "Industriezeitalter", "Gegenwart", "Zukunft"],
    stats: {
      reserve: "max. Truppen", troops: "Truppen/s", gold: "Gold/s", radius: "Bauradius", district_limit: "Bezirke/Typ",
      reinforcements: "Verstärkung", capture: "Eroberungstempo", trade: "Handel Gold/s", travel: "Routentempo",
      range: "Reichweite", resistance: "Eroberungsschutz", defense: "Schadensreduktion", fleet: "Flottenplätze",
      docks: "Boote zugleich", repair: "Reparatur/s", speed: "Seegeschwindigkeit", charges: "Salven", fire_rate: "Abfang/min", build_time: "Bauzeit",
    },
    intro: "Gründe Städte als Kernanker und errichte bis zu 5 spezialisierte Bezirke innerhalb ihres wachsenden Radius an einem verbundenen Straßen- und Schienennetz.",
    note: "Preise und Bauzeiten gelten für ein Gebäude bei Normaltempo inklusive Gesamtkosten und Stufenzuwachs.",
    level: "Stufe", era: "Zeitalter", build: "Bau", upgrade: "Ausbau", invested: "Gesamt", thisLevel: "diese Stufe", sameGold: "Gleiches Budget", immediate: "Soforttruppen",
    route: "Verbundene Landrouten ab N1", railway: "Eisenbahn ab N6", industrial: "Fabrik (N5) · SAM (N5) · Nuklear (N5–N7)", nav: "Gebäudelinien",
    codex: makeCodex({
      eyebrow: "Strategischer Kodex", allLines: "Alle Gebäude", coreAnchor: "Stadtzentrum", cityDistrict: "Stadtbezirk", coastalOnly: "Nur Küste",
      cartStopBonus: "Karawanen-Stopp-Bonus", foundationsHeading: "Stadt- & Bezirksregeln", tilesUnit: "Felder", secondsUnit: "s", perKindUnit: "/ Typ",
      factoryUnlock: "Fabrik-Ära", samUnlock: "SAM-Abwehr", districtCapUnlock: "{n}× Bezirke / Typ",
    }),
  },
  fil: {
    roles: {
      City: "Pangunahing tirahan ng tropa, lawak ng tayuan (+2 tile/ant.), agarang dagdag-tropa, at hangganan ng antas ng distrito.",
      Farm: "Balanseng produksyon ng tropa, banayad na ginto, at pabahay sa loob ng sakop ng lungsod.",
      TradingPost: "Pinakamataas na kita sa ginto at konektadong ruta; nagiging Pandayan (N3), Pabrika (N5), at Riles (N6+).",
      Port: "Bapor pandigma, puwesto ng plota, pagkumpuni, pandaigdigang bilis sa dagat, at mataas na bonus sa kariton.",
      Tower: "Depensa ng lugar at panlaban sa pagsakop; nagbubukas ng SAM anti-missile sa N5+ sa parehong saklaw.",
      MilitaryCamp: "Pinakamataas na produksyon ng tropa, bilis ng pagsakop, at paglulunsad ng nuklear (Atom N5, Hydrogen N6, MIRV N7).",
      SAM: "Awtomatikong pagsabat sa paparating na sandatang nuklear.",
      MissileSilo: "Kakayahang maglunsad ng atomic, hydrogen at MIRV.",
    },
    eras: ["Pantribo", "Pansakahan", "Klasiko", "Gitnang Panahon", "Maagang Makabago", "Industriyal", "Kasalukuyan", "Hinaharap"],
    stats: {
      reserve: "max na tropa", troops: "tropa/s", gold: "ginto/s", radius: "saklaw ng tayo", district_limit: "distrito/uri",
      reinforcements: "dagdag-tropa", capture: "bilis ng sakop", trade: "ginto sa kalakalan/s", travel: "bilis ng ruta",
      range: "saklaw ng depensa", resistance: "panlaban sa sakop", defense: "bawas sa talo", fleet: "puwesto ng plota",
      docks: "sabayang bangka", repair: "kumpuni/s", speed: "bilis sa dagat", charges: "salba", fire_rate: "sabat/min", build_time: "oras",
    },
    intro: "Magtayo ng mga Lungsod sa mapa at punuin ang lumalawak nilang saklaw ng 5 Distrito na nakakonekta sa kalsada, riles, at kalakalan.",
    note: "Ipinapakita ang presyo bawat gusali sa Normal na bilis, tagal ng pagtatayo, kabuuang nagastos, at dagdag sa bawat antas.",
    level: "Antas", era: "Panahon", build: "Gawa", upgrade: "Upgrade", invested: "kabuuan", thisLevel: "sa antas na ito", sameGold: "Parehong badyet", immediate: "Agarang tropa",
    route: "Konektadong rutang lupa mula N1", railway: "Riles ng tren sa N6", industrial: "Pabrika (N5) · SAM (N5) · Nuklear (N5–N7)", nav: "Mga linya ng gusali",
    codex: makeCodex({
      eyebrow: "Estratehikong Kodigo", allLines: "Lahat ng Gusali", coreAnchor: "Punong Lungsod", cityDistrict: "Distrito ng Lungsod", coastalOnly: "Pampang Lamang",
      cartStopBonus: "Bonus sa Hinto ng Kariton", foundationsHeading: "Pundasyon ng Lungsod at Distrito", tilesUnit: "tile", secondsUnit: "s", perKindUnit: "/ uri",
      factoryUnlock: "Panahon ng Pabrika", samUnlock: "Depensang SAM", districtCapUnlock: "{n}× Distrito / uri",
    }),
  },
  fr: {
    roles: {
      City: "Logement principal, rayon de construction (+2 cases/nv), renforts d’amélioration et plafond de niveau des quartiers.",
      Farm: "Production équilibrée de troupes, revenu d’or léger et logement au sein du territoire urbain.",
      TradingPost: "Revenu maximal en or et routes connectées ; évolue en Forge (N3), Usine (N5) et réseau ferré (N6+).",
      Port: "Navires de guerre, capacité de flotte, réparation, vitesse maritime globale et fort bonus de convoi.",
      Tower: "Défense de zone et résistance à la capture ; active l’interception antimissile SAM dès le N5+ sur le même rayon.",
      MilitaryCamp: "Production maximale de troupes, vitesse de conquête et frappes nucléaires (Atomique N5, Hydrogène N6, MIRV N7).",
      SAM: "Interception automatique des armes nucléaires entrantes.",
      MissileSilo: "Capacité de lancement atomique, hydrogène et MIRV.",
    },
    eras: ["Tribal", "Agraire", "Classique", "Médiévale", "Moderne ancienne", "Industrielle", "Contemporaine", "Futuriste"],
    stats: {
      reserve: "troupes max", troops: "troupes/s", gold: "or/s", radius: "rayon d’urbanisme", district_limit: "quartiers/type",
      reinforcements: "renforts", capture: "vitesse conquête", trade: "or commerce/s", travel: "vitesse route",
      range: "portée", resistance: "résist. capture", defense: "réduct. pertes", fleet: "places flotte",
      docks: "navires simult.", repair: "réparation/s", speed: "vitesse navale", charges: "salves", fire_rate: "interceptions/min", build_time: "durée",
    },
    intro: "Fondez des Villes sur la carte et spécialisez leur rayon croissant avec 5 Quartiers reliés au réseau routier, ferroviaire et commercial.",
    note: "Les coûts, durées et rendements s’entendent pour un bâtiment à vitesse normale, avec total investi et gain net par palier.",
    level: "Niveau", era: "Ère", build: "Construire", upgrade: "Améliorer", invested: "investi", thisLevel: "ce niveau", sameGold: "Même budget", immediate: "Renforts directs",
    route: "Routes terrestres connectées dès N1", railway: "Voies ferrées au N6", industrial: "Usine (N5) · SAM (N5) · Nucléaire (N5–N7)", nav: "Lignes de bâtiments",
    codex: makeCodex({
      eyebrow: "Codex stratégique", allLines: "Tout afficher", coreAnchor: "Noyau urbain", cityDistrict: "Quartier urbain", coastalOnly: "Côte uniquement",
      cartStopBonus: "Bonus d’arrêt chariot", foundationsHeading: "Fondations Villes & Quartiers", tilesUnit: "cases", secondsUnit: "s", perKindUnit: "/ type",
      factoryUnlock: "Ère Usine", samUnlock: "Antimissile SAM", districtCapUnlock: "{n}× Quartiers / type",
    }),
  },
  id: {
    roles: {
      City: "Kapasitas pasukan utama, radius bangun (+2 petak/lv), bala bantuan instan, dan batas level distrik.",
      Farm: "Keseimbangan produksi pasukan, emas ringan, dan hunian di dalam wilayah kota.",
      TradingPost: "Pendapatan emas maksimal dan rute darat; berevolusi menjadi Pandai Besi (N3), Pabrik (N5), dan Rel Kereta (N6+).",
      Port: "Kapal perang, kapasitas armada, perbaikan, kecepatan maritim global, dan bonus gerobak tinggi.",
      Tower: "Pertahanan area dan ketahanan perebutan; mengaktifkan intersepsi anti-rudal SAM mulai N5+ pada radius yang sama.",
      MilitaryCamp: "Produksi pasukan puncak, kecepatan perebutan darat, dan peluncuran nuklir (Atom N5, Hidrogen N6, MIRV N7).",
      SAM: "Pencegatan otomatis senjata nuklir yang datang.",
      MissileSilo: "Kapasitas peluncuran atom, hidrogen, dan MIRV.",
    },
    eras: ["Kesukuan", "Agraris", "Klasik", "Abad Pertengahan", "Modern Awal", "Industri", "Kontemporer", "Masa Depan"],
    stats: {
      reserve: "maks. pasukan", troops: "pasukan/dtk", gold: "emas/dtk", radius: "radius bangun", district_limit: "distrik/jenis",
      reinforcements: "bala bantuan", capture: "cepat merebut", trade: "emas dagang/dtk", travel: "laju rute",
      range: "jarak jangkau", resistance: "tahan rebut", defense: "kurangi korban", fleet: "slot armada",
      docks: "perahu serentak", repair: "perbaikan/dtk", speed: "cepat laut", charges: "muatan", fire_rate: "intersep/mnt", build_time: "durasi",
    },
    intro: "Bangun Kota sebagai pusat wilayah dan lengkapi radiusnya dengan 5 Distrik yang terhubung ke jaringan jalan, rel, dan perdagangan.",
    note: "Nilai berlaku untuk satu bangunan pada kecepatan Normal beserta durasi, total biaya, dan tambahan tiap tingkat.",
    level: "Level", era: "Era", build: "Bangun", upgrade: "Upgrade", invested: "total", thisLevel: "level ini", sameGold: "Anggaran sama", immediate: "Pasukan instan",
    route: "Rute darat terhubung sejak N1", railway: "Rel kereta terbuka di N6", industrial: "Pabrik (N5) · SAM (N5) · Nuklir (N5–N7)", nav: "Jalur bangunan",
    codex: makeCodex({
      eyebrow: "Kodeks Strategi", allLines: "Semua Struktur", coreAnchor: "Jangkar Utama", cityDistrict: "Distrik Kota", coastalOnly: "Khusus Pesisir",
      cartStopBonus: "Bonus Pemberhentian", foundationsHeading: "Fondasi Kota & Distrik", tilesUnit: "petak", secondsUnit: "dtk", perKindUnit: "/ jenis",
      factoryUnlock: "Era Pabrik", samUnlock: "Anti-Rudal SAM", districtCapUnlock: "{n}× Distrik / jenis",
    }),
  },
  it: {
    roles: {
      City: "Alloggiamento truppe principale, raggio di costruzione (+2 caselle/lv), rinforzi al potenziamento e limite livello dei distretti.",
      Farm: "Produzione bilanciata di truppe, piccolo reddito d’oro e alloggi nel territorio cittadino.",
      TradingPost: "Massimo introito d’oro e rotte connesse; evolve in Fucina (N3), Fabbrica (N5) e rete ferroviaria (N6+).",
      Port: "Navi da guerra, capacità flotta, riparazioni, velocità navale globale e alto bonus ai carri.",
      Tower: "Difesa d’area e resistenza alla conquista; attiva intercettazione antimissile SAM da N5+ sullo stesso raggio.",
      MilitaryCamp: "Produzione truppe massima, velocità di conquista e lanci nucleari (Atomica N5, Idrogeno N6, MIRV N7).",
      SAM: "Intercettazione automatica delle armi nucleari in arrivo.",
      MissileSilo: "Capacità di lancio per armi atomiche, a idrogeno e MIRV.",
    },
    eras: ["Tribale", "Agraria", "Classica", "Medievale", "Prima età moderna", "Industriale", "Contemporanea", "Futura"],
    stats: {
      reserve: "truppe max", troops: "truppe/s", gold: "oro/s", radius: "raggio urbano", district_limit: "distretti/tipo",
      reinforcements: "rinforzi", capture: "vel. conquista", trade: "oro rotta/s", travel: "vel. carri",
      range: "raggio difesa", resistance: "resist. conquista", defense: "riduz. perdite", fleet: "posti flotta",
      docks: "navi simult.", repair: "riparaz./s", speed: "vel. navale", charges: "salve", fire_rate: "intercetti/min", build_time: "durata",
    },
    intro: "Fonda Città sulla mappa per controllare il territorio e specializza il loro raggio con 5 Distretti collegati alla rete stradale e ferroviaria.",
    note: "I valori indicano costi e tempi per singolo edificio a velocità Normale, investimento totale e incremento per livello.",
    level: "Livello", era: "Era", build: "Costruisci", upgrade: "Migliora", invested: "investito", thisLevel: "questo livello", sameGold: "Stesso oro", immediate: "Truppe immediate",
    route: "Rotte terrestri connesse da N1", railway: "Ferrovie al livello N6", industrial: "Fabbrica (N5) · SAM (N5) · Nucleari (N5–N7)", nav: "Linee di edifici",
    codex: makeCodex({
      eyebrow: "Codice Strategico", allLines: "Tutte le strutture", coreAnchor: "Nucleo centrale", cityDistrict: "Distretto urbano", coastalOnly: "Solo costa",
      cartStopBonus: "Bonus sosta carro", foundationsHeading: "Fondamenta Città e Distretti", tilesUnit: "caselle", secondsUnit: "s", perKindUnit: "/ tipo",
      factoryUnlock: "Era Fabbrica", samUnlock: "Antimissile SAM", districtCapUnlock: "{n}× Distretti / tipo",
    }),
  },
  ja: {
    roles: {
      City: "主力兵力収容、建設半径(+2マス/Lv)、強化時の即時増援、そして各地区の最大レベル上限を決定します。",
      Farm: "兵力生産・少量の金収入・収容枠のバランスが取れた都市内の農業地区です。",
      TradingPost: "最大の金収入と接続交易網を提供し、Lv3で鍛冶場、Lv5で工場、Lv6+で鉄道拠点へ進化します。",
      Port: "軍艦建造、艦隊枠、修理、全海上ユニットの速度強化、そして高い荷馬車ボーナスを担います。",
      Tower: "範囲防御と占領耐性を提供し、Lv5以降は同じ射程でSAM迎撃（核ミサイル防御）を自動展開します。",
      MilitaryCamp: "最大の兵力生産、制圧速度強化に加え、Lv5以降に核攻撃（原子N5・水爆N6・MIRV N7）を解放します。",
      SAM: "飛来する核兵器を自動迎撃。",
      MissileSilo: "原子・水素・MIRV兵器の発射能力。",
    },
    eras: ["部族", "農耕", "古典", "中世", "近世", "産業", "現代", "未来"],
    stats: {
      reserve: "最大兵力", troops: "兵/秒", gold: "金/秒", radius: "建設半径", district_limit: "地区上限/種",
      reinforcements: "即時増援", capture: "占領速度", trade: "交易金/秒", travel: "輸送速度",
      range: "防衛射程", resistance: "占領耐性", defense: "損失軽減", fleet: "艦隊枠",
      docks: "同時出港", repair: "修理/秒", speed: "海上速度", charges: "弾数", fire_rate: "迎撃/分", build_time: "工期",
    },
    intro: "都市を建設して領土の核とし、拡大する都市圏内に道路・鉄道で繋がる5種類の地区を配置して経済と戦力を築きます。",
    note: "通常速度における建物1棟あたりの費用、工期、累計投資額、および各レベルでの純増分を表示しています。",
    level: "レベル", era: "時代", build: "建設", upgrade: "強化", invested: "累計", thisLevel: "増加分", sameGold: "同予算", immediate: "即時増援",
    route: "Lv1から道路網に接続", railway: "Lv6で鉄道を解放", industrial: "工場(N5)・SAM(N5)・核兵器(N5–N7)", nav: "建物ライン",
    codex: makeCodex({
      eyebrow: "戦略コーデックス", allLines: "すべて表示", coreAnchor: "中核都市", cityDistrict: "都市地区", coastalOnly: "沿岸専用",
      cartStopBonus: "荷馬車停車ボーナス", foundationsHeading: "都市と地区の基本ルール", tilesUnit: "マス", secondsUnit: "秒", perKindUnit: "/ 種",
      factoryUnlock: "工場時代", samUnlock: "SAM対ミサイル", districtCapUnlock: "{n}× 地区 / 種",
    }),
  },
  ko: {
    roles: {
      City: "핵심 병력 수용, 건설 반경(+2타일/Lv), 업그레이드 즉시 증원, 지구 최대 레벨 상한을 담당합니다.",
      Farm: "병력 생산, 소량의 골드 수입, 병력 수용이 균형 잡힌 도시 내 농업 지구입니다.",
      TradingPost: "최대 골드 수입과 연결 교역망을 제공하며 3레벨 대장간, 5레벨 공장, 6레벨 철도 허브로 진화합니다.",
      Port: "전함 건조, 함대 슬롯, 수리, 전체 해상 속도 향상 및 높은 수레 정차 보너스를 제공합니다.",
      Tower: "범위 방어 및 점령 저항을 제공하며 5레벨부터 동일 반경의 SAM 미사일 방어 역할을 겸합니다.",
      MilitaryCamp: "최대 병력 생산, 점령 속도 보너스 및 5레벨부터 핵무기 발사(원자 N5·수소 N6·MIRV N7)를 해금합니다.",
      SAM: "날아오는 핵무기를 자동 요격.",
      MissileSilo: "원자·수소·MIRV 무기 발사 능력.",
    },
    eras: ["부족", "농경", "고전", "중세", "근세", "산업", "현대", "미래"],
    stats: {
      reserve: "최대 병력", troops: "병력/초", gold: "골드/초", radius: "건설 반경", district_limit: "지구 한도/종",
      reinforcements: "즉시 증원", capture: "점령 속도", trade: "교역 골드/초", travel: "경로 속도",
      range: "방어 사거리", resistance: "점령 저항", defense: "피해 감소", fleet: "함대 슬롯",
      docks: "동시 출항", repair: "수리/초", speed: "해상 속도", charges: "장탄수", fire_rate: "요격/분", build_time: "시간",
    },
    intro: "지도에 도시를 세워 영토를 확보하고, 확장되는 반경 안에 도로와 철도로 연결된 5개 지구를 전략적으로 배치하세요.",
    note: "일반 속도 기준 건물 1개의 건설/업그레이드 비용, 소요 시간, 누적 투자액 및 레벨별 증가량을 표시합니다.",
    level: "레벨", era: "시대", build: "건설", upgrade: "업그레이드", invested: "누적", thisLevel: "이번 레벨", sameGold: "동일 예산", immediate: "즉시 증원",
    route: "1레벨부터 도로망 연결", railway: "6레벨 철도 해금", industrial: "공장(N5) · SAM(N5) · 핵무기(N5–N7)", nav: "건물 계열",
    codex: makeCodex({
      eyebrow: "전략 코덱스", allLines: "전체 보기", coreAnchor: "핵심 도시", cityDistrict: "도시 지구", coastalOnly: "해안 전용",
      cartStopBonus: "수레 정차 보너스", foundationsHeading: "도시 및 지구 기본 규칙", tilesUnit: "타일", secondsUnit: "초", perKindUnit: "/ 종",
      factoryUnlock: "공장 시대", samUnlock: "SAM 요격", districtCapUnlock: "{n}× 지구 / 종",
    }),
  },
  "pt-br": {
    roles: {
      City: "Alojamento principal de tropas, raio de construção (+2 casas/nv), reforços ao melhorar e limite de nível dos distritos.",
      Farm: "Produção equilibrada de tropas, ouro moderado e alojamento dentro do território urbano.",
      TradingPost: "Renda máxima de ouro e rotas conectadas; evolui para Ferraria (N3), Fábrica (N5) e Ferrovias (N6+).",
      Port: "Navios de guerra, capacidade de frota, reparo, velocidade marítima global e alto bônus para carroças.",
      Tower: "Defesa de área e resistência à conquista; ativa interceptação antimíssil SAM no N5+ com o mesmo raio.",
      MilitaryCamp: "Produção máxima de tropas, velocidade de conquista e lançamentos nucleares (Atômica N5, Hidrogênio N6, MIRV N7).",
      SAM: "Interceptação automática de armas nucleares que chegam.",
      MissileSilo: "Capacidade para lançar armas atômicas, de hidrogênio e MIRV.",
    },
    eras: ["Tribal", "Agrária", "Clássica", "Medieval", "Moderna inicial", "Industrial", "Contemporânea", "Futura"],
    stats: {
      reserve: "máx. tropas", troops: "tropas/s", gold: "ouro/s", radius: "raio urbano", district_limit: "distritos/tipo",
      reinforcements: "reforços", capture: "vel. conquista", trade: "ouro rota/s", travel: "vel. da rota",
      range: "alcance", resistance: "resist. conquista", defense: "redução de baixas", fleet: "vagas frota",
      docks: "barcos simult.", repair: "reparo/s", speed: "vel. marítima", charges: "salvas", fire_rate: "intercept./min", build_time: "tempo",
    },
    intro: "Funde Cidades no mapa e desenvolva seu raio crescente com 5 Distritos integrados à rede de estradas, trilhos e comércio.",
    note: "Valores por estrutura na velocidade Normal: custo, tempo de obra, total investido e ganho adicional de cada nível.",
    level: "Nível", era: "Era", build: "Construir", upgrade: "Melhorar", invested: "investido", thisLevel: "este nível", sameGold: "Mesmo ouro", immediate: "Reforço imediato",
    route: "Rotas terrestres conectadas no N1", railway: "Ferrovias no N6", industrial: "Fábrica (N5) · SAM (N5) · Nucleares (N5–N7)", nav: "Linhas de edifícios",
    codex: makeCodex({
      eyebrow: "Códice Estratégico", allLines: "Todos", coreAnchor: "Núcleo Central", cityDistrict: "Distrito Urbano", coastalOnly: "Somente Costa",
      cartStopBonus: "Bônus de Parada", foundationsHeading: "Fundamentos de Cidades e Distritos", tilesUnit: "casas", secondsUnit: "s", perKindUnit: "/ tipo",
      factoryUnlock: "Era da Fábrica", samUnlock: "SAM Antimíssil", districtCapUnlock: "{n}× Distritos / tipo",
    }),
  },
  ru: {
    roles: {
      City: "Основная вместимость войск, радиус застройки (+2 клетки/ур.), подкрепления при улучшении и потолок уровня районов.",
      Farm: "Баланс производства войск, лёгкого дохода золота и жилья в черте города.",
      TradingPost: "Максимальный доход золота и сеть дорог; развивается в Кузницу (ур.3), Завод (ур.5) и ж/д узел (ур.6+).",
      Port: "Боевые корабли, слоты флота, ремонт, общая скорость флота и высокий бонус торговым повозкам.",
      Tower: "Оборона зоны и защита от захвата; с 5-го уровня активирует ПВО SAM на том же радиусе действия.",
      MilitaryCamp: "Максимум производства войск, скорость захвата и ядерные удары (Атомная N5, Водородная N6, MIRV N7).",
      SAM: "Автоматический перехват входящего ядерного оружия.",
      MissileSilo: "Запуск атомных, водородных ракет и MIRV.",
    },
    eras: ["Племенная", "Аграрная", "Классическая", "Средневековая", "Раннее Новое время", "Индустриальная", "Современная", "Будущая"],
    stats: {
      reserve: "макс. войск", troops: "войск/с", gold: "золота/с", radius: "радиус города", district_limit: "районов/вид",
      reinforcements: "подкрепление", capture: "скор. захвата", trade: "золото пути/с", travel: "скор. маршрута",
      range: "дальность", resistance: "сопротивл.", defense: "сниж. потерь", fleet: "слоты флота",
      docks: "судов разом", repair: "ремонт/с", speed: "мор. скорость", charges: "залпы", fire_rate: "перехв./мин", build_time: "время",
    },
    intro: "Основывайте Города для контроля карты и застраивайте их растущий радиус 5 районами, объединёнными в сеть дорог, ж/д и торговли.",
    note: "Показаны цены для одного здания при обычной скорости, время постройки, сумма вложений и чистый прирост каждого уровня.",
    level: "Уровень", era: "Эпоха", build: "Построить", upgrade: "Улучшить", invested: "всего", thisLevel: "этот ур.", sameGold: "Тот же бюджет", immediate: "Войска сразу",
    route: "Сухопутная сеть с 1-го ур.", railway: "Железные дороги на 6-м ур.", industrial: "Завод (N5) · SAM (N5) · Ядерное (N5–N7)", nav: "Линии зданий",
    codex: makeCodex({
      eyebrow: "Стратегический кодекс", allLines: "Все постройки", coreAnchor: "Центр города", cityDistrict: "Городской район", coastalOnly: "Только берег",
      cartStopBonus: "Бонус остановки", foundationsHeading: "Основы городов и районов", tilesUnit: "клеток", secondsUnit: "с", perKindUnit: "/ вид",
      factoryUnlock: "Эпоха завода", samUnlock: "ПВО SAM", districtCapUnlock: "{n}× районов / вид",
    }),
  },
  tr: {
    roles: {
      City: "Ana asker kapasitesi, inşaat yarıçapı (+2 kare/sv.), yükseltme takviyesi ve bölge seviye sınırı.",
      Farm: "Dengeli asker üretimi, hafif altın geliri ve şehir sınırları içinde barınma.",
      TradingPost: "Azami altın geliri ve bağlı yollar; N3 Demirci, N5 Fabrika ve N6+ Demiryoluna evrilir.",
      Port: "Savaş gemileri, filo yuvası, onarım, küresel deniz hızı ve yüksek kervan durağı bonusu.",
      Tower: "Alan savunması ve ele geçirilmeye direnç; N5+ seviyesinden itibaren aynı menzille SAM hava savunması sağlar.",
      MilitaryCamp: "En yüksek asker üretimi, fetih hızı ve N5+ nükleer fırlatma (Atom N5, Hidrojen N6, MIRV N7).",
      SAM: "Gelen nükleer silahları otomatik engeller.",
      MissileSilo: "Atom, hidrojen ve MIRV silahlarını fırlatma kapasitesi.",
    },
    eras: ["Kabile", "Tarım", "Klasik", "Orta Çağ", "Erken Modern", "Sanayi", "Çağdaş", "Gelecek"],
    stats: {
      reserve: "maks. asker", troops: "asker/sn", gold: "altın/sn", radius: "şehir çapı", district_limit: "bölge/tür",
      reinforcements: "takviye", capture: "fetih hızı", trade: "ticaret altın/sn", travel: "rota hızı",
      range: "menzil", resistance: "ele geçir. direnci", defense: "kayıp azaltma", fleet: "filo yuvası",
      docks: "eşzamanlı gemi", repair: "onarım/sn", speed: "deniz hızı", charges: "salvo", fire_rate: "önleme/dk", build_time: "süre",
    },
    intro: "Haritada Şehirler kurun ve genişleyen menzillerini yollar, demiryolları ve ticarete bağlı 5 Bölge ile özelleştirin.",
    note: "Fiyatlar Normal hızda tek yapı içindir; yapım süresi, toplam yatırım ve her kademenin net artışını gösterir.",
    level: "Seviye", era: "Çağ", build: "İnşa Et", upgrade: "Yükselt", invested: "toplam", thisLevel: "bu seviye", sameGold: "Aynı bütçe", immediate: "Anında asker",
    route: "N1’den bağlı kara rotaları", railway: "N6’da demiryolu açılır", industrial: "Fabrika (N5) · SAM (N5) · Nükleer (N5–N7)", nav: "Bina hatları",
    codex: makeCodex({
      eyebrow: "Strateji Kodeksi", allLines: "Tüm Yapılar", coreAnchor: "Ana Merkez", cityDistrict: "Şehir Bölgesi", coastalOnly: "Sadece Kıyı",
      cartStopBonus: "Araba Durak Bonusu", foundationsHeading: "Şehir ve Bölge Temelleri", tilesUnit: "kare", secondsUnit: "sn", perKindUnit: "/ tür",
      factoryUnlock: "Fabrika Çağı", samUnlock: "SAM Füze Kalkanı", districtCapUnlock: "{n}× Bölge / tür",
    }),
  },
  vi: {
    roles: {
      City: "Sức chứa quân chính, bán kính xây dựng (+2 ô/cấp), quân tiếp viện khi nâng cấp và trần cấp độ khu vực.",
      Farm: "Cân bằng giữa sản xuất quân, ít vàng mỗi giây và chỗ ở trong phạm vi thành phố.",
      TradingPost: "Tối đa vàng/giây và mạng đường bộ kết nối; tiến hóa thành Lò rèn (N3), Nhà máy (N5) và Đường sắt (N6+).",
      Port: "Tàu chiến, chỗ hạm đội, sửa chữa, tăng tốc hàng hải toàn quân và thưởng trạm xe hàng cao.",
      Tower: "Phòng thủ khu vực và kháng chiếm đóng; kích hoạt đánh chặn SAM từ N5+ trên cùng bán kính.",
      MilitaryCamp: "Sản xuất quân tối đa, tốc độ chiếm đất và phóng hạt nhân (Nguyên tử N5, Hydro N6, MIRV N7).",
      SAM: "Tự động đánh chặn vũ khí hạt nhân đang tới.",
      MissileSilo: "Khả năng phóng vũ khí nguyên tử, hydro và MIRV.",
    },
    eras: ["Bộ lạc", "Nông nghiệp", "Cổ điển", "Trung cổ", "Cận đại", "Công nghiệp", "Đương đại", "Tương lai"],
    stats: {
      reserve: "quân tối đa", troops: "quân/giây", gold: "vàng/giây", radius: "bán kính xây", district_limit: "khu/loại",
      reinforcements: "tiếp viện", capture: "tốc độ chiếm", trade: "vàng tuyến/giây", travel: "tốc độ tuyến",
      range: "tầm phòng thủ", resistance: "kháng chiếm", defense: "giảm tổn thất", fleet: "chỗ hạm đội",
      docks: "tàu cùng lúc", repair: "sửa/giây", speed: "tốc độ biển", charges: "loạt đạn", fire_rate: "chặn/phút", build_time: "thời gian",
    },
    intro: "Lập Thành phố trên bản đồ rồi phát triển 5 Khu vực trong bán kính mở rộng, liên kết chặt vào mạng lưới đường bộ, đường sắt và thương mại.",
    note: "Hiển thị chi phí, thời gian xây, tổng vốn đầu tư ở tốc độ Normal và phần cộng thêm ở mỗi cấp.",
    level: "Cấp", era: "Thời đại", build: "Xây", upgrade: "Nâng cấp", invested: "tổng", thisLevel: "cấp này", sameGold: "Cùng ngân sách", immediate: "Quân tức thì",
    route: "Kết nối đường bộ từ N1", railway: "Mở đường sắt ở N6", industrial: "Nhà máy (N5) · SAM (N5) · Hạt nhân (N5–N7)", nav: "Dòng công trình",
    codex: makeCodex({
      eyebrow: "Cẩm nang chiến thuật", allLines: "Tất cả công trình", coreAnchor: "Đô thị trung tâm", cityDistrict: "Khu đô thị", coastalOnly: "Chỉ ven biển",
      cartStopBonus: "Thưởng điểm dừng", foundationsHeading: "Nền tảng Thành phố & Khu vực", tilesUnit: "ô", secondsUnit: "giây", perKindUnit: "/ loại",
      factoryUnlock: "Kỷ nguyên Nhà máy", samUnlock: "Đánh chặn SAM", districtCapUnlock: "{n}× Khu vực / loại",
    }),
  },
  "zh-cn": {
    roles: {
      City: "核心兵力容量、建造半径（每级+2格）、升级即时增援以及所有城区的最高等级上限。",
      Farm: "兼具稳健造兵、少量金币收入与兵力容纳的城市农业城区。",
      TradingPost: "最高金币产出与连通商路；分别在3级演变为铁匠铺、5级演变为工厂、6级解锁铁路枢纽。",
      Port: "建造战舰、增加船队与并发舰船上限、全军航海加速，并提供极高的商车停靠加成。",
      Tower: "区域防御与抗占领；从5级起共用相同防御半径兼作SAM防空反导。",
      MilitaryCamp: "峰值兵力产出与攻占速度加成，并从5级起解锁核打击（原子弹N5、氢弹N6、MIRV N7）。",
      SAM: "自动拦截来袭核武器。",
      MissileSilo: "发射原子弹、氢弹和MIRV的能力。",
    },
    eras: ["部落", "农耕", "古典", "中世纪", "近代早期", "工业", "当代", "未来"],
    stats: {
      reserve: "最大兵力", troops: "兵力/秒", gold: "金币/秒", radius: "建造半径", district_limit: "城区上限/类",
      reinforcements: "即时增援", capture: "占领速度", trade: "商路金币/秒", travel: "路线速度",
      range: "防御射程", resistance: "抗占领权重", defense: "减损防御", fleet: "舰队位",
      docks: "并发船只", repair: "维修/秒", speed: "航海速度", charges: "弹药", fire_rate: "拦截/分钟", build_time: "工期",
    },
    intro: "在地图上建立城市核心，并在其不断扩大的半径内自由布局5大城区，接入道路、铁路与贸易网络。",
    note: "显示普通速度下单座建筑的建造与升级花费、工期、累计投入及每级净增益。",
    level: "等级", era: "时代", build: "建造", upgrade: "升级", invested: "累计", thisLevel: "本级", sameGold: "同预算", immediate: "即时增援",
    route: "1级即连通陆路", railway: "6级解锁铁路", industrial: "工厂(N5) · SAM(N5) · 核武(N5–N7)", nav: "建筑路线",
    codex: makeCodex({
      eyebrow: "战略图鉴", allLines: "全部建筑", coreAnchor: "核心锚点", cityDistrict: "城市城区", coastalOnly: "仅限沿海",
      cartStopBonus: "商车停靠加成", foundationsHeading: "城市与城区机制基础", tilesUnit: "格", secondsUnit: "秒", perKindUnit: "/ 类",
      factoryUnlock: "工厂时代", samUnlock: "SAM反导", districtCapUnlock: "{n}× 城区 / 类",
    }),
  },
};

export const BUILDING_KINDS = Object.keys(lines);

function localeCode(locale: string): string {
  const normalized = locale.toLowerCase().replaceAll("_", "-");
  if (normalized === "pt" || normalized.startsWith("pt-")) return "pt-br";
  if (normalized === "tl" || normalized.startsWith("tl-")) return "fil";
  if (normalized === "zh" || normalized.startsWith("zh-")) return "zh-cn";
  return normalized.split("-")[0];
}

export function buildingLine(kind: string, locale: string) {
  const code = localeCode(locale);
  const names = localizedTiers[code]?.[kind as BuildingKind] ?? lines[kind]?.[code === "es" ? "es" : "en"];
  const localized = pageLocales[code] ?? pageLocales.en;
  return names?.map((name, index) => ({
    name,
    era: localized.eras[lines[kind].en.length === 4 ? Math.min(7, index + 5) : index],
  })) ?? [];
}

export function buildingRole(kind: string, locale: string) {
  const code = localeCode(locale);
  return (pageLocales[code] ?? pageLocales.en).roles[kind as BuildingKind] ?? "";
}

const goldUnits: Record<string, string> = {
  ar: "ذهب", de: "Gold", en: "gold", es: "oro", fil: "ginto", fr: "or", id: "emas", it: "oro",
  ja: "ゴールド", ko: "골드", "pt-br": "ouro", ru: "золота", tr: "altın", vi: "vàng", "zh-cn": "金币",
};

const extraCodexLocales: Record<string, Partial<CodexCopy>> = {
  ar: {
    placementCity: "عالمي · تباعد 2× النطاق",
    placementDistrict: "داخل المدينة · Nv ≤ المدينة · 1–3/نوع",
    placementCoastal: "ساحلي داخل المدينة · Nv ≤ المدينة · 1–3/نوع",
    sameGoldTooltip: "نفس ميزانية بناء {n} مبنى N1 جديد.",
    investedShort: "الإجمالي",
    maxLevel: "الحد N8",
    roleTagCoastal: "ساحلي",
    roleTagSam: "SAM N5+",
    roleTagNukes: "نووي N5–7",
    roleTagFactory: "مصنع N5+",
    foundations: {
      cities: { title: "المدن ونطاق السيطرة", badge: "{baseRadius}–{maxRadius} خانة", body: "أسس المدن بتباعد يتناسب مع 2× نطاق المدن القريبة ({minExclusion}–{maxExclusion} خانة). تضيف الترقية +{radiusStep} خانات للبناء وتعزيزات فورية." },
      districts: { title: "المقاطعات والحدود والاستيلاء", badge: "1 / 2 / 3 لكل نوع", body: "ابنِ 5 مقاطعات بحرية داخل نطاق مدينتك: مستوى المقاطعة ≤ المدينة، والحد يتوسع مع المدينة ({c1}، {c2}، {c3}). كل خانة تُحتل بشكل مستقل." },
      trade: { title: "شبكة القوافل والسكك الحديدية", badge: "مكافأة 25%–150%", body: "يرتبط كل مبنى بشبكة الطرق والسكك. تمنح القوافل ذهباً حسب مكافأة كل محطة، مع صدارة المراكز التجارية/المصانع (150%) والموانئ (125%)." },
      evolutions: { title: "قفزات العصور والأسلحة الخارقة", badge: "فتح N5–N7", body: "يتحول المركز التجاري إلى مصنع (N5) وسكك حديد (N6)، وتفعل الأبراج دفاع SAM بنفس المدى من N5، وتفتح المعسكرات القنابل الذرية والهيدروجينية وMIRV من N5–N7." },
    },
  },
  de: {
    placementCity: "Global · 2× Radius-Abstand",
    placementDistrict: "In Stadt · St. ≤ Stadt · 1–3/Typ",
    placementCoastal: "Küste in Stadt · St. ≤ Stadt · 1–3/Typ",
    sameGoldTooltip: "Entspricht den Baukosten von {n} neuen N1-Gebäuden.",
    investedShort: "Gesamt",
    maxLevel: "Max. N8",
    roleTagCoastal: "Küste",
    roleTagSam: "SAM N5+",
    roleTagNukes: "Nuklear N5–7",
    roleTagFactory: "Fabrik N5+",
    foundations: {
      cities: { title: "Städte & Bauradius", badge: "{baseRadius}–{maxRadius} Felder", body: "Gründe Städte mit proportionalem Abstand (2× Radius naher Städte: {minExclusion}–{maxExclusion} Felder). Jede Stufe gewährt +{radiusStep} Felder Bauradius, Truppenlimits und Sofortverstärkung." },
      districts: { title: "Bezirke, Limits & Eroberung", badge: "1 / 2 / 3 pro Typ", body: "Errichte 5 Bezirke frei im Stadtradius: Bezirk ≤ Stadtstufe, Limit pro Typ steigt mit der Stadt ({c1} bei N1–3, {c2} bei N4–7, {c3} bei N8). Eigene Felder werden separat erobert." },
      trade: { title: "Karawanen- & Schienennetz", badge: "25%–150% Stopp", body: "Jedes Gebäude verbindet sich automatisch mit Straßen und Gleisen. Karawanen zahlen Gold gemäß Stopp-Bonus — Handelsposten/Fabriken (150%) und Häfen (125%) führen." },
      evolutions: { title: "Epochen-Upgrades & Superwaffen", badge: "N5–N7 Freischaltung", body: "Handelsposten werden zu Fabriken (N5) und Eisenbahnen (N6); Türme aktivieren SAM-Abwehr ab N5 im selben Radius; Militärlager schalten Atom (N5), Wasserstoff (N6) und MIRV (N7) frei." },
    },
  },
  fil: {
    placementCity: "Global · 2× agwat ng saklaw",
    placementDistrict: "Sa Lungsod · Ant. ≤ Lungsod · 1–3/uri",
    placementCoastal: "Pampang sa Lungsod · Ant. ≤ Lungsod · 1–3/uri",
    sameGoldTooltip: "Katumbas ng pagtatayo ng {n} bagong gusaling N1.",
    investedShort: "Kabuuan",
    maxLevel: "Max N8",
    roleTagCoastal: "Pampang",
    roleTagSam: "SAM N5+",
    roleTagNukes: "Nuklear N5–7",
    roleTagFactory: "Pabrika N5+",
    foundations: {
      cities: { title: "Mga Lungsod at Saklaw", badge: "{baseRadius}–{maxRadius} tile", body: "Magtayo ng Lungsod na may agwat na 2× saklaw ng kalapit na Lungsod ({minExclusion}–{maxExclusion} tile). Bawat upgrade ay nagdadagdag ng +{radiusStep} tile, tropa, at agarang reinforcements." },
      districts: { title: "Mga Distrito at Limitasyon", badge: "1 / 2 / 3 bawat uri", body: "Malayang magtayo ng 5 Distrito sa sakop ng sariling Lungsod: Antas ng Distrito ≤ Lungsod, at limitasyon ay lumalaki ({c1} sa N1–3, {c2} sa N4–7, {c3} sa N8)." },
      trade: { title: "Konektadong Kalsada at Riles", badge: "25%–150% hinto", body: "Bawat gusali ay kumokonekta sa daan at riles. Ang mga kariton at tren ay nagbibigay ng ginto ayon sa Stop Bonus — Pabrika/Kalakalan (150%) at Daungan (125%)." },
      evolutions: { title: "Pagbabago ng Panahon", badge: "N5–N7 unlocks", body: "Nagiging Pabrika (N5) at Riles (N6) ang Trading Post; nagbubukas ng SAM anti-missile ang Tore sa N5+ sa parehong saklaw; nagbubukas ng Atom (N5), Hydrogen (N6), at MIRV (N7) ang Kampong Militar." },
    },
  },
  fr: {
    placementCity: "Global · écart 2× rayon",
    placementDistrict: "En Ville · Nv ≤ Ville · 1–3/type",
    placementCoastal: "Côte en Ville · Nv ≤ Ville · 1–3/type",
    sameGoldTooltip: "Même budget que construire {n} nouvelles structures N1.",
    investedShort: "Total",
    maxLevel: "Max N8",
    roleTagCoastal: "Côtier",
    roleTagSam: "SAM N5+",
    roleTagNukes: "Nucléaire N5–7",
    roleTagFactory: "Usine N5+",
    foundations: {
      cities: { title: "Villes & rayon d’action", badge: "{baseRadius}–{maxRadius} cases", body: "Fondez des Villes avec un espacement proportionnel (2× le rayon voisin : {minExclusion}–{maxExclusion} cases). Chaque niveau ajoute +{radiusStep} cases de rayon, capacité de troupes et renforts immédiats." },
      districts: { title: "Quartiers, plafonds & conquête", badge: "1 / 2 / 3 par type", body: "Construisez librement 5 Quartiers dans le rayon d’une Ville alliée : Niveau ≤ Ville, et plafond par type évolutif ({c1} en N1–3, {c2} en N4–7, {c3} en N8). Chaque case se capture indépendamment." },
      trade: { title: "Réseau connecté charrettes & rails", badge: "25%–150% arrêt", body: "Chaque structure se relie aux routes et voies ferrées. Les convois rapportent de l’or selon le bonus d’arrêt : Comptoir/Usine (150%) et Port (125%) dominent." },
      evolutions: { title: "Paliers d’ère & super-armes", badge: "Débloqués N5–N7", body: "Le Comptoir devient Usine (N5) puis Rail (N6), la Tour active l’antimissile SAM dès N5 avec son rayon, et le Camp Militaire débloque Atomique (N5), Hydrogène (N6) et MIRV (N7)." },
    },
  },
  id: {
    placementCity: "Global · jarak 2× radius",
    placementDistrict: "Dalam Kota · Lv ≤ Kota · 1–3/jenis",
    placementCoastal: "Pesisir Kota · Lv ≤ Kota · 1–3/jenis",
    sameGoldTooltip: "Setara biaya membangun {n} struktur N1 baru.",
    investedShort: "Total",
    maxLevel: "Maks N8",
    roleTagCoastal: "Pesisir",
    roleTagSam: "SAM N5+",
    roleTagNukes: "Nuklir N5–7",
    roleTagFactory: "Pabrik N5+",
    foundations: {
      cities: { title: "Kota & Radius Wilayah", badge: "{baseRadius}–{maxRadius} petak", body: "Dirikan Kota dengan jarak proporsional (2× radius Kota terdekat: {minExclusion}–{maxExclusion} petak). Tiap peningkatan menambah +{radiusStep} petak, batas pasukan, dan bantuan instan." },
      districts: { title: "Distrik, Batas & Perebutan", badge: "1 / 2 / 3 per jenis", body: "Bangun 5 Distrik bebas dalam radius Kota: Level Distrik ≤ Kota, dengan batas jenis naik ({c1} di N1–3, {c2} di N4–7, {c3} di N8). Setiap petak dapat direbut terpisah." },
      trade: { title: "Jaringan Gerobak & Rel", badge: "25%–150% henti", body: "Setiap bangunan otomatis terhubung ke jalan dan rel. Gerobak dan kereta memberi emas sesuai bonus pemberhentian: Pos Dagang/Pabrik (150%) dan Pelabuhan (125%)." },
      evolutions: { title: "Evolusi Era & Senjata Super", badge: "Buka di N5–N7", body: "Pos Dagang menjadi Pabrik (N5) dan Rel (N6); Menara mengaktifkan anti-rudal SAM mulai N5 pada radius sama; Kamp Militer membuka Atom (N5), Hidrogen (N6), dan MIRV (N7)." },
    },
  },
  it: {
    placementCity: "Globale · distanza 2× raggio",
    placementDistrict: "In Città · Lv ≤ Città · 1–3/tipo",
    placementCoastal: "Costa in Città · Lv ≤ Città · 1–3/tipo",
    sameGoldTooltip: "Pari al costo di costruzione di {n} nuove strutture N1.",
    investedShort: "Totale",
    maxLevel: "Max N8",
    roleTagCoastal: "Costiero",
    roleTagSam: "SAM N5+",
    roleTagNukes: "Nucleari N5–7",
    roleTagFactory: "Fabbrica N5+",
    foundations: {
      cities: { title: "Città e raggio di controllo", badge: "{baseRadius}–{maxRadius} caselle", body: "Fonda Città rispettando una distanza pari a 2× il raggio vicino ({minExclusion}–{maxExclusion} caselle). Ogni potenziamento aggiunge +{radiusStep} caselle di raggio, capacità truppe e rinforzi immediati." },
      districts: { title: "Distretti, limiti e conquista", badge: "1 / 2 / 3 per tipo", body: "Costruisci 5 Distretti liberamente nel raggio cittadino: Livello Distretto ≤ Città, limite per tipo crescente ({c1} a N1–3, {c2} a N4–7, {c3} a N8). Caselle conquistabili separatamente." },
      trade: { title: "Rete commerciale e ferroviaria", badge: "25%–150% sosta", body: "Ogni edificio si collega a strade e binari. Carri e treni assegnano oro in base al bonus di sosta: Avamposto/Fabbrica (150%) e Porto (125%) in testa." },
      evolutions: { title: "Svolte d’era e superarmi", badge: "Sblocchi N5–N7", body: "L’Avamposto evolve in Fabbrica (N5) e Ferrovie (N6); la Torre attiva la difesa SAM da N5 sullo stesso raggio; il Campo Militare sblocca Atomica (N5), Idrogeno (N6) e MIRV (N7)." },
    },
  },
  ja: {
    placementCity: "マップ全域 · 半径2×間隔",
    placementDistrict: "都市圏内 · Lv≤都市 · 各1–3",
    placementCoastal: "都市圏沿岸 · Lv≤都市 · 各1–3",
    sameGoldTooltip: "新規N1施設を{n}棟建てるのと同等の予算です。",
    investedShort: "累計",
    maxLevel: "最大N8",
    roleTagCoastal: "沿岸",
    roleTagSam: "SAM N5+",
    roleTagNukes: "核 N5–7",
    roleTagFactory: "工場 N5+",
    foundations: {
      cities: { title: "都市と建設半径", badge: "{baseRadius}–{maxRadius} マス", body: "近隣都市の建設半径の2倍（{minExclusion}–{maxExclusion}マス）を空けて都市を設立します。強化ごとに半径+{radiusStep}マス、最大兵力、即時増援が増加します。" },
      districts: { title: "地区・上限と占領", badge: "各種 1 / 2 / 3", body: "自都市の半径内に5種の地区を自由に建設できます（地区Lv ≤ 都市Lv、各種上限 {c1}→{c2}→{c3}）。都市と地区は別マスで個別に占領可能です。" },
      trade: { title: "道路・鉄道の接続交易網", badge: "停車 25%–150%", body: "新規建物は道路と鉄道に接続されます。通過する荷馬車・列車は目的地の停車ボーナス（交易所/工場150%、港125%など）に応じた金を運びます。" },
      evolutions: { title: "時代進化と戦略兵器", badge: "N5–N7 解放", body: "交易所はN5で工場・N6で鉄道へ進化し、防衛塔はN5から同射程のSAM迎撃を兼ね、軍事キャンプはN5原子・N6水爆・N7 MIRVを解放します。" },
    },
  },
  ko: {
    placementCity: "전역 · 반경 2× 거리",
    placementDistrict: "도시 내 · Lv≤도시 · 종당 1–3",
    placementCoastal: "도시 해안 · Lv≤도시 · 종당 1–3",
    sameGoldTooltip: "새 N1 건물 {n}개를 짓는 것과 같은 비용입니다.",
    investedShort: "누적",
    maxLevel: "최대 N8",
    roleTagCoastal: "해안",
    roleTagSam: "SAM N5+",
    roleTagNukes: "핵 N5–7",
    roleTagFactory: "공장 N5+",
    foundations: {
      cities: { title: "도시와 건설 반경", badge: "{baseRadius}–{maxRadius} 타일", body: "인접 도시 건설 반경의 2배({minExclusion}–{maxExclusion}타일)를 띄워 도시를 세웁니다. 업그레이드마다 건설 반경 +{radiusStep}타일, 병력 수용량, 즉시 증원이 증가합니다." },
      districts: { title: "지구 한도 및 점령 규칙", badge: "종류당 1 / 2 / 3", body: "소유 도시 반경 안에 5개 지구를 자유롭게 배치하세요(지구 Lv ≤ 도시 Lv, 한도 {c1}→{c2}→{c3}). 도시와 지구는 별도 타일이라 개별 점령됩니다." },
      trade: { title: "연결된 수레·철도 교역망", badge: "정차 25%–150%", body: "모든 새 건물은 도로와 철도에 자동 연결됩니다. 수레와 열차는 정차 보너스(교역소/공장 150%, 항구 125% 등)에 비례해 골드를 지급합니다." },
      evolutions: { title: "시대 전환과 전략 병기", badge: "N5–N7 해금", body: "교역소는 N5 공장 및 N6 철도로 진화하고, 방어탑은 N5부터 동일 반경 SAM 요격을 겸하며, 군사 캠프는 N5 원자·N6 수소·N7 MIRV를 해금합니다." },
    },
  },
  "pt-br": {
    placementCity: "Global · distância 2× raio",
    placementDistrict: "Na Cidade · Nv ≤ Cidade · 1–3/tipo",
    placementCoastal: "Costa na Cidade · Nv ≤ Cidade · 1–3/tipo",
    sameGoldTooltip: "Equivale ao custo de construir {n} novas estruturas N1.",
    investedShort: "Total",
    maxLevel: "Máx N8",
    roleTagCoastal: "Costeiro",
    roleTagSam: "SAM N5+",
    roleTagNukes: "Nucleares N5–7",
    roleTagFactory: "Fábrica N5+",
    foundations: {
      cities: { title: "Cidades e raio urbano", badge: "{baseRadius}–{maxRadius} casas", body: "Funde Cidades mantendo 2× o raio de Cidades vizinhas ({minExclusion}–{maxExclusion} casas). Cada melhoria adiciona +{radiusStep} casas ao raio, capacidade de tropas e reforços imediatos." },
      districts: { title: "Distritos, limites e captura", badge: "1 / 2 / 3 por tipo", body: "Construa os 5 Distritos livremente dentro do raio urbano: Nível do Distrito ≤ Cidade, e o limite por tipo sobe ({c1} em N1–3, {c2} em N4–7, {c3} em N8). Casas separadas são tomadas à parte." },
      trade: { title: "Rede de carroças e ferrovias", badge: "25%–150% parada", body: "Toda estrutura conecta-se às estradas e trilhos. Carroças e trens pagam ouro pelo Bônus de Parada de cada destino: Entreposto/Fábrica (150%) e Porto (125%) lideram." },
      evolutions: { title: "Saltos de era e superarmas", badge: "Desbloqueios N5–N7", body: "O Entreposto vira Fábrica (N5) e Ferrovia (N6); a Torre ativa interceptação SAM desde N5 com o mesmo alcance; o Acampamento libera Atômica (N5), Hidrogênio (N6) e MIRV (N7)." },
    },
  },
  ru: {
    placementCity: "Везде · шаг 2× радиуса",
    placementDistrict: "В городе · Ур. ≤ Города · 1–3/вид",
    placementCoastal: "Берег города · Ур. ≤ Города · 1–3/вид",
    sameGoldTooltip: "Равно цене постройки {n} новых зданий 1-го уровня.",
    investedShort: "Всего",
    maxLevel: "Макс. N8",
    roleTagCoastal: "Берег",
    roleTagSam: "SAM N5+",
    roleTagNukes: "Ядерное N5–7",
    roleTagFactory: "Завод N5+",
    foundations: {
      cities: { title: "Города и радиус застройки", badge: "{baseRadius}–{maxRadius} клеток", body: "Города строятся на расстоянии 2× от радиуса соседних городов ({minExclusion}–{maxExclusion} клеток). Улучшение даёт +{radiusStep} клетки радиуса, вместимость и мгновенные войска." },
      districts: { title: "Районы, лимиты и захват", badge: "1 / 2 / 3 на вид", body: "5 районов свободно строятся в радиусе своего города: ур. района ≤ ур. города, лимит на вид растёт с городом ({c1} на N1–3, {c2} на N4–7, {c3} на N8). Клетки захватываются отдельно." },
      trade: { title: "Сеть повозок и железных дорог", badge: "25%–150% бонус", body: "Каждая постройка подключается к дорогам и ж/д. Повозки и поезда приносят золото по множителю остановки: Торговый пост/Завод (150%) и Порт (125%) в лидерах." },
      evolutions: { title: "Эпохальные скачки и супероружие", badge: "Открытия N5–N7", body: "Торговый пост становится Заводом (N5) и ж/д (N6), Башня с N5 получает перехват SAM на том же радиусе, а Военный лагерь открывает Атомную (N5), Водородную (N6) и MIRV (N7)." },
    },
  },
  tr: {
    placementCity: "Küresel · 2× çap mesafesi",
    placementDistrict: "Şehir içi · Sv ≤ Şehir · 1–3/tür",
    placementCoastal: "Şehir kıyısı · Sv ≤ Şehir · 1–3/tür",
    sameGoldTooltip: "{n} yeni N1 bina inşa etme bütçesine eşittir.",
    investedShort: "Toplam",
    maxLevel: "Azami N8",
    roleTagCoastal: "Kıyı",
    roleTagSam: "SAM N5+",
    roleTagNukes: "Nükleer N5–7",
    roleTagFactory: "Fabrika N5+",
    foundations: {
      cities: { title: "Şehirler ve Kontrol Çapı", badge: "{baseRadius}–{maxRadius} kare", body: "Yakındaki Şehirlerin inşaat yarıçapının 2 katı ({minExclusion}–{maxExclusion} kare) aralıkla Şehir kurun. Her seviye +{radiusStep} kare menzil, asker kapasitesi ve anında takviye kazandırır." },
      districts: { title: "Bölgeler, Sınırlar ve Fetih", badge: "Tür başına 1 / 2 / 3", body: "5 Bölge kendi Şehrinin çapında serbestçe kurulur: Bölge Sv ≤ Şehir Sv ve tür sınırı şehirle artar ({c1} N1–3, {c2} N4–7, {c3} N8). Ayrı kareler bağımsız fethedilir." },
      trade: { title: "Bağlı Kervan ve Demiryolu Ağı", badge: "%25–%150 durak", body: "Her yeni yapı yollara ve raylara bağlanır. Kervan ve trenler durak bonusuna göre altın bırakır: Ticaret/Fabrika (%150) ve Liman (%125) zirvededir." },
      evolutions: { title: "Çağ Atlamaları ve Süper Silahlar", badge: "N5–N7 açılışları", body: "Ticaret Karakolu N5’te Fabrikaya ve N6’da Demiryoluna dönüşür; Kule N5’ten itibaren aynı menzilde SAM füze kalkanı kazanır; Askerî Kamp N5 Atom, N6 Hidrojen ve N7 MIRV açar." },
    },
  },
  vi: {
    placementCity: "Toàn bản đồ · cách 2× bán kính",
    placementDistrict: "Trong Thành phố · Cấp ≤ TP · 1–3/loại",
    placementCoastal: "Ven biển TP · Cấp ≤ TP · 1–3/loại",
    sameGoldTooltip: "Tương đương ngân sách xây {n} công trình N1 mới.",
    investedShort: "Tổng",
    maxLevel: "Tối đa N8",
    roleTagCoastal: "Ven biển",
    roleTagSam: "SAM N5+",
    roleTagNukes: "Hạt nhân N5–7",
    roleTagFactory: "Nhà máy N5+",
    foundations: {
      cities: { title: "Thành phố & Bán kính kiểm soát", badge: "{baseRadius}–{maxRadius} ô", body: "Lập Thành phố giữ khoảng cách 2× bán kính Thành phố gần nhất ({minExclusion}–{maxExclusion} ô). Mỗi cấp thêm +{radiusStep} ô bán kính, sức chứa quân và tiếp viện tức thì." },
      districts: { title: "Khu vực, Giới hạn & Chiếm đóng", badge: "1 / 2 / 3 mỗi loại", body: "Tự do đặt 5 Khu vực trong bán kính Thành phố: Cấp Khu ≤ Thành phố, giới hạn theo loại tăng ({c1} ở N1–3, {c2} ở N4–7, {c3} ở N8). Các ô được chiếm độc lập." },
      trade: { title: "Mạng xe hàng & đường sắt kết nối", badge: "Thưởng 25%–150%", body: "Mỗi công trình nối vào đường bộ và đường sắt. Xe hàng và tàu hỏa trả vàng theo Thưởng điểm dừng: Thương điếm/Nhà máy (150%) và Cảng (125%) dẫn đầu." },
      evolutions: { title: "Bước ngoặt thời đại & Siêu vũ khí", badge: "Mở khóa N5–N7", body: "Trạm giao thương nâng lên Nhà máy (N5) và Đường sắt (N6); Tháp kích hoạt SAM đánh chặn từ N5 cùng bán kính; Trại quân sự mở Nguyên tử (N5), Hydro (N6) và MIRV (N7)." },
    },
  },
  "zh-cn": {
    placementCity: "全局 · 保持2×半径间距",
    placementDistrict: "城市半径内 · 等级≤城市 · 每类1–3",
    placementCoastal: "城圈沿海 · 等级≤城市 · 每类1–3",
    sameGoldTooltip: "相当于建造 {n} 座全新 1 级建筑的预算。",
    investedShort: "总计",
    maxLevel: "满级 N8",
    roleTagCoastal: "沿海",
    roleTagSam: "SAM N5+",
    roleTagNukes: "核武 N5–7",
    roleTagFactory: "工厂 N5+",
    foundations: {
      cities: { title: "城市与建造半径", badge: "{baseRadius}–{maxRadius} 格", body: "建造城市需与邻近城市保持 2 倍建造半径的间距（{minExclusion}–{maxExclusion} 格）。每次升级增加 +{radiusStep} 格半径、兵力上限与即时增援。" },
      districts: { title: "城区上限与独立占领", badge: "每类 1 / 2 / 3", body: "5 大城区可自由建于己方城市半径内：城区等级 ≤ 城市等级，同类上限随城市升级解锁（{c1}→{c2}→{c3}）。城市与城区各自占格，互不绑定沦陷。" },
      trade: { title: "互联商路与铁路网络", badge: "25%–150% 停靠", body: "所有新建建筑都会接入道路与铁路网。经过的商车与列车按目的地停靠加成发放金币——贸易站/工厂（150%）与港口（125%）收益最高。" },
      evolutions: { title: "时代蜕变与终极武库", badge: "N5–N7 解锁", body: "贸易站在 5 级演变为工厂、6 级开通铁路；防御塔自 5 级起用同半径兼任 SAM 防空反导；军事营地在 5/6/7 级解锁原子弹、氢弹与 MIRV。" },
    },
  },
};

export function buildingPageText(locale: string): PageLocale & { gold: string } {
  const code = localeCode(locale);
  const entry = pageLocales[code] ?? pageLocales.en;
  const extra = extraCodexLocales[code];
  return {
    ...entry,
    codex: extra ? { ...entry.codex, ...extra } : entry.codex,
    gold: goldUnits[code] ?? goldUnits.en,
  };
}
