const lines: Record<string, { en: string[]; es: string[] }> = {
  City: {
    en: ["Settlement", "Village", "Town", "Walled City", "Capital", "Industrial City", "Metropolis", "Megacity"],
    es: ["Asentamiento", "Aldea", "Pueblo", "Ciudad amurallada", "Capital", "Ciudad industrial", "Metrópolis", "Megaciudad"],
  },
  MilitaryCamp: {
    en: ["Military Camp", "Barracks", "Garrison", "Stronghold", "Military Base", "Mobilization Center", "Command Center", "Strategic Command"],
    es: ["Campamento militar", "Cuartel", "Guarnición", "Bastión militar", "Base militar", "Centro de movilización", "Centro de mando", "Mando estratégico"],
  },
  TradingPost: {
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

type BuildingKind = keyof typeof lines;
const localizedTiers: Record<string, Partial<Record<BuildingKind, string[]>>> = {
  ar: {
    City: ["مستوطنة", "قرية", "بلدة", "مدينة مسوّرة", "عاصمة", "مدينة صناعية", "حاضرة", "مدينة كبرى"],
    MilitaryCamp: ["معسكر عسكري", "ثكنة", "حامية", "حصن", "قاعدة عسكرية", "مركز تعبئة", "مركز قيادة", "قيادة استراتيجية"],
    TradingPost: ["مركز تجاري", "سوق", "ساحة تجارية", "حي التجار", "مستودع تجاري", "محطة شحن", "مركز لوجستي", "مجمع تجاري"],
    Tower: ["برج مراقبة", "برج حراسة", "برج حجري", "قلعة", "حصن", "حصن مدفعي", "محطة دفاع", "برج الحارس"],
    Port: ["رصيف صيد", "رصيف تجاري", "ميناء", "ميناء محصن", "ترسانة بحرية", "حوض بناء سفن صناعي", "قاعدة الأسطول", "قيادة الأسطول"],
    SAM: ["بطارية SAM I", "بطارية SAM II", "بطارية SAM III", "بطارية SAM IV"],
    MissileSilo: ["صومعة صواريخ I", "صومعة صواريخ II", "صومعة صواريخ III", "صومعة صواريخ IV"],
  },
  de: {
    City: ["Siedlung", "Dorf", "Kleinstadt", "Mauerstadt", "Hauptstadt", "Industriestadt", "Metropole", "Megastadt"],
    MilitaryCamp: ["Militärlager", "Kaserne", "Garnison", "Festung", "Militärbasis", "Mobilisierungszentrum", "Kommandozentrum", "Strategisches Oberkommando"],
    TradingPost: ["Handelsposten", "Markt", "Handelsplatz", "Kaufmannsviertel", "Handelsdepot", "Frachtterminal", "Logistikzentrum", "Handelskomplex"],
    Tower: ["Wachturm", "Wachtturm", "Steinturm", "Bergfried", "Bastion", "Artilleriebastion", "Verteidigungsstation", "Wächterturm"],
    Port: ["Fischersteg", "Handelskai", "Hafen", "Festungshafen", "Marinewerft", "Industrielle Werft", "Flottenstützpunkt", "Flottenkommando"],
    SAM: ["SAM-Batterie I", "SAM-Batterie II", "SAM-Batterie III", "SAM-Batterie IV"],
    MissileSilo: ["Raketensilo I", "Raketensilo II", "Raketensilo III", "Raketensilo IV"],
  },
  fil: {
    City: ["Pamayanan", "Nayon", "Bayan", "Napapaderang Lungsod", "Kabisera", "Lungsod Industriyal", "Kalakhang Lungsod", "Megasiyudad"],
    MilitaryCamp: ["Kampong Militar", "Kuwartel", "Garnison", "Muog", "Base Militar", "Sentro ng Mobilisasyon", "Sentro ng Utos", "Estratikong Punong-himpilan"],
    TradingPost: ["Pamilihang Pangkalakalan", "Palengke", "Plasa ng Kalakalan", "Distrito ng Mangangalakal", "Depo ng Kalakalan", "Terminal ng Kargamento", "Sentro ng Lohistika", "Kompleks ng Kalakalan"],
    Tower: ["Tore ng Bantay", "Tore ng Guwardiya", "Tore na Bato", "Kuta", "Bastyon", "Bastyon ng Artilerya", "Himpilan ng Depensa", "Tore ng Tagapagbantay"],
    Port: ["Pantalan ng Pangingisda", "Pantalan ng Kalakalan", "Daungan", "Pinatibay na Daungan", "Pagawaan ng Barko", "Pagawaan ng Barkong Industriyal", "Base ng Plota", "Punong-himpilan ng Plota"],
    SAM: ["Bateryang SAM I", "Bateryang SAM II", "Bateryang SAM III", "Bateryang SAM IV"],
    MissileSilo: ["Silo ng Misayl I", "Silo ng Misayl II", "Silo ng Misayl III", "Silo ng Misayl IV"],
  },
  fr: {
    City: ["Établissement", "Village", "Bourg", "Cité fortifiée", "Capitale", "Ville industrielle", "Métropole", "Mégapole"],
    MilitaryCamp: ["Camp militaire", "Caserne", "Garnison", "Place forte", "Base militaire", "Centre de mobilisation", "Centre de commandement", "Commandement stratégique"],
    TradingPost: ["Comptoir commercial", "Marché", "Place du commerce", "Quartier marchand", "Dépôt commercial", "Terminal de fret", "Centre logistique", "Complexe commercial"],
    Tower: ["Tour de guet", "Tour de garde", "Tour de pierre", "Donjon", "Bastion", "Bastion d’artillerie", "Station défensive", "Tour sentinelle"],
    Port: ["Jetée de pêche", "Quai marchand", "Port", "Port fortifié", "Chantier naval", "Chantier naval industriel", "Base navale", "Commandement de la flotte"],
    SAM: ["Batterie SAM I", "Batterie SAM II", "Batterie SAM III", "Batterie SAM IV"],
    MissileSilo: ["Silo de missiles I", "Silo de missiles II", "Silo de missiles III", "Silo de missiles IV"],
  },
  id: {
    City: ["Permukiman", "Desa", "Kota kecil", "Kota bertembok", "Ibu kota", "Kota industri", "Metropolis", "Megakota"],
    MilitaryCamp: ["Kamp Militer", "Barak", "Garnisun", "Benteng", "Pangkalan Militer", "Pusat Mobilisasi", "Pusat Komando", "Komando Strategis"],
    TradingPost: ["Pos Dagang", "Pasar", "Alun-alun Dagang", "Kawasan Pedagang", "Depot Dagang", "Terminal Kargo", "Pusat Logistik", "Kompleks Dagang"],
    Tower: ["Menara Pengawas", "Menara Jaga", "Menara Batu", "Benteng Dalam", "Bastion", "Bastion Artileri", "Pos Pertahanan", "Menara Penjaga"],
    Port: ["Dermaga Nelayan", "Dermaga Dagang", "Pelabuhan", "Pelabuhan Berbenteng", "Galangan Angkatan Laut", "Galangan Industri", "Pangkalan Armada", "Komando Armada"],
    SAM: ["Baterai SAM I", "Baterai SAM II", "Baterai SAM III", "Baterai SAM IV"],
    MissileSilo: ["Silo Rudal I", "Silo Rudal II", "Silo Rudal III", "Silo Rudal IV"],
  },
  it: {
    City: ["Insediamento", "Villaggio", "Borgo", "Città murata", "Capitale", "Città industriale", "Metropoli", "Megalopoli"],
    MilitaryCamp: ["Campo militare", "Caserma", "Guarnigione", "Roccaforte", "Base militare", "Centro di mobilitazione", "Centro di comando", "Comando strategico"],
    TradingPost: ["Avamposto commerciale", "Mercato", "Piazza commerciale", "Quartiere mercantile", "Deposito commerciale", "Terminal merci", "Centro logistico", "Complesso commerciale"],
    Tower: ["Torre di vedetta", "Torre di guardia", "Torre di pietra", "Mastio", "Bastione", "Bastione d’artiglieria", "Stazione difensiva", "Torre sentinella"],
    Port: ["Molo da pesca", "Molo commerciale", "Porto", "Porto fortificato", "Arsenale navale", "Cantiere industriale", "Base della flotta", "Comando della flotta"],
    SAM: ["Batteria SAM I", "Batteria SAM II", "Batteria SAM III", "Batteria SAM IV"],
    MissileSilo: ["Silo missilistico I", "Silo missilistico II", "Silo missilistico III", "Silo missilistico IV"],
  },
  ja: {
    City: ["開拓地", "村", "町", "城壁都市", "首都", "工業都市", "大都市", "メガシティ"],
    MilitaryCamp: ["軍事キャンプ", "兵舎", "守備隊", "要塞", "軍事基地", "動員センター", "司令部", "戦略司令部"],
    TradingPost: ["交易所", "市場", "交易広場", "商業地区", "交易倉庫", "貨物ターミナル", "物流拠点", "交易複合施設"],
    Tower: ["見張り台", "警備塔", "石塔", "天守", "堡塁", "砲兵堡塁", "防衛拠点", "哨戒塔"],
    Port: ["漁港桟橋", "交易桟橋", "港", "要塞港", "海軍工廠", "工業造船所", "艦隊基地", "艦隊司令部"],
    SAM: ["SAM砲台 I", "SAM砲台 II", "SAM砲台 III", "SAM砲台 IV"],
    MissileSilo: ["ミサイルサイロ I", "ミサイルサイロ II", "ミサイルサイロ III", "ミサイルサイロ IV"],
  },
  ko: {
    City: ["정착지", "마을", "소도시", "성벽 도시", "수도", "산업 도시", "대도시", "메가시티"],
    MilitaryCamp: ["군사 캠프", "병영", "주둔지", "요새", "군사 기지", "동원 센터", "지휘 본부", "전략 사령부"],
    TradingPost: ["교역소", "시장", "교역 광장", "상인 지구", "교역 창고", "화물 터미널", "물류 허브", "교역 단지"],
    Tower: ["감시탑", "경비탑", "석탑", "성루", "보루", "포병 보루", "방어 기지", "파수탑"],
    Port: ["어업 부두", "교역 부두", "항구", "요새 항구", "해군 조선소", "산업 조선소", "함대 기지", "함대 사령부"],
    SAM: ["SAM 포대 I", "SAM 포대 II", "SAM 포대 III", "SAM 포대 IV"],
    MissileSilo: ["미사일 사일로 I", "미사일 사일로 II", "미사일 사일로 III", "미사일 사일로 IV"],
  },
  "pt-br": {
    City: ["Povoado", "Vila", "Cidade", "Cidade Murada", "Capital", "Cidade Industrial", "Metrópole", "Megacidade"],
    MilitaryCamp: ["Acampamento Militar", "Quartel", "Guarnição", "Fortaleza", "Base Militar", "Centro de Mobilização", "Centro de Comando", "Comando Estratégico"],
    TradingPost: ["Entreposto Comercial", "Mercado", "Praça Comercial", "Bairro Mercantil", "Depósito Comercial", "Terminal de Carga", "Centro Logístico", "Complexo Comercial"],
    Tower: ["Torre de Vigia", "Torre de Guarda", "Torre de Pedra", "Fortaleza", "Bastião", "Bastião de Artilharia", "Estação de Defesa", "Torre Sentinela"],
    Port: ["Cais de Pesca", "Cais Comercial", "Porto", "Porto Fortificado", "Estaleiro Naval", "Estaleiro Industrial", "Base da Frota", "Comando da Frota"],
    SAM: ["Bateria SAM I", "Bateria SAM II", "Bateria SAM III", "Bateria SAM IV"],
    MissileSilo: ["Silo de Mísseis I", "Silo de Mísseis II", "Silo de Mísseis III", "Silo de Mísseis IV"],
  },
  ru: {
    City: ["Поселение", "Деревня", "Городок", "Город за стеной", "Столица", "Промышленный город", "Мегаполис", "Мегагород"],
    MilitaryCamp: ["Военный лагерь", "Казармы", "Гарнизон", "Крепость", "Военная база", "Центр мобилизации", "Командный центр", "Стратегическое командование"],
    TradingPost: ["Торговый пост", "Рынок", "Торговая площадь", "Купеческий квартал", "Торговое депо", "Грузовой терминал", "Логистический центр", "Торговый комплекс"],
    Tower: ["Сторожевая башня", "Караульная башня", "Каменная башня", "Донжон", "Бастион", "Артиллерийский бастион", "Оборонительный пункт", "Башня стражи"],
    Port: ["Рыбацкий причал", "Торговый причал", "Порт", "Укреплённый порт", "Военная верфь", "Промышленная верфь", "База флота", "Командование флота"],
    SAM: ["Батарея SAM I", "Батарея SAM II", "Батарея SAM III", "Батарея SAM IV"],
    MissileSilo: ["Ракетная шахта I", "Ракетная шахта II", "Ракетная шахта III", "Ракетная шахта IV"],
  },
  tr: {
    City: ["Yerleşim", "Köy", "Kasaba", "Surlu Şehir", "Başkent", "Sanayi Şehri", "Metropol", "Mega Şehir"],
    MilitaryCamp: ["Askerî Kamp", "Kışla", "Garnizon", "Hisar", "Askerî Üs", "Seferberlik Merkezi", "Komuta Merkezi", "Stratejik Komutanlık"],
    TradingPost: ["Ticaret Karakolu", "Pazar", "Ticaret Meydanı", "Tüccar Mahallesi", "Ticaret Deposu", "Yük Terminali", "Lojistik Merkezi", "Ticaret Kompleksi"],
    Tower: ["Gözetleme Kulesi", "Muhafız Kulesi", "Taş Kule", "İç Kale", "Tabya", "Topçu Tabyası", "Savunma İstasyonu", "Nöbetçi Kulesi"],
    Port: ["Balıkçı İskelesi", "Ticaret İskelesi", "Liman", "Tahkimli Liman", "Donanma Tersanesi", "Sanayi Tersanesi", "Filo Üssü", "Filo Komutanlığı"],
    SAM: ["SAM Bataryası I", "SAM Bataryası II", "SAM Bataryası III", "SAM Bataryası IV"],
    MissileSilo: ["Füze Silosu I", "Füze Silosu II", "Füze Silosu III", "Füze Silosu IV"],
  },
  vi: {
    City: ["Khu định cư", "Làng", "Thị trấn", "Thành phố có tường", "Thủ đô", "Thành phố công nghiệp", "Đô thị lớn", "Siêu đô thị"],
    MilitaryCamp: ["Trại quân sự", "Doanh trại", "Đồn trú", "Thành trì", "Căn cứ quân sự", "Trung tâm động viên", "Trung tâm chỉ huy", "Bộ chỉ huy chiến lược"],
    TradingPost: ["Trạm giao thương", "Chợ", "Quảng trường thương mại", "Khu thương nhân", "Kho thương mại", "Bến hàng hóa", "Trung tâm hậu cần", "Khu phức hợp thương mại"],
    Tower: ["Tháp canh", "Tháp gác", "Tháp đá", "Tháp thành", "Pháo đài", "Pháo đài pháo binh", "Trạm phòng thủ", "Tháp canh gác"],
    Port: ["Cầu cá", "Cầu cảng thương mại", "Cảng", "Cảng kiên cố", "Xưởng hải quân", "Xưởng đóng tàu công nghiệp", "Căn cứ hạm đội", "Bộ chỉ huy hạm đội"],
    SAM: ["Trận địa SAM I", "Trận địa SAM II", "Trận địa SAM III", "Trận địa SAM IV"],
    MissileSilo: ["Hầm tên lửa I", "Hầm tên lửa II", "Hầm tên lửa III", "Hầm tên lửa IV"],
  },
  "zh-cn": {
    City: ["定居点", "村庄", "城镇", "城墙城市", "首都", "工业城市", "大都会", "特大城市"],
    MilitaryCamp: ["军事营地", "兵营", "驻军营地", "要塞", "军事基地", "动员中心", "指挥中心", "战略司令部"],
    TradingPost: ["贸易站", "市场", "贸易广场", "商人街区", "贸易仓库", "货运站", "物流中心", "贸易综合体"],
    Tower: ["瞭望塔", "警卫塔", "石塔", "主堡", "堡垒", "炮兵堡垒", "防御站", "哨卫塔"],
    Port: ["渔业码头", "贸易码头", "港口", "设防港口", "海军船坞", "工业船厂", "舰队基地", "舰队司令部"],
    SAM: ["SAM防空炮台 I", "SAM防空炮台 II", "SAM防空炮台 III", "SAM防空炮台 IV"],
    MissileSilo: ["导弹发射井 I", "导弹发射井 II", "导弹发射井 III", "导弹发射井 IV"],
  },
};
type PageLocale = {
  roles: Record<BuildingKind, string>;
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
  immediate: string;
  road: string;
  route: string;
  railway: string;
  industrial: string;
  nav: string;
};

const pageLocales: Record<string, PageLocale> = {
  en: {
    roles: { City: "Troop capacity and the nuclear launch era requirement.", Tower: "Area defense and resistance to capture.", TradingPost: "Gold income and a connected land trade network.", Port: "Fleet capacity, ship repair, maritime access, and fleet speed that stacks up to 30%.", MilitaryCamp: "Troop production and faster land conquest.", SAM: "Automatic interception of incoming nuclear weapons.", MissileSilo: "Launch capacity for atomic, hydrogen and MIRV weapons." },
    eras: ["Tribal", "Agrarian", "Classical", "Medieval", "Early Modern", "Industrial", "Contemporary", "Future"],
    stats: { reserve: "troop capacity", troops: "troops/s", reinforcements: "reinforcements", capture: "% conquest speed", trade: "gold/s on an active land route", travel: "× trade delivery frequency", range: "tiles of range", resistance: "capture resistance weight", fleet: "fleet slots", docks: "docks", repair: "repair/s", charges: "charges", gold: "gold/s", speed: "ship speed bonus", defense: "troop loss reduction", fire_rate: "interceptors/min" },
    intro: "New buildings start at N1. Industrial City N6 unlocks SAM batteries and missile silos.",
    note: "Prices are for one building: construction at N1, then each upgrade and total invested. Income uses Normal speed; effects show the tier total and its added benefit.",
    level: "Level", era: "Era", build: "Build", upgrade: "Upgrade", invested: "invested", thisLevel: "added this level", immediate: "Immediate reinforcements",
    route: "Trading Posts connect to any nearby owned building from N1; land trade income requires a valid connected route.", railway: "Roads upgrade by era: dirt tracks (N1), stone roads (N3), railways (N6), and high-speed maglev lines (N8).", industrial: "Industrial City N6 unlocks SAM batteries and missile silos.", nav: "Building lines",
  },
  es: {
    roles: { City: "Capacidad de tropas y requisito de era para lanzar armas nucleares.", Tower: "Defensa de zona y resistencia a la captura.", TradingPost: "Ingresos de oro y una red comercial terrestre conectada.", Port: "Capacidad de flota, reparación de barcos, acceso marítimo y velocidad naval acumulable hasta 30%.", MilitaryCamp: "Generación de tropas y conquista terrestre más rápida.", SAM: "Intercepción automática de armas nucleares entrantes.", MissileSilo: "Capacidad para lanzar armas atómicas, de hidrógeno y MIRV." },
    eras: ["Tribal", "Agraria", "Clásica", "Medieval", "Moderna temprana", "Industrial", "Contemporánea", "Futura"],
    stats: { reserve: "capacidad de tropas", troops: "tropas/s", reinforcements: "refuerzos", capture: "% de avance de conquista", trade: "oro/s con ruta terrestre activa", travel: "× frecuencia de entregas", range: "tiles de alcance", resistance: "resistencia a captura", fleet: "plazas de flota", docks: "atraques", repair: "reparación/s", charges: "cargas", gold: "oro/s", speed: "bono de velocidad naval", defense: "reducción de pérdidas de tropas", fire_rate: "interceptores/min" },
    intro: "Los edificios nuevos empiezan en N1. La Ciudad Industrial N6 desbloquea las baterías SAM y los silos.",
    note: "Precios para un edificio: construcción en N1, costo de cada mejora y total invertido. Ingresos a velocidad Normal; cada efecto muestra el total y lo que añade el nivel.",
    level: "Nivel", era: "Era", build: "Construcción", upgrade: "Mejora", invested: "invertido", thisLevel: "añade este nivel", immediate: "Refuerzo inmediato",
    route: "El Puesto comercial conecta con cualquier edificio propio cercano desde N1; el ingreso requiere una ruta terrestre conectada y válida.", railway: "Las rutas evolucionan por era: terracería (N1), calzada de piedra (N3), ferrocarril (N6) y alta velocidad (N8).", industrial: "La Ciudad Industrial N6 desbloquea baterías SAM y silos.", nav: "Líneas de edificios",
  },
  ar: {
    roles: { City: "سعة القوات ومتطلب عصر إطلاق الأسلحة النووية.", Tower: "دفاع المنطقة ومقاومة الاستيلاء.", TradingPost: "دخل الذهب وشبكة تجارة برية متصلة.", Port: "سعة الأسطول وإصلاح السفن والوصول البحري وزيادة سرعة الأسطول حتى 30%.", MilitaryCamp: "إنتاج القوات وتسريع الاستيلاء البري.", SAM: "اعتراض تلقائي للأسلحة النووية القادمة.", MissileSilo: "إطلاق الأسلحة الذرية والهيدروجينية وMIRV." },
    eras: ["قبلية", "زراعية", "كلاسيكية", "العصور الوسطى", "حديثة مبكرة", "صناعية", "معاصرة", "مستقبلية"],
    stats: { reserve: "سعة القوات", troops: "قوات/ث", reinforcements: "تعزيزات", capture: "% سرعة الاستيلاء", trade: "ذهب/ث مع مسار بري نشط", travel: "× وتيرة التسليم التجاري", range: "مدى بالخانات", resistance: "مقاومة الاستيلاء", fleet: "سعة الأسطول", docks: "أرصفة", repair: "إصلاح/ث", charges: "شحنات", gold: "ذهب/ث", speed: "مكافأة سرعة السفن", defense: "تقليل خسائر القوات", fire_rate: "صواريخ اعتراضية/دقيقة" },
    intro: "تبدأ المباني الجديدة بالمستوى 1. تفتح المدينة الصناعية بالمستوى 6 بطاريات SAM وصوامع الصواريخ.",
    note: "الأسعار لمبنى واحد: الإنشاء عند المستوى 1، ثم تكلفة كل ترقية والإجمالي. الدخل بسرعة عادية، ويعرض كل تأثير الإجمالي وما يضيفه المستوى.",
    level: "المستوى", era: "العصر", build: "إنشاء", upgrade: "ترقية", invested: "الإجمالي", thisLevel: "ما يضيفه المستوى", immediate: "تعزيز فوري",
    route: "ينشئ المركز التجاري الطرق من المستوى 1؛ ويتطلب دخل التجارة البرية مساراً صالحاً ومتصلًا.", railway: "تظهر سكك الحديد عند المستوى 6.", industrial: "تفتح المدينة الصناعية 6 بطاريات SAM وصوامع الصواريخ.", nav: "خطوط المباني",
  },
  de: {
    roles: { City: "Truppenkapazität und Voraussetzung für das Atomzeitalter.", Tower: "Gebietsverteidigung und Eroberungswiderstand.", TradingPost: "Goldeinkommen und ein verbundenes Handelsnetz über Land.", Port: "Flottenkapazität, Schiffsreparatur, Zugang zum Meer und bis zu 30 % stapelbarer Flottengeschwindigkeit.", MilitaryCamp: "Truppenproduktion und schnellere Eroberung an Land.", SAM: "Automatisches Abfangen eingehender Atomwaffen.", MissileSilo: "Startkapazität für Atom-, Wasserstoff- und MIRV-Waffen." },
    eras: ["Stammeszeit", "Agrarzeit", "Antike", "Mittelalter", "Frühe Neuzeit", "Industriezeitalter", "Gegenwart", "Zukunft"],
    stats: { reserve: "Truppenkapazität", troops: "Truppen/s", reinforcements: "Verstärkung", capture: "% Eroberungstempo", trade: "Gold/s bei aktiver Landroute", travel: "× Handelstransporte", range: "Reichweite in Feldern", resistance: "Eroberungswiderstand", fleet: "Flottenplätze", docks: "Liegeplätze", repair: "Reparatur/s", charges: "Ladungen", gold: "Gold/s", speed: "Schiffsgeschwindigkeitsbonus", defense: "Verringerung der Truppenverluste", fire_rate: "Abfangraketen/min" },
    intro: "Neue Gebäude beginnen auf Stufe 1. Eine Industriestadt auf Stufe 6 schaltet SAM-Batterien und Raketensilos frei.",
    note: "Preise für ein Gebäude: Bau auf Stufe 1, danach jeder Ausbau und die Gesamtsumme. Einkommen bei Normaltempo; Effekte zeigen Gesamtwert und Zuwachs.",
    level: "Stufe", era: "Zeitalter", build: "Bau", upgrade: "Ausbau", invested: "Gesamt", thisLevel: "Zuwachs dieser Stufe", immediate: "Sofortige Verstärkung",
    route: "Handelsposten bauen ab Stufe 1 Straßen; Handelseinkommen braucht eine gültige, verbundene Route.", railway: "Ab Stufe 6 erscheinen Eisenbahngleise.", industrial: "Industriestadt Stufe 6 schaltet SAM-Batterien und Raketensilos frei.", nav: "Gebäudelinien",
  },
  fil: {
    roles: { City: "Kapasidad ng tropa at kailangang panahon para sa mga sandatang nuklear.", Tower: "Depensa ng lugar at panlaban sa pagsakop.", TradingPost: "Kita sa ginto at konektadong rutang pangkalakalan sa lupa.", Port: "Kapasidad ng plota, pagkukumpuni ng barko, daanang pandagat, at bilis ng plota na naiipon hanggang 30%.", MilitaryCamp: "Produksyon ng tropa at mas mabilis na pagsakop sa lupa.", SAM: "Awtomatikong pagsabat sa paparating na sandatang nuklear.", MissileSilo: "Kakayahang maglunsad ng atomic, hydrogen at MIRV." },
    eras: ["Pantribo", "Pansakahan", "Klasiko", "Gitnang Panahon", "Maagang Makabago", "Industriyal", "Kasalukuyan", "Hinaharap"],
    stats: { reserve: "kapasidad ng tropa", troops: "tropa/s", reinforcements: "dagdag na tropa", capture: "% bilis ng pagsakop", trade: "ginto/s sa aktibong rutang lupa", travel: "× dalas ng kalakalan", range: "layo sa tile", resistance: "panlaban sa pagsakop", fleet: "puwesto ng plota", docks: "dungguan", repair: "ayos/s", charges: "karga", gold: "ginto/s", speed: "dagdag-bilis ng barko", defense: "pagbawas sa pagkatalo ng tropa", fire_rate: "interceptor/min" },
    intro: "Nagsisimula sa antas 1 ang mga bagong gusali. Binubuksan ng Lungsod Industriyal antas 6 ang mga SAM at silo ng misil.",
    note: "Presyo para sa isang gusali: paggawa sa antas 1, bawat upgrade at kabuuang nagastos. Kita sa Normal na bilis; ipinapakita ang kabuuan at dagdag ng antas.",
    level: "Antas", era: "Panahon", build: "Gawa", upgrade: "Upgrade", invested: "kabuuan", thisLevel: "dagdag sa antas na ito", immediate: "Agarang dagdag na tropa",
    route: "Gumagawa ng mga daan ang Trading Post mula antas 1; kailangan ng wastong konektadong ruta para kumita sa kalakalang panlupa.", railway: "Lilitaw ang riles sa antas 6.", industrial: "Binubuksan ng Lungsod Industriyal antas 6 ang mga SAM at silo.", nav: "Mga linya ng gusali",
  },
  fr: {
    roles: { City: "Capacité des troupes et condition d’accès à l’ère nucléaire.", Tower: "Défense de zone et résistance à la capture.", TradingPost: "Revenus en or et réseau commercial terrestre connecté.", Port: "Capacité de flotte, réparation des navires, accès maritime et bonus de vitesse cumulable jusqu’à 30%.", MilitaryCamp: "Production de troupes et conquête terrestre accélérée.", SAM: "Interception automatique des armes nucléaires entrantes.", MissileSilo: "Capacité de lancement atomique, hydrogène et MIRV." },
    eras: ["Tribal", "Agraire", "Classique", "Médiévale", "Moderne ancienne", "Industrielle", "Contemporaine", "Futuriste"],
    stats: { reserve: "capacité des troupes", troops: "troupes/s", reinforcements: "renforts", capture: "% vitesse de conquête", trade: "or/s avec route terrestre active", travel: "× fréquence des livraisons", range: "portée en cases", resistance: "résistance à la capture", fleet: "places de flotte", docks: "places à quai", repair: "réparation/s", charges: "charges", gold: "or/s", speed: "bonus de vitesse des navires", defense: "réduction des pertes de troupes", fire_rate: "intercepteurs/min" },
    intro: "Les nouveaux bâtiments commencent au niveau 1. La ville industrielle N6 débloque les batteries SAM et les silos.",
    note: "Prix pour un bâtiment : construction au N1, coût de chaque amélioration et total investi. Revenus à vitesse normale ; les effets montrent le total et l’apport du niveau.",
    level: "Niveau", era: "Ère", build: "Construction", upgrade: "Amélioration", invested: "investi", thisLevel: "apport de ce niveau", immediate: "Renforts immédiats",
    route: "Les comptoirs créent des routes dès le N1 ; le revenu commercial exige une route terrestre valide et connectée.", railway: "Les voies ferrées apparaissent au N6.", industrial: "La ville industrielle N6 débloque les batteries SAM et les silos.", nav: "Lignes de bâtiments",
  },
  id: {
    roles: { City: "Kapasitas pasukan dan syarat era peluncuran nuklir.", Tower: "Pertahanan area dan ketahanan terhadap perebutan.", TradingPost: "Pendapatan emas dan jaringan dagang darat yang terhubung.", Port: "Kapasitas armada, perbaikan kapal, akses maritim, dan bonus kecepatan armada yang menumpuk hingga 30%.", MilitaryCamp: "Produksi pasukan dan perebutan darat yang lebih cepat.", SAM: "Pencegatan otomatis senjata nuklir yang datang.", MissileSilo: "Kapasitas peluncuran atom, hidrogen, dan MIRV." },
    eras: ["Kesukuan", "Agraris", "Klasik", "Abad Pertengahan", "Modern Awal", "Industri", "Kontemporer", "Masa Depan"],
    stats: { reserve: "kapasitas pasukan", troops: "pasukan/dtk", reinforcements: "bala bantuan", capture: "% kecepatan merebut", trade: "emas/dtk dengan rute darat aktif", travel: "× frekuensi pengiriman", range: "jangkauan petak", resistance: "ketahanan perebutan", fleet: "slot armada", docks: "dermaga", repair: "perbaikan/dtk", charges: "muatan", gold: "emas/dtk", speed: "bonus kecepatan kapal", defense: "pengurangan kehilangan pasukan", fire_rate: "interseptor/menit" },
    intro: "Bangunan baru dimulai dari level 1. Kota Industri level 6 membuka Baterai SAM dan Silo Rudal.",
    note: "Harga untuk satu bangunan: pembangunan level 1, biaya tiap upgrade, dan total investasi. Pendapatan pada kecepatan Normal; efek menunjukkan total dan tambahannya.",
    level: "Level", era: "Era", build: "Bangun", upgrade: "Upgrade", invested: "total", thisLevel: "tambahan level ini", immediate: "Bala bantuan langsung",
    route: "Pos Dagang membangun jalan sejak level 1; pendapatan dagang darat memerlukan rute terhubung yang valid.", railway: "Rel kereta muncul pada level 6.", industrial: "Kota Industri level 6 membuka Baterai SAM dan Silo Rudal.", nav: "Jalur bangunan",
  },
  it: {
    roles: { City: "Capacità delle truppe e requisito per l’era nucleare.", Tower: "Difesa dell’area e resistenza alla conquista.", TradingPost: "Entrate in oro e rete commerciale terrestre collegata.", Port: "Capacità della flotta, riparazione delle navi, accesso marittimo e velocità navale cumulabile fino al 30%.", MilitaryCamp: "Produzione di truppe e conquista terrestre più rapida.", SAM: "Intercettazione automatica delle armi nucleari in arrivo.", MissileSilo: "Capacità di lancio per armi atomiche, a idrogeno e MIRV." },
    eras: ["Tribale", "Agraria", "Classica", "Medievale", "Prima età moderna", "Industriale", "Contemporanea", "Futura"],
    stats: { reserve: "capacità truppe", troops: "truppe/s", reinforcements: "rinforzi", capture: "% velocità conquista", trade: "oro/s con rotta terrestre attiva", travel: "× frequenza consegne", range: "raggio in caselle", resistance: "resistenza alla conquista", fleet: "posti flotta", docks: "ormeggi", repair: "riparazione/s", charges: "cariche", gold: "oro/s", speed: "bonus velocità navale", defense: "riduzione delle perdite di truppe", fire_rate: "intercettori/min" },
    intro: "I nuovi edifici iniziano dal livello 1. La Città industriale N6 sblocca batterie SAM e silos missilistici.",
    note: "Prezzi per un edificio: costruzione al livello 1, costo di ogni upgrade e totale investito. Entrate a velocità Normale; gli effetti mostrano totale e incremento.",
    level: "Livello", era: "Era", build: "Costruzione", upgrade: "Upgrade", invested: "investito", thisLevel: "incremento del livello", immediate: "Rinforzi immediati",
    route: "Gli avamposti commerciali creano strade dal livello 1; il reddito richiede una rotta terrestre valida e collegata.", railway: "I binari ferroviari appaiono al livello 6.", industrial: "La Città industriale N6 sblocca batterie SAM e silos.", nav: "Linee di edifici",
  },
  ja: {
    roles: { City: "兵力上限と核兵器時代の条件。", Tower: "地域防衛と占領への抵抗。", TradingPost: "金収入と接続された陸上交易網。", Port: "艦隊容量、船の修理、海上アクセス、最大30%まで累積する艦隊速度ボーナス。", MilitaryCamp: "兵力生産と陸上占領の加速。", SAM: "飛来する核兵器を自動迎撃。", MissileSilo: "原子・水素・MIRV兵器の発射能力。" },
    eras: ["部族", "農耕", "古典", "中世", "近世", "産業", "現代", "未来"],
    stats: { reserve: "兵力上限", troops: "兵/s", reinforcements: "即時増援", capture: "%占領速度", trade: "陸路接続中の金/s", travel: "×交易配送頻度", range: "射程マス", resistance: "占領耐性", fleet: "艦隊枠", docks: "停泊枠", repair: "修理/s", charges: "弾数", gold: "金/s", speed: "船速ボーナス", defense: "兵力損失の軽減", fire_rate: "迎撃弾/分" },
    intro: "新しい建物はレベル1から始まります。工業都市N6でSAM砲台とミサイルサイロが解放されます。",
    note: "価格は建物1つ分です。N1の建設費、各強化費、累計額を表示します。収入は通常速度、効果は累計とそのレベルの増加分です。",
    level: "レベル", era: "時代", build: "建設", upgrade: "強化", invested: "累計", thisLevel: "このレベルの増加", immediate: "即時増援",
    route: "交易所はN1から道を作ります。陸上交易の収入には有効な接続ルートが必要です。", railway: "N6で線路が表示されます。", industrial: "工業都市N6でSAM砲台とサイロが解放されます。", nav: "建物ライン",
  },
  ko: {
    roles: { City: "병력 수용량과 핵무기 시대 진입 조건.", Tower: "지역 방어와 점령 저항.", TradingPost: "골드 수입과 연결된 육상 교역망.", Port: "함대 수용량, 함선 수리, 해상 진출, 최대 30%까지 중첩되는 함대 속도 보너스.", MilitaryCamp: "병력 생산과 육상 점령 가속.", SAM: "날아오는 핵무기를 자동 요격.", MissileSilo: "원자·수소·MIRV 무기 발사 능력." },
    eras: ["부족", "농경", "고전", "중세", "근세", "산업", "현대", "미래"],
    stats: { reserve: "병력 수용량", troops: "병력/초", reinforcements: "즉시 증원", capture: "% 점령 속도", trade: "육상 경로 연결 시 골드/초", travel: "× 교역 배송 빈도", range: "타일 사거리", resistance: "점령 저항", fleet: "함대 슬롯", docks: "정박지", repair: "수리/초", charges: "탄약", gold: "골드/초", speed: "함선 속도 보너스", defense: "병력 손실 감소", fire_rate: "요격탄/분" },
    intro: "새 건물은 1레벨부터 시작합니다. 산업 도시 6레벨에서 SAM 포대와 미사일 사일로가 해금됩니다.",
    note: "가격은 건물 1개 기준입니다. 1레벨 건설비, 각 업그레이드 비용과 누적 비용을 표시합니다. 수입은 일반 속도, 효과는 총량과 해당 레벨의 증가량입니다.",
    level: "레벨", era: "시대", build: "건설", upgrade: "업그레이드", invested: "누적", thisLevel: "이번 레벨 증가량", immediate: "즉시 증원",
    route: "교역소는 1레벨부터 길을 만듭니다. 육상 교역 수입에는 유효한 연결 경로가 필요합니다.", railway: "6레벨부터 철로가 표시됩니다.", industrial: "산업 도시 6레벨에서 SAM 포대와 사일로가 해금됩니다.", nav: "건물 계열",
  },
  "pt-br": {
    roles: { City: "Capacidade de tropas e requisito da era nuclear.", Tower: "Defesa territorial e resistência à conquista.", TradingPost: "Renda de ouro e rede comercial terrestre conectada.", Port: "Capacidade da frota, reparo de navios, acesso marítimo e bônus de velocidade acumulável até 30%.", MilitaryCamp: "Produção de tropas e conquista terrestre mais rápida.", SAM: "Interceptação automática de armas nucleares que chegam.", MissileSilo: "Capacidade para lançar armas atômicas, de hidrogênio e MIRV." },
    eras: ["Tribal", "Agrária", "Clássica", "Medieval", "Moderna inicial", "Industrial", "Contemporânea", "Futura"],
    stats: { reserve: "capacidade de tropas", troops: "tropas/s", reinforcements: "reforços", capture: "% velocidade de conquista", trade: "ouro/s com rota terrestre ativa", travel: "× frequência de entregas", range: "alcance em casas", resistance: "resistência à conquista", fleet: "vagas de frota", docks: "vagas de atracação", repair: "reparo/s", charges: "cargas", gold: "ouro/s", speed: "bônus de velocidade naval", defense: "redução de perdas de tropas", fire_rate: "interceptadores/min" },
    intro: "Novos edifícios começam no N1. Cidade Industrial N6 libera baterias SAM e silos de mísseis.",
    note: "Preços para um edifício: construção no N1, custo de cada melhoria e total investido. Renda na velocidade Normal; efeitos mostram o total e o acréscimo do nível.",
    level: "Nível", era: "Era", build: "Construção", upgrade: "Melhoria", invested: "investido", thisLevel: "acréscimo deste nível", immediate: "Reforços imediatos",
    route: "Entrepostos criam estradas desde o N1; a renda comercial terrestre exige uma rota válida e conectada.", railway: "Os trilhos aparecem no N6.", industrial: "Cidade Industrial N6 libera baterias SAM e silos.", nav: "Linhas de edifícios",
  },
  ru: {
    roles: { City: "Вместимость войск и требование ядерной эпохи.", Tower: "Оборона территории и сопротивление захвату.", TradingPost: "Доход золота и связанная сухопутная торговая сеть.", Port: "Вместимость флота, ремонт кораблей, выход к морю и суммируемый бонус скорости флота до 30%.", MilitaryCamp: "Производство войск и ускорение сухопутного захвата.", SAM: "Автоматический перехват входящего ядерного оружия.", MissileSilo: "Запуск атомных, водородных ракет и MIRV." },
    eras: ["Племенная", "Аграрная", "Классическая", "Средневековая", "Раннее Новое время", "Индустриальная", "Современная", "Будущая"],
    stats: { reserve: "вместимость войск", troops: "войск/с", reinforcements: "подкрепления", capture: "% скорости захвата", trade: "золота/с при активном сухопутном пути", travel: "× частота торговых доставок", range: "дальность в клетках", resistance: "сопротивление захвату", fleet: "места флота", docks: "места в доках", repair: "ремонт/с", charges: "заряды", gold: "золота/с", speed: "бонус скорости флота", defense: "снижение потерь войск", fire_rate: "перехватчиков/мин" },
    intro: "Новые здания начинаются с уровня 1. Индустриальный город 6-го уровня открывает батареи SAM и ракетные шахты.",
    note: "Цены для одного здания: постройка уровня 1, стоимость каждого улучшения и общая сумма. Доход при обычной скорости; эффекты показывают итог и прирост уровня.",
    level: "Уровень", era: "Эпоха", build: "Постройка", upgrade: "Улучшение", invested: "всего", thisLevel: "прирост уровня", immediate: "Мгновенное подкрепление",
    route: "Торговые посты прокладывают дороги уже на 1-м уровне; для сухопутного дохода нужен действующий связанный маршрут.", railway: "Железнодорожные пути появляются на уровне 6.", industrial: "Индустриальный город 6-го уровня открывает SAM и ракетные шахты.", nav: "Линии зданий",
  },
  tr: {
    roles: { City: "Asker kapasitesi ve nükleer çağ gereksinimi.", Tower: "Bölge savunması ve ele geçirilmeye direnç.", TradingPost: "Altın geliri ve bağlantılı kara ticaret ağı.", Port: "Filo kapasitesi, gemi onarımı, deniz erişimi ve toplamda %30’a kadar biriken filo hızı bonusu.", MilitaryCamp: "Asker üretimi ve daha hızlı kara fethi.", SAM: "Gelen nükleer silahları otomatik engeller.", MissileSilo: "Atom, hidrojen ve MIRV silahlarını fırlatma kapasitesi." },
    eras: ["Kabile", "Tarım", "Klasik", "Orta Çağ", "Erken Modern", "Sanayi", "Çağdaş", "Gelecek"],
    stats: { reserve: "asker kapasitesi", troops: "asker/sn", reinforcements: "takviye", capture: "% fetih hızı", trade: "etkin kara rotasında altın/sn", travel: "× ticaret teslimat sıklığı", range: "kare menzili", resistance: "ele geçirilme direnci", fleet: "filo yuvası", docks: "rıhtım", repair: "onarım/sn", charges: "yük", gold: "altın/sn", speed: "gemi hızı bonusu", defense: "asker kaybı azaltma", fire_rate: "önleyici/dk" },
    intro: "Yeni binalar 1. seviyeden başlar. 6. seviye Sanayi Şehri SAM bataryalarını ve füze silolarını açar.",
    note: "Fiyatlar tek bina içindir: 1. seviye inşaatı, her yükseltme ve toplam yatırım. Gelir Normal hızda; etkiler toplamı ve seviye artışını gösterir.",
    level: "Seviye", era: "Çağ", build: "İnşa", upgrade: "Yükseltme", invested: "toplam", thisLevel: "bu seviyenin artışı", immediate: "Anında takviye",
    route: "Ticaret Karakolları 1. seviyeden itibaren yol yapar; kara ticareti geliri için geçerli ve bağlantılı rota gerekir.", railway: "Demiryolu rayları 6. seviyede görünür.", industrial: "6. seviye Sanayi Şehri SAM ve füze silolarını açar.", nav: "Bina hatları",
  },
  vi: {
    roles: { City: "Sức chứa quân và điều kiện mở thời đại hạt nhân.", Tower: "Phòng thủ lãnh thổ và kháng chiếm đóng.", TradingPost: "Thu nhập vàng và mạng lưới thương mại đường bộ kết nối.", Port: "Sức chứa hạm đội, sửa tàu, tiếp cận biển và cộng dồn tốc độ hạm đội tối đa 30%.", MilitaryCamp: "Sản xuất quân và chiếm đất nhanh hơn.", SAM: "Tự động đánh chặn vũ khí hạt nhân đang tới.", MissileSilo: "Khả năng phóng vũ khí nguyên tử, hydro và MIRV." },
    eras: ["Bộ lạc", "Nông nghiệp", "Cổ điển", "Trung cổ", "Cận đại", "Công nghiệp", "Đương đại", "Tương lai"],
    stats: { reserve: "sức chứa quân", troops: "quân/giây", reinforcements: "quân tiếp viện", capture: "% tốc độ chiếm đất", trade: "vàng/giây khi có tuyến bộ hoạt động", travel: "× tần suất giao hàng", range: "tầm theo ô", resistance: "kháng chiếm đóng", fleet: "chỗ hạm đội", docks: "chỗ neo đậu", repair: "sửa/giây", charges: "lượt", gold: "vàng/giây", speed: "tăng tốc độ tàu", defense: "giảm tổn thất quân", fire_rate: "đạn đánh chặn/phút" },
    intro: "Công trình mới bắt đầu ở cấp 1. Thành phố Công nghiệp cấp 6 mở khóa trận địa SAM và hầm phóng tên lửa.",
    note: "Giá cho một công trình: xây cấp 1, chi phí từng lần nâng cấp và tổng đã đầu tư. Thu nhập ở tốc độ Normal; hiệu ứng gồm tổng và phần tăng thêm.",
    level: "Cấp", era: "Thời đại", build: "Xây", upgrade: "Nâng cấp", invested: "tổng đầu tư", thisLevel: "tăng ở cấp này", immediate: "Quân tiếp viện tức thì",
    route: "Trạm giao thương xây đường từ cấp 1; thu nhập thương mại đường bộ cần tuyến kết nối hợp lệ.", railway: "Đường ray xuất hiện từ cấp 6.", industrial: "Thành phố Công nghiệp cấp 6 mở khóa SAM và hầm phóng tên lửa.", nav: "Các dòng công trình",
  },
  "zh-cn": {
    roles: { City: "兵力容量及核武器时代的前置要求。", Tower: "区域防御与抗占领能力。", TradingPost: "金币收入及已连接的陆上贸易网络。", Port: "舰队容量、舰船维修、海上通道，以及可叠加至30%的舰队速度加成。", MilitaryCamp: "兵力生产并加快陆地占领。", SAM: "自动拦截来袭核武器。", MissileSilo: "发射原子弹、氢弹和MIRV的能力。" },
    eras: ["部落", "农耕", "古典", "中世纪", "近代早期", "工业", "当代", "未来"],
    stats: { reserve: "兵力容量", troops: "兵力/秒", reinforcements: "即时增援", capture: "%占领速度", trade: "陆路连接生效时的金币/秒", travel: "×贸易交付频率", range: "射程格数", resistance: "抗占领权重", fleet: "舰队位", docks: "停泊位", repair: "维修/秒", charges: "弹药", gold: "金币/秒", speed: "舰船速度加成", defense: "减少部队损失", fire_rate: "拦截弹/分钟" },
    intro: "新建筑从1级开始。工业城市达到6级后解锁SAM防空炮台和导弹发射井。",
    note: "价格按单个建筑计算：1级建造、每级升级和累计投入。收入按普通速度；效果显示总值及本级增量。",
    level: "等级", era: "时代", build: "建造", upgrade: "升级", invested: "累计投入", thisLevel: "本级增加", immediate: "即时增援",
    route: "贸易站从1级开始修路；陆上贸易收入需要有效且已连接的路线。", railway: "6级开始显示铁路轨道。", industrial: "工业城市6级解锁SAM和导弹发射井。", nav: "建筑线路",
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

export function buildingPageText(locale: string): PageLocale & { gold: string } {
  const code = localeCode(locale);
  return { ...(pageLocales[code] ?? pageLocales.en), gold: goldUnits[code] ?? goldUnits.en };
}
