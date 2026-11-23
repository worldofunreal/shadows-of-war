const lines: Record<string, { en: string[]; es: string[]; role: [string, string] }> = {
  City: {
    role: ["Troop capacity and your launch-era requirement.", "Capacidad de tropas y requisito de era nuclear."],
    en: ["Settlement", "Village", "Town", "Walled City", "Capital", "Industrial City", "Metropolis", "Megacity"],
    es: ["Asentamiento", "Aldea", "Pueblo", "Ciudad amurallada", "Capital", "Ciudad industrial", "Metrópolis", "Megaciudad"],
  },
  MilitaryCamp: {
    role: ["Rapid troop production and faster land conquest.", "Producción rápida de tropas y conquista terrestre."],
    en: ["Military Camp", "Barracks", "Garrison", "Stronghold", "Military Base", "Mobilization Center", "Command Center", "Strategic Command"],
    es: ["Campamento militar", "Cuartel", "Guarnición", "Bastión militar", "Base militar", "Centro de movilización", "Centro de mando", "Mando estratégico"],
  },
  TradingPost: {
    role: ["Gold income and a growing overland trade network.", "Ingresos de oro y una red comercial terrestre que mejora."],
    en: ["Trading Post", "Market", "Trade Square", "Merchant Quarter", "Trade Depot", "Freight Terminal", "Logistics Hub", "Trade Complex"],
    es: ["Puesto comercial", "Mercado", "Plaza comercial", "Barrio mercantil", "Depósito comercial", "Terminal de carga", "Centro logístico", "Complejo comercial"],
  },
  Tower: {
    role: ["Territory defense, capture resistance and local coverage.", "Defensa territorial, resistencia a captura y cobertura local."],
    en: ["Watchtower", "Guard Tower", "Stone Tower", "Keep", "Bastion", "Artillery Bastion", "Defense Station", "Sentinel Tower"],
    es: ["Atalaya", "Torre de guardia", "Torre de piedra", "Torreón", "Bastión", "Bastión de artillería", "Estación defensiva", "Torre centinela"],
  },
  Port: {
    role: ["Fleet capacity, ship repair and maritime access.", "Capacidad de flota, reparación y acceso marítimo."],
    en: ["Fishing Pier", "Trading Dock", "Harbor", "Fortified Harbor", "Naval Yard", "Industrial Shipyard", "Fleet Base", "Fleet Command"],
    es: ["Muelle pesquero", "Muelle comercial", "Puerto", "Puerto fortificado", "Astillero naval", "Astillero industrial", "Base naval", "Mando naval"],
  },
  SAM: {
    role: ["Automatic interception of incoming nuclear weapons.", "Intercepción automática de armas nucleares entrantes."],
    en: ["SAM Battery I", "SAM Battery II", "SAM Battery III", "SAM Battery IV"],
    es: ["Batería SAM I", "Batería SAM II", "Batería SAM III", "Batería SAM IV"],
  },
  MissileSilo: {
    role: ["Launch capacity for atomic, hydrogen and MIRV weapons.", "Capacidad de lanzamiento atómico, hidrógeno y MIRV."],
    en: ["Missile Silo I", "Missile Silo II", "Missile Silo III", "Missile Silo IV"],
    es: ["Silo de misiles I", "Silo de misiles II", "Silo de misiles III", "Silo de misiles IV"],
  },
};

type BuildingKind = keyof typeof lines;
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
  route: string;
  railway: string;
  industrial: string;
  nav: string;
};

const pageLocales: Record<string, PageLocale> = {
  en: {
    roles: { City: "Troop capacity and the nuclear launch era requirement.", Tower: "Area defense and resistance to capture.", TradingPost: "Gold income and a connected land trade network.", Port: "Fleet capacity, ship repair and maritime access.", MilitaryCamp: "Troop production and faster land conquest.", SAM: "Automatic interception of incoming nuclear weapons.", MissileSilo: "Launch capacity for atomic, hydrogen and MIRV weapons." },
    eras: ["Tribal", "Agrarian", "Classical", "Medieval", "Early Modern", "Industrial", "Contemporary", "Future"],
    stats: { reserve: "troop capacity", troops: "troops/s", reinforcements: "reinforcements", capture: "% conquest speed", trade: "gold/s on an active land route", travel: "× trade delivery frequency", range: "tiles of range", resistance: "capture resistance weight", fleet: "fleet slots", docks: "docks", repair: "repair/s", charges: "charges" },
    intro: "City level sets the starting tier of new buildings. Industrial City N6 unlocks SAM batteries and missile silos.",
    note: "Prices are for one building: construction at N1, then each upgrade and total invested. Income uses Normal speed (0.18); effects show the tier total and its added benefit.",
    level: "Level", era: "Era", build: "Build", upgrade: "Upgrade", invested: "invested", thisLevel: "added this level", immediate: "Immediate reinforcements",
    route: "Land trade income requires a valid connected route.", railway: "Railway tracks appear at N6.", industrial: "Industrial City N6 unlocks SAM batteries and missile silos.", nav: "Building lines",
  },
  es: {
    roles: { City: "Capacidad de tropas y requisito de era para lanzar armas nucleares.", Tower: "Defensa de zona y resistencia a la captura.", TradingPost: "Ingresos de oro y una red comercial terrestre conectada.", Port: "Capacidad de flota, reparación y acceso marítimo.", MilitaryCamp: "Generación de tropas y conquista terrestre más rápida.", SAM: "Intercepción automática de armas nucleares entrantes.", MissileSilo: "Capacidad para lanzar armas atómicas, de hidrógeno y MIRV." },
    eras: ["Tribal", "Agraria", "Clásica", "Medieval", "Moderna temprana", "Industrial", "Contemporánea", "Futura"],
    stats: { reserve: "capacidad de tropas", troops: "tropas/s", reinforcements: "refuerzos", capture: "% de avance de conquista", trade: "oro/s con ruta terrestre activa", travel: "× frecuencia de entregas", range: "tiles de alcance", resistance: "resistencia a captura", fleet: "plazas de flota", docks: "atraques", repair: "reparación/s", charges: "cargas" },
    intro: "El nivel de Ciudad determina el nivel inicial de los edificios nuevos. La Ciudad Industrial N6 desbloquea las baterías SAM y los silos.",
    note: "Precios para un edificio: construcción en N1, costo de cada mejora y total invertido. Ingresos a velocidad Normal (0.18); cada efecto muestra el total y lo que añade el nivel.",
    level: "Nivel", era: "Era", build: "Construcción", upgrade: "Mejora", invested: "invertido", thisLevel: "añade este nivel", immediate: "Refuerzo inmediato",
    route: "El ingreso comercial terrestre requiere una ruta conectada válida.", railway: "Las vías de ferrocarril aparecen en N6.", industrial: "La Ciudad Industrial N6 desbloquea baterías SAM y silos.", nav: "Líneas de edificios",
  },
  ar: {
    roles: { City: "سعة القوات ومتطلب عصر إطلاق الأسلحة النووية.", Tower: "دفاع المنطقة ومقاومة الاستيلاء.", TradingPost: "دخل الذهب وشبكة تجارة برية متصلة.", Port: "سعة الأسطول وإصلاح السفن والوصول البحري.", MilitaryCamp: "إنتاج القوات وتسريع الاستيلاء البري.", SAM: "اعتراض تلقائي للأسلحة النووية القادمة.", MissileSilo: "إطلاق الأسلحة الذرية والهيدروجينية وMIRV." },
    eras: ["قبلية", "زراعية", "كلاسيكية", "العصور الوسطى", "حديثة مبكرة", "صناعية", "معاصرة", "مستقبلية"],
    stats: { reserve: "سعة القوات", troops: "قوات/ث", reinforcements: "تعزيزات", capture: "% سرعة الاستيلاء", trade: "ذهب/ث مع مسار بري نشط", travel: "× وتيرة التسليم التجاري", range: "مدى بالخانات", resistance: "مقاومة الاستيلاء", fleet: "سعة الأسطول", docks: "أرصفة", repair: "إصلاح/ث", charges: "شحنات" },
    intro: "يحدد مستوى المدينة المستوى الابتدائي للمباني الجديدة. تفتح المدينة الصناعية 6 بطاريات SAM وصوامع الصواريخ.",
    note: "الأسعار لمبنى واحد: الإنشاء عند المستوى 1، ثم تكلفة كل ترقية والإجمالي. الدخل بسرعة عادية (0.18)، ويعرض كل تأثير الإجمالي وما يضيفه المستوى.",
    level: "المستوى", era: "العصر", build: "إنشاء", upgrade: "ترقية", invested: "الإجمالي", thisLevel: "ما يضيفه المستوى", immediate: "تعزيز فوري",
    route: "دخل التجارة البرية يتطلب مساراً متصلاً صالحاً.", railway: "تظهر سكك الحديد عند المستوى 6.", industrial: "تفتح المدينة الصناعية 6 بطاريات SAM وصوامع الصواريخ.", nav: "خطوط المباني",
  },
  de: {
    roles: { City: "Truppenkapazität und Voraussetzung für das Atomzeitalter.", Tower: "Gebietsverteidigung und Eroberungswiderstand.", TradingPost: "Goldeinkommen und ein verbundenes Handelsnetz über Land.", Port: "Flottenkapazität, Schiffsreparatur und Zugang zum Meer.", MilitaryCamp: "Truppenproduktion und schnellere Eroberung an Land.", SAM: "Automatisches Abfangen eingehender Atomwaffen.", MissileSilo: "Startkapazität für Atom-, Wasserstoff- und MIRV-Waffen." },
    eras: ["Stammeszeit", "Agrarzeit", "Antike", "Mittelalter", "Frühe Neuzeit", "Industriezeitalter", "Gegenwart", "Zukunft"],
    stats: { reserve: "Truppenkapazität", troops: "Truppen/s", reinforcements: "Verstärkung", capture: "% Eroberungstempo", trade: "Gold/s bei aktiver Landroute", travel: "× Handelstransporte", range: "Reichweite in Feldern", resistance: "Eroberungswiderstand", fleet: "Flottenplätze", docks: "Liegeplätze", repair: "Reparatur/s", charges: "Ladungen" },
    intro: "Die Stadtstufe bestimmt die Startstufe neuer Gebäude. Eine Industriestadt auf Stufe 6 schaltet SAM-Batterien und Raketensilos frei.",
    note: "Preise für ein Gebäude: Bau auf Stufe 1, danach jeder Ausbau und die Gesamtsumme. Einkommen bei Normaltempo (0.18); Effekte zeigen Gesamtwert und Zuwachs.",
    level: "Stufe", era: "Zeitalter", build: "Bau", upgrade: "Ausbau", invested: "Gesamt", thisLevel: "Zuwachs dieser Stufe", immediate: "Sofortige Verstärkung",
    route: "Landhandelseinkommen braucht eine gültige, verbundene Route.", railway: "Ab Stufe 6 erscheinen Eisenbahngleise.", industrial: "Industriestadt Stufe 6 schaltet SAM-Batterien und Raketensilos frei.", nav: "Gebäudelinien",
  },
  fil: {
    roles: { City: "Kapasidad ng tropa at kailangang panahon para sa mga sandatang nuklear.", Tower: "Depensa ng lugar at panlaban sa pagsakop.", TradingPost: "Kita sa ginto at konektadong rutang pangkalakalan sa lupa.", Port: "Kapasidad ng plota, pagkukumpuni ng barko at daan sa dagat.", MilitaryCamp: "Produksyon ng tropa at mas mabilis na pagsakop sa lupa.", SAM: "Awtomatikong pagsabat sa paparating na sandatang nuklear.", MissileSilo: "Kakayahang maglunsad ng atomic, hydrogen at MIRV." },
    eras: ["Pantribo", "Pansakahan", "Klasiko", "Gitnang Panahon", "Maagang Makabago", "Industriyal", "Kasalukuyan", "Hinaharap"],
    stats: { reserve: "kapasidad ng tropa", troops: "tropa/s", reinforcements: "dagdag na tropa", capture: "% bilis ng pagsakop", trade: "ginto/s sa aktibong rutang lupa", travel: "× dalas ng kalakalan", range: "layo sa tile", resistance: "panlaban sa pagsakop", fleet: "puwesto ng plota", docks: "dungguan", repair: "ayos/s", charges: "karga" },
    intro: "Tinutukoy ng antas ng Lungsod ang panimulang antas ng mga bagong gusali. Binubuksan ng Lungsod Industriyal antas 6 ang mga SAM at silo ng misil.",
    note: "Presyo para sa isang gusali: paggawa sa antas 1, bawat upgrade at kabuuang nagastos. Kita sa Normal na bilis (0.18); ipinapakita ang kabuuan at dagdag ng antas.",
    level: "Antas", era: "Panahon", build: "Gawa", upgrade: "Upgrade", invested: "kabuuan", thisLevel: "dagdag sa antas na ito", immediate: "Agarang dagdag na tropa",
    route: "Kailangan ng wastong konektadong ruta para sa kalakalang panlupa.", railway: "Lilitaw ang riles sa antas 6.", industrial: "Binubuksan ng Lungsod Industriyal antas 6 ang mga SAM at silo.", nav: "Mga linya ng gusali",
  },
  fr: {
    roles: { City: "Capacité des troupes et condition d’accès à l’ère nucléaire.", Tower: "Défense de zone et résistance à la capture.", TradingPost: "Revenus en or et réseau commercial terrestre connecté.", Port: "Capacité de flotte, réparation et accès maritime.", MilitaryCamp: "Production de troupes et conquête terrestre accélérée.", SAM: "Interception automatique des armes nucléaires entrantes.", MissileSilo: "Capacité de lancement atomique, hydrogène et MIRV." },
    eras: ["Tribal", "Agraire", "Classique", "Médiévale", "Moderne ancienne", "Industrielle", "Contemporaine", "Futuriste"],
    stats: { reserve: "capacité des troupes", troops: "troupes/s", reinforcements: "renforts", capture: "% vitesse de conquête", trade: "or/s avec route terrestre active", travel: "× fréquence des livraisons", range: "portée en cases", resistance: "résistance à la capture", fleet: "places de flotte", docks: "places à quai", repair: "réparation/s", charges: "charges" },
    intro: "Le niveau de la ville fixe le niveau de départ des nouveaux bâtiments. La ville industrielle N6 débloque les batteries SAM et les silos.",
    note: "Prix pour un bâtiment : construction au N1, coût de chaque amélioration et total investi. Revenus à vitesse normale (0.18) ; les effets montrent le total et l’apport du niveau.",
    level: "Niveau", era: "Ère", build: "Construction", upgrade: "Amélioration", invested: "investi", thisLevel: "apport de ce niveau", immediate: "Renforts immédiats",
    route: "Le revenu commercial terrestre exige une route valide et connectée.", railway: "Les voies ferrées apparaissent au N6.", industrial: "La ville industrielle N6 débloque les batteries SAM et les silos.", nav: "Lignes de bâtiments",
  },
  id: {
    roles: { City: "Kapasitas pasukan dan syarat era peluncuran nuklir.", Tower: "Pertahanan area dan ketahanan terhadap perebutan.", TradingPost: "Pendapatan emas dan jaringan dagang darat yang terhubung.", Port: "Kapasitas armada, perbaikan kapal, dan akses maritim.", MilitaryCamp: "Produksi pasukan dan perebutan darat yang lebih cepat.", SAM: "Pencegatan otomatis senjata nuklir yang datang.", MissileSilo: "Kapasitas peluncuran atom, hidrogen, dan MIRV." },
    eras: ["Kesukuan", "Agraris", "Klasik", "Abad Pertengahan", "Modern Awal", "Industri", "Kontemporer", "Masa Depan"],
    stats: { reserve: "kapasitas pasukan", troops: "pasukan/dtk", reinforcements: "bala bantuan", capture: "% kecepatan merebut", trade: "emas/dtk dengan rute darat aktif", travel: "× frekuensi pengiriman", range: "jangkauan petak", resistance: "ketahanan perebutan", fleet: "slot armada", docks: "dermaga", repair: "perbaikan/dtk", charges: "muatan" },
    intro: "Level Kota menentukan level awal bangunan baru. Kota Industri level 6 membuka Baterai SAM dan Silo Rudal.",
    note: "Harga untuk satu bangunan: pembangunan level 1, biaya tiap upgrade, dan total investasi. Pendapatan pada kecepatan Normal (0.18); efek menunjukkan total dan tambahannya.",
    level: "Level", era: "Era", build: "Bangun", upgrade: "Upgrade", invested: "total", thisLevel: "tambahan level ini", immediate: "Bala bantuan langsung",
    route: "Pendapatan dagang darat memerlukan rute terhubung yang valid.", railway: "Rel kereta muncul pada level 6.", industrial: "Kota Industri level 6 membuka Baterai SAM dan Silo Rudal.", nav: "Jalur bangunan",
  },
  it: {
    roles: { City: "Capacità delle truppe e requisito per l’era nucleare.", Tower: "Difesa dell’area e resistenza alla conquista.", TradingPost: "Entrate in oro e rete commerciale terrestre collegata.", Port: "Capacità della flotta, riparazione e accesso marittimo.", MilitaryCamp: "Produzione di truppe e conquista terrestre più rapida.", SAM: "Intercettazione automatica delle armi nucleari in arrivo.", MissileSilo: "Capacità di lancio per armi atomiche, a idrogeno e MIRV." },
    eras: ["Tribale", "Agraria", "Classica", "Medievale", "Prima età moderna", "Industriale", "Contemporanea", "Futura"],
    stats: { reserve: "capacità truppe", troops: "truppe/s", reinforcements: "rinforzi", capture: "% velocità conquista", trade: "oro/s con rotta terrestre attiva", travel: "× frequenza consegne", range: "raggio in caselle", resistance: "resistenza alla conquista", fleet: "posti flotta", docks: "ormeggi", repair: "riparazione/s", charges: "cariche" },
    intro: "Il livello della Città determina il livello iniziale dei nuovi edifici. La Città industriale N6 sblocca batterie SAM e silos missilistici.",
    note: "Prezzi per un edificio: costruzione al livello 1, costo di ogni upgrade e totale investito. Entrate a velocità Normale (0.18); gli effetti mostrano totale e incremento.",
    level: "Livello", era: "Era", build: "Costruzione", upgrade: "Upgrade", invested: "investito", thisLevel: "incremento del livello", immediate: "Rinforzi immediati",
    route: "Le entrate commerciali via terra richiedono una rotta valida e collegata.", railway: "I binari ferroviari appaiono al livello 6.", industrial: "La Città industriale N6 sblocca batterie SAM e silos.", nav: "Linee di edifici",
  },
  ja: {
    roles: { City: "兵力上限と核兵器時代の条件。", Tower: "地域防衛と占領への抵抗。", TradingPost: "金収入と接続された陸上交易網。", Port: "艦隊容量、修理、海上アクセス。", MilitaryCamp: "兵力生産と陸上占領の加速。", SAM: "飛来する核兵器を自動迎撃。", MissileSilo: "原子・水素・MIRV兵器の発射能力。" },
    eras: ["部族", "農耕", "古典", "中世", "近世", "産業", "現代", "未来"],
    stats: { reserve: "兵力上限", troops: "兵/s", reinforcements: "即時増援", capture: "%占領速度", trade: "陸路接続中の金/s", travel: "×交易配送頻度", range: "射程マス", resistance: "占領耐性", fleet: "艦隊枠", docks: "停泊枠", repair: "修理/s", charges: "弾数" },
    intro: "都市レベルで新しい建物の初期レベルが決まります。工業都市N6でSAM砲台とミサイルサイロが解放されます。",
    note: "価格は建物1つ分です。N1の建設費、各強化費、累計額を表示します。収入は通常速度(0.18)、効果は累計とそのレベルの増加分です。",
    level: "レベル", era: "時代", build: "建設", upgrade: "強化", invested: "累計", thisLevel: "このレベルの増加", immediate: "即時増援",
    route: "陸上交易収入には有効な接続ルートが必要です。", railway: "N6で線路が表示されます。", industrial: "工業都市N6でSAM砲台とサイロが解放されます。", nav: "建物ライン",
  },
  ko: {
    roles: { City: "병력 수용량과 핵무기 시대 진입 조건.", Tower: "지역 방어와 점령 저항.", TradingPost: "골드 수입과 연결된 육상 교역망.", Port: "함대 수용량, 함선 수리, 해상 진출.", MilitaryCamp: "병력 생산과 육상 점령 가속.", SAM: "날아오는 핵무기를 자동 요격.", MissileSilo: "원자·수소·MIRV 무기 발사 능력." },
    eras: ["부족", "농경", "고전", "중세", "근세", "산업", "현대", "미래"],
    stats: { reserve: "병력 수용량", troops: "병력/초", reinforcements: "즉시 증원", capture: "% 점령 속도", trade: "육상 경로 연결 시 골드/초", travel: "× 교역 배송 빈도", range: "타일 사거리", resistance: "점령 저항", fleet: "함대 슬롯", docks: "정박지", repair: "수리/초", charges: "탄약" },
    intro: "도시 레벨이 새 건물의 시작 레벨을 결정합니다. 산업 도시 6레벨에서 SAM 포대와 미사일 사일로가 해금됩니다.",
    note: "가격은 건물 1개 기준입니다. 1레벨 건설비, 각 업그레이드 비용과 누적 비용을 표시합니다. 수입은 일반 속도(0.18), 효과는 총량과 해당 레벨의 증가량입니다.",
    level: "레벨", era: "시대", build: "건설", upgrade: "업그레이드", invested: "누적", thisLevel: "이번 레벨 증가량", immediate: "즉시 증원",
    route: "육상 교역 수입에는 유효한 연결 경로가 필요합니다.", railway: "6레벨부터 철로가 표시됩니다.", industrial: "산업 도시 6레벨에서 SAM 포대와 사일로가 해금됩니다.", nav: "건물 계열",
  },
  "pt-br": {
    roles: { City: "Capacidade de tropas e requisito da era nuclear.", Tower: "Defesa territorial e resistência à conquista.", TradingPost: "Renda de ouro e rede comercial terrestre conectada.", Port: "Capacidade da frota, reparo de navios e acesso marítimo.", MilitaryCamp: "Produção de tropas e conquista terrestre mais rápida.", SAM: "Interceptação automática de armas nucleares que chegam.", MissileSilo: "Capacidade para lançar armas atômicas, de hidrogênio e MIRV." },
    eras: ["Tribal", "Agrária", "Clássica", "Medieval", "Moderna inicial", "Industrial", "Contemporânea", "Futura"],
    stats: { reserve: "capacidade de tropas", troops: "tropas/s", reinforcements: "reforços", capture: "% velocidade de conquista", trade: "ouro/s com rota terrestre ativa", travel: "× frequência de entregas", range: "alcance em casas", resistance: "resistência à conquista", fleet: "vagas de frota", docks: "vagas de atracação", repair: "reparo/s", charges: "cargas" },
    intro: "O nível da Cidade define o nível inicial de novos edifícios. Cidade Industrial N6 libera baterias SAM e silos de mísseis.",
    note: "Preços para um edifício: construção no N1, custo de cada melhoria e total investido. Renda na velocidade Normal (0.18); efeitos mostram o total e o acréscimo do nível.",
    level: "Nível", era: "Era", build: "Construção", upgrade: "Melhoria", invested: "investido", thisLevel: "acréscimo deste nível", immediate: "Reforços imediatos",
    route: "A renda comercial terrestre exige uma rota válida e conectada.", railway: "Os trilhos aparecem no N6.", industrial: "Cidade Industrial N6 libera baterias SAM e silos.", nav: "Linhas de edifícios",
  },
  ru: {
    roles: { City: "Вместимость войск и требование ядерной эпохи.", Tower: "Оборона территории и сопротивление захвату.", TradingPost: "Доход золота и связанная сухопутная торговая сеть.", Port: "Вместимость флота, ремонт кораблей и выход к морю.", MilitaryCamp: "Производство войск и ускорение сухопутного захвата.", SAM: "Автоматический перехват входящего ядерного оружия.", MissileSilo: "Запуск атомных, водородных ракет и MIRV." },
    eras: ["Племенная", "Аграрная", "Классическая", "Средневековая", "Раннее Новое время", "Индустриальная", "Современная", "Будущая"],
    stats: { reserve: "вместимость войск", troops: "войск/с", reinforcements: "подкрепления", capture: "% скорости захвата", trade: "золота/с при активном сухопутном пути", travel: "× частота торговых доставок", range: "дальность в клетках", resistance: "сопротивление захвату", fleet: "места флота", docks: "места в доках", repair: "ремонт/с", charges: "заряды" },
    intro: "Уровень Города задаёт начальный уровень новых зданий. Индустриальный город 6-го уровня открывает батареи SAM и ракетные шахты.",
    note: "Цены для одного здания: постройка уровня 1, стоимость каждого улучшения и общая сумма. Доход при обычной скорости (0.18); эффекты показывают итог и прирост уровня.",
    level: "Уровень", era: "Эпоха", build: "Постройка", upgrade: "Улучшение", invested: "всего", thisLevel: "прирост уровня", immediate: "Мгновенное подкрепление",
    route: "Для сухопутного торгового дохода нужен действующий связанный маршрут.", railway: "Железнодорожные пути появляются на уровне 6.", industrial: "Индустриальный город 6-го уровня открывает SAM и ракетные шахты.", nav: "Линии зданий",
  },
  tr: {
    roles: { City: "Asker kapasitesi ve nükleer çağ gereksinimi.", Tower: "Bölge savunması ve ele geçirilmeye direnç.", TradingPost: "Altın geliri ve bağlantılı kara ticaret ağı.", Port: "Filo kapasitesi, gemi onarımı ve deniz erişimi.", MilitaryCamp: "Asker üretimi ve daha hızlı kara fethi.", SAM: "Gelen nükleer silahları otomatik engeller.", MissileSilo: "Atom, hidrojen ve MIRV silahlarını fırlatma kapasitesi." },
    eras: ["Kabile", "Tarım", "Klasik", "Orta Çağ", "Erken Modern", "Sanayi", "Çağdaş", "Gelecek"],
    stats: { reserve: "asker kapasitesi", troops: "asker/sn", reinforcements: "takviye", capture: "% fetih hızı", trade: "etkin kara rotasında altın/sn", travel: "× ticaret teslimat sıklığı", range: "kare menzili", resistance: "ele geçirilme direnci", fleet: "filo yuvası", docks: "rıhtım", repair: "onarım/sn", charges: "yük" },
    intro: "Şehir seviyesi yeni binaların başlangıç seviyesini belirler. 6. seviye Sanayi Şehri SAM bataryalarını ve füze silolarını açar.",
    note: "Fiyatlar tek bina içindir: 1. seviye inşaatı, her yükseltme ve toplam yatırım. Gelir Normal hızda (0.18); etkiler toplamı ve seviye artışını gösterir.",
    level: "Seviye", era: "Çağ", build: "İnşa", upgrade: "Yükseltme", invested: "toplam", thisLevel: "bu seviyenin artışı", immediate: "Anında takviye",
    route: "Kara ticaret geliri için geçerli ve bağlantılı bir rota gerekir.", railway: "Demiryolu rayları 6. seviyede görünür.", industrial: "6. seviye Sanayi Şehri SAM ve füze silolarını açar.", nav: "Bina hatları",
  },
  vi: {
    roles: { City: "Sức chứa quân và điều kiện mở thời đại hạt nhân.", Tower: "Phòng thủ lãnh thổ và kháng chiếm đóng.", TradingPost: "Thu nhập vàng và mạng lưới thương mại đường bộ kết nối.", Port: "Sức chứa hạm đội, sửa tàu và tiếp cận biển.", MilitaryCamp: "Sản xuất quân và chiếm đất nhanh hơn.", SAM: "Tự động đánh chặn vũ khí hạt nhân đang tới.", MissileSilo: "Khả năng phóng vũ khí nguyên tử, hydro và MIRV." },
    eras: ["Bộ lạc", "Nông nghiệp", "Cổ điển", "Trung cổ", "Cận đại", "Công nghiệp", "Đương đại", "Tương lai"],
    stats: { reserve: "sức chứa quân", troops: "quân/giây", reinforcements: "quân tiếp viện", capture: "% tốc độ chiếm đất", trade: "vàng/giây khi có tuyến bộ hoạt động", travel: "× tần suất giao hàng", range: "tầm theo ô", resistance: "kháng chiếm đóng", fleet: "chỗ hạm đội", docks: "chỗ neo đậu", repair: "sửa/giây", charges: "lượt" },
    intro: "Cấp Thành phố quyết định cấp khởi đầu của công trình mới. Thành phố Công nghiệp cấp 6 mở khóa trận địa SAM và hầm phóng tên lửa.",
    note: "Giá cho một công trình: xây cấp 1, chi phí từng lần nâng cấp và tổng đã đầu tư. Thu nhập ở tốc độ Normal (0.18); hiệu ứng gồm tổng và phần tăng thêm.",
    level: "Cấp", era: "Thời đại", build: "Xây", upgrade: "Nâng cấp", invested: "tổng đầu tư", thisLevel: "tăng ở cấp này", immediate: "Quân tiếp viện tức thì",
    route: "Thu nhập thương mại đường bộ cần tuyến kết nối hợp lệ.", railway: "Đường ray xuất hiện từ cấp 6.", industrial: "Thành phố Công nghiệp cấp 6 mở khóa SAM và hầm phóng tên lửa.", nav: "Các dòng công trình",
  },
  "zh-cn": {
    roles: { City: "兵力容量及核武器时代的前置要求。", Tower: "区域防御与抗占领能力。", TradingPost: "金币收入及已连接的陆上贸易网络。", Port: "舰队容量、舰船维修和海上通道。", MilitaryCamp: "兵力生产并加快陆地占领。", SAM: "自动拦截来袭核武器。", MissileSilo: "发射原子弹、氢弹和MIRV的能力。" },
    eras: ["部落", "农耕", "古典", "中世纪", "近代早期", "工业", "当代", "未来"],
    stats: { reserve: "兵力容量", troops: "兵力/秒", reinforcements: "即时增援", capture: "%占领速度", trade: "陆路连接生效时的金币/秒", travel: "×贸易交付频率", range: "射程格数", resistance: "抗占领权重", fleet: "舰队位", docks: "停泊位", repair: "维修/秒", charges: "弹药" },
    intro: "城市等级决定新建筑的起始等级。工业城市达到6级后解锁SAM防空炮台和导弹发射井。",
    note: "价格按单个建筑计算：1级建造、每级升级和累计投入。收入按普通速度（0.18）；效果显示总值及本级增量。",
    level: "等级", era: "时代", build: "建造", upgrade: "升级", invested: "累计投入", thisLevel: "本级增加", immediate: "即时增援",
    route: "陆上贸易收入需要有效且已连接的路线。", railway: "6级开始显示铁路轨道。", industrial: "工业城市6级解锁SAM和导弹发射井。", nav: "建筑线路",
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
  const language = localeCode(locale) === "es" ? "es" : "en";
  const localized = pageLocales[localeCode(locale)] ?? pageLocales.en;
  return lines[kind]?.[language].map((name, index) => ({
    name,
    era: localized.eras[lines[kind].en.length === 4 ? Math.min(7, index + 5) : index],
  })) ?? [];
}

export function buildingRole(kind: string, locale: string) {
  const code = localeCode(locale);
  return (pageLocales[code] ?? pageLocales.en).roles[kind as BuildingKind] ?? "";
}

export function buildingPageText(locale: string): PageLocale {
  return pageLocales[localeCode(locale)] ?? pageLocales.en;
}
