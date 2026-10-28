const fn hex_to_rgb(hex: u32) -> [f32; 3] {
    let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let b = (hex & 0xFF) as f32 / 255.0;
    [r, g, b]
}

pub struct NamedColor {
    pub name: &'static str,
    pub rgb: [f32; 3],
}

// Maximin ΔEOK2 palette; Nation saturation, brightness, and rendered colors are bounded.
// Catalog Nations use the first 202 slots; breakaways select free slots against active colors.
pub static PREMIUM_COLORS: [NamedColor; 1024] = [
    NamedColor {
        name: "Nation slot 0001",
        rgb: hex_to_rgb(0xF4FFD8),
    },
    NamedColor {
        name: "Nation slot 0002",
        rgb: hex_to_rgb(0xFCA0F4),
    },
    NamedColor {
        name: "Nation slot 0003",
        rgb: hex_to_rgb(0xA09C38),
    },
    NamedColor {
        name: "Nation slot 0004",
        rgb: hex_to_rgb(0x484000),
    },
    NamedColor {
        name: "Nation slot 0005",
        rgb: hex_to_rgb(0xC45494),
    },
    NamedColor {
        name: "Nation slot 0006",
        rgb: hex_to_rgb(0xE8906C),
    },
    NamedColor {
        name: "Nation slot 0007",
        rgb: hex_to_rgb(0x946CE0),
    },
    NamedColor {
        name: "Nation slot 0008",
        rgb: hex_to_rgb(0x78FC9C),
    },
    NamedColor {
        name: "Nation slot 0009",
        rgb: hex_to_rgb(0x70A8C0),
    },
    NamedColor {
        name: "Nation slot 0010",
        rgb: hex_to_rgb(0x9C3C54),
    },
    NamedColor {
        name: "Nation slot 0011",
        rgb: hex_to_rgb(0x5C6800),
    },
    NamedColor {
        name: "Nation slot 0012",
        rgb: hex_to_rgb(0xF4646C),
    },
    NamedColor {
        name: "Nation slot 0013",
        rgb: hex_to_rgb(0x5CA074),
    },
    NamedColor {
        name: "Nation slot 0014",
        rgb: hex_to_rgb(0x5C54B4),
    },
    NamedColor {
        name: "Nation slot 0015",
        rgb: hex_to_rgb(0xFFFF54),
    },
    NamedColor {
        name: "Nation slot 0016",
        rgb: hex_to_rgb(0x44FFF8),
    },
    NamedColor {
        name: "Nation slot 0017",
        rgb: hex_to_rgb(0x4C7850),
    },
    NamedColor {
        name: "Nation slot 0018",
        rgb: hex_to_rgb(0x7C9CF8),
    },
    NamedColor {
        name: "Nation slot 0019",
        rgb: hex_to_rgb(0x808400),
    },
    NamedColor {
        name: "Nation slot 0020",
        rgb: hex_to_rgb(0x244C40),
    },
    NamedColor {
        name: "Nation slot 0021",
        rgb: hex_to_rgb(0xC05448),
    },
    NamedColor {
        name: "Nation slot 0022",
        rgb: hex_to_rgb(0xB090A0),
    },
    NamedColor {
        name: "Nation slot 0023",
        rgb: hex_to_rgb(0x68542C),
    },
    NamedColor {
        name: "Nation slot 0024",
        rgb: hex_to_rgb(0x683020),
    },
    NamedColor {
        name: "Nation slot 0025",
        rgb: hex_to_rgb(0x904894),
    },
    NamedColor {
        name: "Nation slot 0026",
        rgb: hex_to_rgb(0xA474AC),
    },
    NamedColor {
        name: "Nation slot 0027",
        rgb: hex_to_rgb(0x5CAC40),
    },
    NamedColor {
        name: "Nation slot 0028",
        rgb: hex_to_rgb(0x005400),
    },
    NamedColor {
        name: "Nation slot 0029",
        rgb: hex_to_rgb(0xE49830),
    },
    NamedColor {
        name: "Nation slot 0030",
        rgb: hex_to_rgb(0x848444),
    },
    NamedColor {
        name: "Nation slot 0031",
        rgb: hex_to_rgb(0xFC70B0),
    },
    NamedColor {
        name: "Nation slot 0032",
        rgb: hex_to_rgb(0xA0A078),
    },
    NamedColor {
        name: "Nation slot 0033",
        rgb: hex_to_rgb(0xFC743C),
    },
    NamedColor {
        name: "Nation slot 0034",
        rgb: hex_to_rgb(0x7C284C),
    },
    NamedColor {
        name: "Nation slot 0035",
        rgb: hex_to_rgb(0x349CBC),
    },
    NamedColor {
        name: "Nation slot 0036",
        rgb: hex_to_rgb(0xB88008),
    },
    NamedColor {
        name: "Nation slot 0037",
        rgb: hex_to_rgb(0xECD0E8),
    },
    NamedColor {
        name: "Nation slot 0038",
        rgb: hex_to_rgb(0xAC3420),
    },
    NamedColor {
        name: "Nation slot 0039",
        rgb: hex_to_rgb(0x5C70D0),
    },
    NamedColor {
        name: "Nation slot 0040",
        rgb: hex_to_rgb(0x74B89C),
    },
    NamedColor {
        name: "Nation slot 0041",
        rgb: hex_to_rgb(0xCC6CCC),
    },
    NamedColor {
        name: "Nation slot 0042",
        rgb: hex_to_rgb(0x408C84),
    },
    NamedColor {
        name: "Nation slot 0043",
        rgb: hex_to_rgb(0xC87484),
    },
    NamedColor {
        name: "Nation slot 0044",
        rgb: hex_to_rgb(0x8C544C),
    },
    NamedColor {
        name: "Nation slot 0045",
        rgb: hex_to_rgb(0x24B894),
    },
    NamedColor {
        name: "Nation slot 0046",
        rgb: hex_to_rgb(0x306458),
    },
    NamedColor {
        name: "Nation slot 0047",
        rgb: hex_to_rgb(0xCCB060),
    },
    NamedColor {
        name: "Nation slot 0048",
        rgb: hex_to_rgb(0x006884),
    },
    NamedColor {
        name: "Nation slot 0049",
        rgb: hex_to_rgb(0x30581C),
    },
    NamedColor {
        name: "Nation slot 0050",
        rgb: hex_to_rgb(0x70387C),
    },
    NamedColor {
        name: "Nation slot 0051",
        rgb: hex_to_rgb(0xB88044),
    },
    NamedColor {
        name: "Nation slot 0052",
        rgb: hex_to_rgb(0xA894D8),
    },
    NamedColor {
        name: "Nation slot 0053",
        rgb: hex_to_rgb(0x88BC40),
    },
    NamedColor {
        name: "Nation slot 0054",
        rgb: hex_to_rgb(0xA06080),
    },
    NamedColor {
        name: "Nation slot 0055",
        rgb: hex_to_rgb(0x3048B0),
    },
    NamedColor {
        name: "Nation slot 0056",
        rgb: hex_to_rgb(0xC8F894),
    },
    NamedColor {
        name: "Nation slot 0057",
        rgb: hex_to_rgb(0x6C4444),
    },
    NamedColor {
        name: "Nation slot 0058",
        rgb: hex_to_rgb(0xC84464),
    },
    NamedColor {
        name: "Nation slot 0059",
        rgb: hex_to_rgb(0x38A0F8),
    },
    NamedColor {
        name: "Nation slot 0060",
        rgb: hex_to_rgb(0x8464B0),
    },
    NamedColor {
        name: "Nation slot 0061",
        rgb: hex_to_rgb(0x187C84),
    },
    NamedColor {
        name: "Nation slot 0062",
        rgb: hex_to_rgb(0x286800),
    },
    NamedColor {
        name: "Nation slot 0063",
        rgb: hex_to_rgb(0xAC58BC),
    },
    NamedColor {
        name: "Nation slot 0064",
        rgb: hex_to_rgb(0x2C7C28),
    },
    NamedColor {
        name: "Nation slot 0065",
        rgb: hex_to_rgb(0xA43478),
    },
    NamedColor {
        name: "Nation slot 0066",
        rgb: hex_to_rgb(0x38C4E4),
    },
    NamedColor {
        name: "Nation slot 0067",
        rgb: hex_to_rgb(0x2474D4),
    },
    NamedColor {
        name: "Nation slot 0068",
        rgb: hex_to_rgb(0x685088),
    },
    NamedColor {
        name: "Nation slot 0069",
        rgb: hex_to_rgb(0xF0BCA0),
    },
    NamedColor {
        name: "Nation slot 0070",
        rgb: hex_to_rgb(0x789494),
    },
    NamedColor {
        name: "Nation slot 0071",
        rgb: hex_to_rgb(0xBC4818),
    },
    NamedColor {
        name: "Nation slot 0072",
        rgb: hex_to_rgb(0x704008),
    },
    NamedColor {
        name: "Nation slot 0073",
        rgb: hex_to_rgb(0x746848),
    },
    NamedColor {
        name: "Nation slot 0074",
        rgb: hex_to_rgb(0xC46C50),
    },
    NamedColor {
        name: "Nation slot 0075",
        rgb: hex_to_rgb(0xC4F4FF),
    },
    NamedColor {
        name: "Nation slot 0076",
        rgb: hex_to_rgb(0x609440),
    },
    NamedColor {
        name: "Nation slot 0077",
        rgb: hex_to_rgb(0x3C3094),
    },
    NamedColor {
        name: "Nation slot 0078",
        rgb: hex_to_rgb(0x647C24),
    },
    NamedColor {
        name: "Nation slot 0079",
        rgb: hex_to_rgb(0xFCC824),
    },
    NamedColor {
        name: "Nation slot 0080",
        rgb: hex_to_rgb(0x8488B4),
    },
    NamedColor {
        name: "Nation slot 0081",
        rgb: hex_to_rgb(0x80AC64),
    },
    NamedColor {
        name: "Nation slot 0082",
        rgb: hex_to_rgb(0xA02440),
    },
    NamedColor {
        name: "Nation slot 0083",
        rgb: hex_to_rgb(0xEC94B0),
    },
    NamedColor {
        name: "Nation slot 0084",
        rgb: hex_to_rgb(0x2CAC64),
    },
    NamedColor {
        name: "Nation slot 0085",
        rgb: hex_to_rgb(0xB080E4),
    },
    NamedColor {
        name: "Nation slot 0086",
        rgb: hex_to_rgb(0x5C80BC),
    },
    NamedColor {
        name: "Nation slot 0087",
        rgb: hex_to_rgb(0x5C5404),
    },
    NamedColor {
        name: "Nation slot 0088",
        rgb: hex_to_rgb(0x045028),
    },
    NamedColor {
        name: "Nation slot 0089",
        rgb: hex_to_rgb(0x3C5468),
    },
    NamedColor {
        name: "Nation slot 0090",
        rgb: hex_to_rgb(0x48A8A4),
    },
    NamedColor {
        name: "Nation slot 0091",
        rgb: hex_to_rgb(0x208C64),
    },
    NamedColor {
        name: "Nation slot 0092",
        rgb: hex_to_rgb(0x544098),
    },
    NamedColor {
        name: "Nation slot 0093",
        rgb: hex_to_rgb(0xA88874),
    },
    NamedColor {
        name: "Nation slot 0094",
        rgb: hex_to_rgb(0x005464),
    },
    NamedColor {
        name: "Nation slot 0095",
        rgb: hex_to_rgb(0x883C24),
    },
    NamedColor {
        name: "Nation slot 0096",
        rgb: hex_to_rgb(0x8450BC),
    },
    NamedColor {
        name: "Nation slot 0097",
        rgb: hex_to_rgb(0x846C04),
    },
    NamedColor {
        name: "Nation slot 0098",
        rgb: hex_to_rgb(0x3C589C),
    },
    NamedColor {
        name: "Nation slot 0099",
        rgb: hex_to_rgb(0xCC5C74),
    },
    NamedColor {
        name: "Nation slot 0100",
        rgb: hex_to_rgb(0x885070),
    },
    NamedColor {
        name: "Nation slot 0101",
        rgb: hex_to_rgb(0x00B8BC),
    },
    NamedColor {
        name: "Nation slot 0102",
        rgb: hex_to_rgb(0x48682C),
    },
    NamedColor {
        name: "Nation slot 0103",
        rgb: hex_to_rgb(0x809C1C),
    },
    NamedColor {
        name: "Nation slot 0104",
        rgb: hex_to_rgb(0x64B8FC),
    },
    NamedColor {
        name: "Nation slot 0105",
        rgb: hex_to_rgb(0x985020),
    },
    NamedColor {
        name: "Nation slot 0106",
        rgb: hex_to_rgb(0xB4BCE8),
    },
    NamedColor {
        name: "Nation slot 0107",
        rgb: hex_to_rgb(0xD47420),
    },
    NamedColor {
        name: "Nation slot 0108",
        rgb: hex_to_rgb(0x4C4828),
    },
    NamedColor {
        name: "Nation slot 0109",
        rgb: hex_to_rgb(0x345CC4),
    },
    NamedColor {
        name: "Nation slot 0110",
        rgb: hex_to_rgb(0x04409C),
    },
    NamedColor {
        name: "Nation slot 0111",
        rgb: hex_to_rgb(0x30ECAC),
    },
    NamedColor {
        name: "Nation slot 0112",
        rgb: hex_to_rgb(0xC49458),
    },
    NamedColor {
        name: "Nation slot 0113",
        rgb: hex_to_rgb(0xD85C30),
    },
    NamedColor {
        name: "Nation slot 0114",
        rgb: hex_to_rgb(0xEC7868),
    },
    NamedColor {
        name: "Nation slot 0115",
        rgb: hex_to_rgb(0x587470),
    },
    NamedColor {
        name: "Nation slot 0116",
        rgb: hex_to_rgb(0x747CEC),
    },
    NamedColor {
        name: "Nation slot 0117",
        rgb: hex_to_rgb(0xAC487C),
    },
    NamedColor {
        name: "Nation slot 0118",
        rgb: hex_to_rgb(0x688868),
    },
    NamedColor {
        name: "Nation slot 0119",
        rgb: hex_to_rgb(0x0474A8),
    },
    NamedColor {
        name: "Nation slot 0120",
        rgb: hex_to_rgb(0xCC74AC),
    },
    NamedColor {
        name: "Nation slot 0121",
        rgb: hex_to_rgb(0x0090AC),
    },
    NamedColor {
        name: "Nation slot 0122",
        rgb: hex_to_rgb(0x847098),
    },
    NamedColor {
        name: "Nation slot 0123",
        rgb: hex_to_rgb(0xBC4040),
    },
    NamedColor {
        name: "Nation slot 0124",
        rgb: hex_to_rgb(0xA46044),
    },
    NamedColor {
        name: "Nation slot 0125",
        rgb: hex_to_rgb(0x706420),
    },
    NamedColor {
        name: "Nation slot 0126",
        rgb: hex_to_rgb(0xC89CCC),
    },
    NamedColor {
        name: "Nation slot 0127",
        rgb: hex_to_rgb(0x28A07C),
    },
    NamedColor {
        name: "Nation slot 0128",
        rgb: hex_to_rgb(0x8478C4),
    },
    NamedColor {
        name: "Nation slot 0129",
        rgb: hex_to_rgb(0x74E4F0),
    },
    NamedColor {
        name: "Nation slot 0130",
        rgb: hex_to_rgb(0x783C5C),
    },
    NamedColor {
        name: "Nation slot 0131",
        rgb: hex_to_rgb(0xA07824),
    },
    NamedColor {
        name: "Nation slot 0132",
        rgb: hex_to_rgb(0xA85060),
    },
    NamedColor {
        name: "Nation slot 0133",
        rgb: hex_to_rgb(0x4888A4),
    },
    NamedColor {
        name: "Nation slot 0134",
        rgb: hex_to_rgb(0x7840A4),
    },
    NamedColor {
        name: "Nation slot 0135",
        rgb: hex_to_rgb(0x507C04),
    },
    NamedColor {
        name: "Nation slot 0136",
        rgb: hex_to_rgb(0x882C30),
    },
    NamedColor {
        name: "Nation slot 0137",
        rgb: hex_to_rgb(0xA4B04C),
    },
    NamedColor {
        name: "Nation slot 0138",
        rgb: hex_to_rgb(0xA47C90),
    },
    NamedColor {
        name: "Nation slot 0139",
        rgb: hex_to_rgb(0x54B478),
    },
    NamedColor {
        name: "Nation slot 0140",
        rgb: hex_to_rgb(0x6064A8),
    },
    NamedColor {
        name: "Nation slot 0141",
        rgb: hex_to_rgb(0x20785C),
    },
    NamedColor {
        name: "Nation slot 0142",
        rgb: hex_to_rgb(0x805008),
    },
    NamedColor {
        name: "Nation slot 0143",
        rgb: hex_to_rgb(0x849468),
    },
    NamedColor {
        name: "Nation slot 0144",
        rgb: hex_to_rgb(0x942460),
    },
    NamedColor {
        name: "Nation slot 0145",
        rgb: hex_to_rgb(0x304050),
    },
    NamedColor {
        name: "Nation slot 0146",
        rgb: hex_to_rgb(0x682868),
    },
    NamedColor {
        name: "Nation slot 0147",
        rgb: hex_to_rgb(0x908C24),
    },
    NamedColor {
        name: "Nation slot 0148",
        rgb: hex_to_rgb(0x643040),
    },
    NamedColor {
        name: "Nation slot 0149",
        rgb: hex_to_rgb(0x8CA8A0),
    },
    NamedColor {
        name: "Nation slot 0150",
        rgb: hex_to_rgb(0xB89410),
    },
    NamedColor {
        name: "Nation slot 0151",
        rgb: hex_to_rgb(0x543080),
    },
    NamedColor {
        name: "Nation slot 0152",
        rgb: hex_to_rgb(0xF89044),
    },
    NamedColor {
        name: "Nation slot 0153",
        rgb: hex_to_rgb(0x5084E4),
    },
    NamedColor {
        name: "Nation slot 0154",
        rgb: hex_to_rgb(0x782C00),
    },
    NamedColor {
        name: "Nation slot 0155",
        rgb: hex_to_rgb(0x9C2410),
    },
    NamedColor {
        name: "Nation slot 0156",
        rgb: hex_to_rgb(0x488848),
    },
    NamedColor {
        name: "Nation slot 0157",
        rgb: hex_to_rgb(0x089CD8),
    },
    NamedColor {
        name: "Nation slot 0158",
        rgb: hex_to_rgb(0x8CA8E8),
    },
    NamedColor {
        name: "Nation slot 0159",
        rgb: hex_to_rgb(0xA0589C),
    },
    NamedColor {
        name: "Nation slot 0160",
        rgb: hex_to_rgb(0x60E4B8),
    },
    NamedColor {
        name: "Nation slot 0161",
        rgb: hex_to_rgb(0xB4686C),
    },
    NamedColor {
        name: "Nation slot 0162",
        rgb: hex_to_rgb(0xB484B4),
    },
    NamedColor {
        name: "Nation slot 0163",
        rgb: hex_to_rgb(0x604C64),
    },
    NamedColor {
        name: "Nation slot 0164",
        rgb: hex_to_rgb(0xACA820),
    },
    NamedColor {
        name: "Nation slot 0165",
        rgb: hex_to_rgb(0x5C8C18),
    },
    NamedColor {
        name: "Nation slot 0166",
        rgb: hex_to_rgb(0xB864AC),
    },
    NamedColor {
        name: "Nation slot 0167",
        rgb: hex_to_rgb(0x907448),
    },
    NamedColor {
        name: "Nation slot 0168",
        rgb: hex_to_rgb(0x6494B4),
    },
    NamedColor {
        name: "Nation slot 0169",
        rgb: hex_to_rgb(0x284C00),
    },
    NamedColor {
        name: "Nation slot 0170",
        rgb: hex_to_rgb(0x185C40),
    },
    NamedColor {
        name: "Nation slot 0171",
        rgb: hex_to_rgb(0x7460D0),
    },
    NamedColor {
        name: "Nation slot 0172",
        rgb: hex_to_rgb(0x9C443C),
    },
    NamedColor {
        name: "Nation slot 0173",
        rgb: hex_to_rgb(0xF87894),
    },
    NamedColor {
        name: "Nation slot 0174",
        rgb: hex_to_rgb(0x8C6468),
    },
    NamedColor {
        name: "Nation slot 0175",
        rgb: hex_to_rgb(0xE05850),
    },
    NamedColor {
        name: "Nation slot 0176",
        rgb: hex_to_rgb(0xB88C30),
    },
    NamedColor {
        name: "Nation slot 0177",
        rgb: hex_to_rgb(0x0C8C44),
    },
    NamedColor {
        name: "Nation slot 0178",
        rgb: hex_to_rgb(0x8C98B0),
    },
    NamedColor {
        name: "Nation slot 0179",
        rgb: hex_to_rgb(0x3868B4),
    },
    NamedColor {
        name: "Nation slot 0180",
        rgb: hex_to_rgb(0x2C481C),
    },
    NamedColor {
        name: "Nation slot 0181",
        rgb: hex_to_rgb(0x583810),
    },
    NamedColor {
        name: "Nation slot 0182",
        rgb: hex_to_rgb(0x8C4074),
    },
    NamedColor {
        name: "Nation slot 0183",
        rgb: hex_to_rgb(0x106014),
    },
    NamedColor {
        name: "Nation slot 0184",
        rgb: hex_to_rgb(0x747438),
    },
    NamedColor {
        name: "Nation slot 0185",
        rgb: hex_to_rgb(0x54644C),
    },
    NamedColor {
        name: "Nation slot 0186",
        rgb: hex_to_rgb(0xC88884),
    },
    NamedColor {
        name: "Nation slot 0187",
        rgb: hex_to_rgb(0x006464),
    },
    NamedColor {
        name: "Nation slot 0188",
        rgb: hex_to_rgb(0x28BCF8),
    },
    NamedColor {
        name: "Nation slot 0189",
        rgb: hex_to_rgb(0x245C88),
    },
    NamedColor {
        name: "Nation slot 0190",
        rgb: hex_to_rgb(0x405838),
    },
    NamedColor {
        name: "Nation slot 0191",
        rgb: hex_to_rgb(0x7C5C90),
    },
    NamedColor {
        name: "Nation slot 0192",
        rgb: hex_to_rgb(0x78A044),
    },
    NamedColor {
        name: "Nation slot 0193",
        rgb: hex_to_rgb(0x484C80),
    },
    NamedColor {
        name: "Nation slot 0194",
        rgb: hex_to_rgb(0x94C094),
    },
    NamedColor {
        name: "Nation slot 0195",
        rgb: hex_to_rgb(0xB0B07C),
    },
    NamedColor {
        name: "Nation slot 0196",
        rgb: hex_to_rgb(0x40A040),
    },
    NamedColor {
        name: "Nation slot 0197",
        rgb: hex_to_rgb(0x987800),
    },
    NamedColor {
        name: "Nation slot 0198",
        rgb: hex_to_rgb(0x4CBC60),
    },
    NamedColor {
        name: "Nation slot 0199",
        rgb: hex_to_rgb(0xB0303C),
    },
    NamedColor {
        name: "Nation slot 0200",
        rgb: hex_to_rgb(0xBC6890),
    },
    NamedColor {
        name: "Nation slot 0201",
        rgb: hex_to_rgb(0x8C1848),
    },
    NamedColor {
        name: "Nation slot 0202",
        rgb: hex_to_rgb(0xE0A4AC),
    },
    NamedColor {
        name: "Nation slot 0203",
        rgb: hex_to_rgb(0xA49054),
    },
    NamedColor {
        name: "Nation slot 0204",
        rgb: hex_to_rgb(0xB43864),
    },
    NamedColor {
        name: "Nation slot 0205",
        rgb: hex_to_rgb(0x148C84),
    },
    NamedColor {
        name: "Nation slot 0206",
        rgb: hex_to_rgb(0x8C3488),
    },
    NamedColor {
        name: "Nation slot 0207",
        rgb: hex_to_rgb(0xAC6CCC),
    },
    NamedColor {
        name: "Nation slot 0208",
        rgb: hex_to_rgb(0x2C9C9C),
    },
    NamedColor {
        name: "Nation slot 0209",
        rgb: hex_to_rgb(0x007CC8),
    },
    NamedColor {
        name: "Nation slot 0210",
        rgb: hex_to_rgb(0xAC4C00),
    },
    NamedColor {
        name: "Nation slot 0211",
        rgb: hex_to_rgb(0xF07CD0),
    },
    NamedColor {
        name: "Nation slot 0212",
        rgb: hex_to_rgb(0x44600C),
    },
    NamedColor {
        name: "Nation slot 0213",
        rgb: hex_to_rgb(0x6CA490),
    },
    NamedColor {
        name: "Nation slot 0214",
        rgb: hex_to_rgb(0xBC582C),
    },
    NamedColor {
        name: "Nation slot 0215",
        rgb: hex_to_rgb(0x5460D0),
    },
    NamedColor {
        name: "Nation slot 0216",
        rgb: hex_to_rgb(0xAC489C),
    },
    NamedColor {
        name: "Nation slot 0217",
        rgb: hex_to_rgb(0x14C8B0),
    },
    NamedColor {
        name: "Nation slot 0218",
        rgb: hex_to_rgb(0x804C30),
    },
    NamedColor {
        name: "Nation slot 0219",
        rgb: hex_to_rgb(0x4078C4),
    },
    NamedColor {
        name: "Nation slot 0220",
        rgb: hex_to_rgb(0x3C8CC4),
    },
    NamedColor {
        name: "Nation slot 0221",
        rgb: hex_to_rgb(0xE4E474),
    },
    NamedColor {
        name: "Nation slot 0222",
        rgb: hex_to_rgb(0xD46868),
    },
    NamedColor {
        name: "Nation slot 0223",
        rgb: hex_to_rgb(0xC09C40),
    },
    NamedColor {
        name: "Nation slot 0224",
        rgb: hex_to_rgb(0x48B8A4),
    },
    NamedColor {
        name: "Nation slot 0225",
        rgb: hex_to_rgb(0x08CCD4),
    },
    NamedColor {
        name: "Nation slot 0226",
        rgb: hex_to_rgb(0xB8786C),
    },
    NamedColor {
        name: "Nation slot 0227",
        rgb: hex_to_rgb(0xA43400),
    },
    NamedColor {
        name: "Nation slot 0228",
        rgb: hex_to_rgb(0x684428),
    },
    NamedColor {
        name: "Nation slot 0229",
        rgb: hex_to_rgb(0xB47004),
    },
    NamedColor {
        name: "Nation slot 0230",
        rgb: hex_to_rgb(0x286840),
    },
    NamedColor {
        name: "Nation slot 0231",
        rgb: hex_to_rgb(0x387070),
    },
    NamedColor {
        name: "Nation slot 0232",
        rgb: hex_to_rgb(0x7854A4),
    },
    NamedColor {
        name: "Nation slot 0233",
        rgb: hex_to_rgb(0x543C40),
    },
    NamedColor {
        name: "Nation slot 0234",
        rgb: hex_to_rgb(0x0C50AC),
    },
    NamedColor {
        name: "Nation slot 0235",
        rgb: hex_to_rgb(0x00B09C),
    },
    NamedColor {
        name: "Nation slot 0236",
        rgb: hex_to_rgb(0x44985C),
    },
    NamedColor {
        name: "Nation slot 0237",
        rgb: hex_to_rgb(0x908460),
    },
    NamedColor {
        name: "Nation slot 0238",
        rgb: hex_to_rgb(0x845C30),
    },
    NamedColor {
        name: "Nation slot 0239",
        rgb: hex_to_rgb(0x286474),
    },
    NamedColor {
        name: "Nation slot 0240",
        rgb: hex_to_rgb(0x54988C),
    },
    NamedColor {
        name: "Nation slot 0241",
        rgb: hex_to_rgb(0x9C90FC),
    },
    NamedColor {
        name: "Nation slot 0242",
        rgb: hex_to_rgb(0x705454),
    },
    NamedColor {
        name: "Nation slot 0243",
        rgb: hex_to_rgb(0x407C8C),
    },
    NamedColor {
        name: "Nation slot 0244",
        rgb: hex_to_rgb(0x80AC34),
    },
    NamedColor {
        name: "Nation slot 0245",
        rgb: hex_to_rgb(0x78BC80),
    },
    NamedColor {
        name: "Nation slot 0246",
        rgb: hex_to_rgb(0xA8A45C),
    },
    NamedColor {
        name: "Nation slot 0247",
        rgb: hex_to_rgb(0xB0703C),
    },
    NamedColor {
        name: "Nation slot 0248",
        rgb: hex_to_rgb(0x8CFF74),
    },
    NamedColor {
        name: "Nation slot 0249",
        rgb: hex_to_rgb(0x0CA8C0),
    },
    NamedColor {
        name: "Nation slot 0250",
        rgb: hex_to_rgb(0xE890FC),
    },
    NamedColor {
        name: "Nation slot 0251",
        rgb: hex_to_rgb(0x6C7858),
    },
    NamedColor {
        name: "Nation slot 0252",
        rgb: hex_to_rgb(0x843C40),
    },
    NamedColor {
        name: "Nation slot 0253",
        rgb: hex_to_rgb(0xD48010),
    },
    NamedColor {
        name: "Nation slot 0254",
        rgb: hex_to_rgb(0xF0888C),
    },
    NamedColor {
        name: "Nation slot 0255",
        rgb: hex_to_rgb(0xFF6C54),
    },
    NamedColor {
        name: "Nation slot 0256",
        rgb: hex_to_rgb(0xE05C8C),
    },
    NamedColor {
        name: "Nation slot 0257",
        rgb: hex_to_rgb(0xD4784C),
    },
    NamedColor {
        name: "Nation slot 0258",
        rgb: hex_to_rgb(0xA85038),
    },
    NamedColor {
        name: "Nation slot 0259",
        rgb: hex_to_rgb(0x703090),
    },
    NamedColor {
        name: "Nation slot 0260",
        rgb: hex_to_rgb(0x2884E0),
    },
    NamedColor {
        name: "Nation slot 0261",
        rgb: hex_to_rgb(0xD460A8),
    },
    NamedColor {
        name: "Nation slot 0262",
        rgb: hex_to_rgb(0x847C20),
    },
    NamedColor {
        name: "Nation slot 0263",
        rgb: hex_to_rgb(0x14706C),
    },
    NamedColor {
        name: "Nation slot 0264",
        rgb: hex_to_rgb(0x286C24),
    },
    NamedColor {
        name: "Nation slot 0265",
        rgb: hex_to_rgb(0x802C70),
    },
    NamedColor {
        name: "Nation slot 0266",
        rgb: hex_to_rgb(0x54A458),
    },
    NamedColor {
        name: "Nation slot 0267",
        rgb: hex_to_rgb(0x288048),
    },
    NamedColor {
        name: "Nation slot 0268",
        rgb: hex_to_rgb(0x348070),
    },
    NamedColor {
        name: "Nation slot 0269",
        rgb: hex_to_rgb(0x245858),
    },
    NamedColor {
        name: "Nation slot 0270",
        rgb: hex_to_rgb(0x707800),
    },
    NamedColor {
        name: "Nation slot 0271",
        rgb: hex_to_rgb(0x8C5C04),
    },
    NamedColor {
        name: "Nation slot 0272",
        rgb: hex_to_rgb(0x7070B0),
    },
    NamedColor {
        name: "Nation slot 0273",
        rgb: hex_to_rgb(0x505C8C),
    },
    NamedColor {
        name: "Nation slot 0274",
        rgb: hex_to_rgb(0x2CBC78),
    },
    NamedColor {
        name: "Nation slot 0275",
        rgb: hex_to_rgb(0x185074),
    },
    NamedColor {
        name: "Nation slot 0276",
        rgb: hex_to_rgb(0x903054),
    },
    NamedColor {
        name: "Nation slot 0277",
        rgb: hex_to_rgb(0xC8B040),
    },
    NamedColor {
        name: "Nation slot 0278",
        rgb: hex_to_rgb(0x9C80C8),
    },
    NamedColor {
        name: "Nation slot 0279",
        rgb: hex_to_rgb(0x005448),
    },
    NamedColor {
        name: "Nation slot 0280",
        rgb: hex_to_rgb(0x443C80),
    },
    NamedColor {
        name: "Nation slot 0281",
        rgb: hex_to_rgb(0x487834),
    },
    NamedColor {
        name: "Nation slot 0282",
        rgb: hex_to_rgb(0x5048B4),
    },
    NamedColor {
        name: "Nation slot 0283",
        rgb: hex_to_rgb(0xC4EC64),
    },
    NamedColor {
        name: "Nation slot 0284",
        rgb: hex_to_rgb(0x8C4854),
    },
    NamedColor {
        name: "Nation slot 0285",
        rgb: hex_to_rgb(0x643460),
    },
    NamedColor {
        name: "Nation slot 0286",
        rgb: hex_to_rgb(0xA4FFF4),
    },
    NamedColor {
        name: "Nation slot 0287",
        rgb: hex_to_rgb(0x9060C8),
    },
    NamedColor {
        name: "Nation slot 0288",
        rgb: hex_to_rgb(0x50A4BC),
    },
    NamedColor {
        name: "Nation slot 0289",
        rgb: hex_to_rgb(0x407004),
    },
    NamedColor {
        name: "Nation slot 0290",
        rgb: hex_to_rgb(0x448C68),
    },
    NamedColor {
        name: "Nation slot 0291",
        rgb: hex_to_rgb(0x283C9C),
    },
    NamedColor {
        name: "Nation slot 0292",
        rgb: hex_to_rgb(0x5C90CC),
    },
    NamedColor {
        name: "Nation slot 0293",
        rgb: hex_to_rgb(0xB85478),
    },
    NamedColor {
        name: "Nation slot 0294",
        rgb: hex_to_rgb(0x0480A0),
    },
    NamedColor {
        name: "Nation slot 0295",
        rgb: hex_to_rgb(0x8C9448),
    },
    NamedColor {
        name: "Nation slot 0296",
        rgb: hex_to_rgb(0xD44C54),
    },
    NamedColor {
        name: "Nation slot 0297",
        rgb: hex_to_rgb(0x604474),
    },
    NamedColor {
        name: "Nation slot 0298",
        rgb: hex_to_rgb(0x90B080),
    },
    NamedColor {
        name: "Nation slot 0299",
        rgb: hex_to_rgb(0x608888),
    },
    NamedColor {
        name: "Nation slot 0300",
        rgb: hex_to_rgb(0x84BC60),
    },
    NamedColor {
        name: "Nation slot 0301",
        rgb: hex_to_rgb(0x842814),
    },
    NamedColor {
        name: "Nation slot 0302",
        rgb: hex_to_rgb(0xB89478),
    },
    NamedColor {
        name: "Nation slot 0303",
        rgb: hex_to_rgb(0xB878C4),
    },
    NamedColor {
        name: "Nation slot 0304",
        rgb: hex_to_rgb(0xA0401C),
    },
    NamedColor {
        name: "Nation slot 0305",
        rgb: hex_to_rgb(0x20708C),
    },
    NamedColor {
        name: "Nation slot 0306",
        rgb: hex_to_rgb(0x609C28),
    },
    NamedColor {
        name: "Nation slot 0307",
        rgb: hex_to_rgb(0x7C205C),
    },
    NamedColor {
        name: "Nation slot 0308",
        rgb: hex_to_rgb(0x883804),
    },
    NamedColor {
        name: "Nation slot 0309",
        rgb: hex_to_rgb(0x705C00),
    },
    NamedColor {
        name: "Nation slot 0310",
        rgb: hex_to_rgb(0x443CA8),
    },
    NamedColor {
        name: "Nation slot 0311",
        rgb: hex_to_rgb(0x7C70D4),
    },
    NamedColor {
        name: "Nation slot 0312",
        rgb: hex_to_rgb(0x78448C),
    },
    NamedColor {
        name: "Nation slot 0313",
        rgb: hex_to_rgb(0x3C4894),
    },
    NamedColor {
        name: "Nation slot 0314",
        rgb: hex_to_rgb(0x305800),
    },
    NamedColor {
        name: "Nation slot 0315",
        rgb: hex_to_rgb(0x0C741C),
    },
    NamedColor {
        name: "Nation slot 0316",
        rgb: hex_to_rgb(0xCC4834),
    },
    NamedColor {
        name: "Nation slot 0317",
        rgb: hex_to_rgb(0x10B080),
    },
    NamedColor {
        name: "Nation slot 0318",
        rgb: hex_to_rgb(0x6078A0),
    },
    NamedColor {
        name: "Nation slot 0319",
        rgb: hex_to_rgb(0xD4507C),
    },
    NamedColor {
        name: "Nation slot 0320",
        rgb: hex_to_rgb(0xBC7C24),
    },
    NamedColor {
        name: "Nation slot 0321",
        rgb: hex_to_rgb(0x486064),
    },
    NamedColor {
        name: "Nation slot 0322",
        rgb: hex_to_rgb(0x448428),
    },
    NamedColor {
        name: "Nation slot 0323",
        rgb: hex_to_rgb(0x842034),
    },
    NamedColor {
        name: "Nation slot 0324",
        rgb: hex_to_rgb(0xC08460),
    },
    NamedColor {
        name: "Nation slot 0325",
        rgb: hex_to_rgb(0x80A478),
    },
    NamedColor {
        name: "Nation slot 0326",
        rgb: hex_to_rgb(0x4C5018),
    },
    NamedColor {
        name: "Nation slot 0327",
        rgb: hex_to_rgb(0x888094),
    },
    NamedColor {
        name: "Nation slot 0328",
        rgb: hex_to_rgb(0xCC6C34),
    },
    NamedColor {
        name: "Nation slot 0329",
        rgb: hex_to_rgb(0x288C9C),
    },
    NamedColor {
        name: "Nation slot 0330",
        rgb: hex_to_rgb(0x64B0AC),
    },
    NamedColor {
        name: "Nation slot 0331",
        rgb: hex_to_rgb(0x008074),
    },
    NamedColor {
        name: "Nation slot 0332",
        rgb: hex_to_rgb(0x709054),
    },
    NamedColor {
        name: "Nation slot 0333",
        rgb: hex_to_rgb(0x688440),
    },
    NamedColor {
        name: "Nation slot 0334",
        rgb: hex_to_rgb(0x9850AC),
    },
    NamedColor {
        name: "Nation slot 0335",
        rgb: hex_to_rgb(0xA85C20),
    },
    NamedColor {
        name: "Nation slot 0336",
        rgb: hex_to_rgb(0x5C6C3C),
    },
    NamedColor {
        name: "Nation slot 0337",
        rgb: hex_to_rgb(0x9C9410),
    },
    NamedColor {
        name: "Nation slot 0338",
        rgb: hex_to_rgb(0x5C5C20),
    },
    NamedColor {
        name: "Nation slot 0339",
        rgb: hex_to_rgb(0x4078E0),
    },
    NamedColor {
        name: "Nation slot 0340",
        rgb: hex_to_rgb(0x9868A0),
    },
    NamedColor {
        name: "Nation slot 0341",
        rgb: hex_to_rgb(0x44AC8C),
    },
    NamedColor {
        name: "Nation slot 0342",
        rgb: hex_to_rgb(0x006094),
    },
    NamedColor {
        name: "Nation slot 0343",
        rgb: hex_to_rgb(0x749024),
    },
    NamedColor {
        name: "Nation slot 0344",
        rgb: hex_to_rgb(0xA0843C),
    },
    NamedColor {
        name: "Nation slot 0345",
        rgb: hex_to_rgb(0x6C80D4),
    },
    NamedColor {
        name: "Nation slot 0346",
        rgb: hex_to_rgb(0x048034),
    },
    NamedColor {
        name: "Nation slot 0347",
        rgb: hex_to_rgb(0xCC5814),
    },
    NamedColor {
        name: "Nation slot 0348",
        rgb: hex_to_rgb(0x144850),
    },
    NamedColor {
        name: "Nation slot 0349",
        rgb: hex_to_rgb(0xFFC888),
    },
    NamedColor {
        name: "Nation slot 0350",
        rgb: hex_to_rgb(0x08BCD8),
    },
    NamedColor {
        name: "Nation slot 0351",
        rgb: hex_to_rgb(0xDC80B4),
    },
    NamedColor {
        name: "Nation slot 0352",
        rgb: hex_to_rgb(0x8C6850),
    },
    NamedColor {
        name: "Nation slot 0353",
        rgb: hex_to_rgb(0xE0A4FF),
    },
    NamedColor {
        name: "Nation slot 0354",
        rgb: hex_to_rgb(0x98343C),
    },
    NamedColor {
        name: "Nation slot 0355",
        rgb: hex_to_rgb(0x787CAC),
    },
    NamedColor {
        name: "Nation slot 0356",
        rgb: hex_to_rgb(0x2068A4),
    },
    NamedColor {
        name: "Nation slot 0357",
        rgb: hex_to_rgb(0xBC8098),
    },
    NamedColor {
        name: "Nation slot 0358",
        rgb: hex_to_rgb(0xAC5C5C),
    },
    NamedColor {
        name: "Nation slot 0359",
        rgb: hex_to_rgb(0x98B4A0),
    },
    NamedColor {
        name: "Nation slot 0360",
        rgb: hex_to_rgb(0xA87C58),
    },
    NamedColor {
        name: "Nation slot 0361",
        rgb: hex_to_rgb(0xA090B8),
    },
    NamedColor {
        name: "Nation slot 0362",
        rgb: hex_to_rgb(0x784874),
    },
    NamedColor {
        name: "Nation slot 0363",
        rgb: hex_to_rgb(0x889C88),
    },
    NamedColor {
        name: "Nation slot 0364",
        rgb: hex_to_rgb(0x54806C),
    },
    NamedColor {
        name: "Nation slot 0365",
        rgb: hex_to_rgb(0x644C9C),
    },
    NamedColor {
        name: "Nation slot 0366",
        rgb: hex_to_rgb(0x1C90BC),
    },
    NamedColor {
        name: "Nation slot 0367",
        rgb: hex_to_rgb(0x2CB8B8),
    },
    NamedColor {
        name: "Nation slot 0368",
        rgb: hex_to_rgb(0x2854AC),
    },
    NamedColor {
        name: "Nation slot 0369",
        rgb: hex_to_rgb(0xF8DCB0),
    },
    NamedColor {
        name: "Nation slot 0370",
        rgb: hex_to_rgb(0x6CB458),
    },
    NamedColor {
        name: "Nation slot 0371",
        rgb: hex_to_rgb(0xA42828),
    },
    NamedColor {
        name: "Nation slot 0372",
        rgb: hex_to_rgb(0x743434),
    },
    NamedColor {
        name: "Nation slot 0373",
        rgb: hex_to_rgb(0x085C2C),
    },
    NamedColor {
        name: "Nation slot 0374",
        rgb: hex_to_rgb(0x743818),
    },
    NamedColor {
        name: "Nation slot 0375",
        rgb: hex_to_rgb(0xD07C6C),
    },
    NamedColor {
        name: "Nation slot 0376",
        rgb: hex_to_rgb(0x946CC0),
    },
    NamedColor {
        name: "Nation slot 0377",
        rgb: hex_to_rgb(0x8C7024),
    },
    NamedColor {
        name: "Nation slot 0378",
        rgb: hex_to_rgb(0x74A060),
    },
    NamedColor {
        name: "Nation slot 0379",
        rgb: hex_to_rgb(0xA42C60),
    },
    NamedColor {
        name: "Nation slot 0380",
        rgb: hex_to_rgb(0x444C00),
    },
    NamedColor {
        name: "Nation slot 0381",
        rgb: hex_to_rgb(0x643C50),
    },
    NamedColor {
        name: "Nation slot 0382",
        rgb: hex_to_rgb(0x985084),
    },
    NamedColor {
        name: "Nation slot 0383",
        rgb: hex_to_rgb(0x4894A0),
    },
    NamedColor {
        name: "Nation slot 0384",
        rgb: hex_to_rgb(0x2474B4),
    },
    NamedColor {
        name: "Nation slot 0385",
        rgb: hex_to_rgb(0x705474),
    },
    NamedColor {
        name: "Nation slot 0386",
        rgb: hex_to_rgb(0x604804),
    },
    NamedColor {
        name: "Nation slot 0387",
        rgb: hex_to_rgb(0x70F0E0),
    },
    NamedColor {
        name: "Nation slot 0388",
        rgb: hex_to_rgb(0x344C54),
    },
    NamedColor {
        name: "Nation slot 0389",
        rgb: hex_to_rgb(0x6038A4),
    },
    NamedColor {
        name: "Nation slot 0390",
        rgb: hex_to_rgb(0xA0A4FC),
    },
    NamedColor {
        name: "Nation slot 0391",
        rgb: hex_to_rgb(0xA8D8F8),
    },
    NamedColor {
        name: "Nation slot 0392",
        rgb: hex_to_rgb(0xB8A0A8),
    },
    NamedColor {
        name: "Nation slot 0393",
        rgb: hex_to_rgb(0x943020),
    },
    NamedColor {
        name: "Nation slot 0394",
        rgb: hex_to_rgb(0xC44880),
    },
    NamedColor {
        name: "Nation slot 0395",
        rgb: hex_to_rgb(0x807080),
    },
    NamedColor {
        name: "Nation slot 0396",
        rgb: hex_to_rgb(0x24ACA4),
    },
    NamedColor {
        name: "Nation slot 0397",
        rgb: hex_to_rgb(0x987088),
    },
    NamedColor {
        name: "Nation slot 0398",
        rgb: hex_to_rgb(0x3C68CC),
    },
    NamedColor {
        name: "Nation slot 0399",
        rgb: hex_to_rgb(0xB43C0C),
    },
    NamedColor {
        name: "Nation slot 0400",
        rgb: hex_to_rgb(0x0C9C70),
    },
    NamedColor {
        name: "Nation slot 0401",
        rgb: hex_to_rgb(0xCC8CB8),
    },
    NamedColor {
        name: "Nation slot 0402",
        rgb: hex_to_rgb(0x5C701C),
    },
    NamedColor {
        name: "Nation slot 0403",
        rgb: hex_to_rgb(0x145C70),
    },
    NamedColor {
        name: "Nation slot 0404",
        rgb: hex_to_rgb(0x88607C),
    },
    NamedColor {
        name: "Nation slot 0405",
        rgb: hex_to_rgb(0x406C48),
    },
    NamedColor {
        name: "Nation slot 0406",
        rgb: hex_to_rgb(0xD4688C),
    },
    NamedColor {
        name: "Nation slot 0407",
        rgb: hex_to_rgb(0xA08400),
    },
    NamedColor {
        name: "Nation slot 0408",
        rgb: hex_to_rgb(0xB44028),
    },
    NamedColor {
        name: "Nation slot 0409",
        rgb: hex_to_rgb(0x405850),
    },
    NamedColor {
        name: "Nation slot 0410",
        rgb: hex_to_rgb(0x6848B4),
    },
    NamedColor {
        name: "Nation slot 0411",
        rgb: hex_to_rgb(0x5858A0),
    },
    NamedColor {
        name: "Nation slot 0412",
        rgb: hex_to_rgb(0x6460BC),
    },
    NamedColor {
        name: "Nation slot 0413",
        rgb: hex_to_rgb(0x884414),
    },
    NamedColor {
        name: "Nation slot 0414",
        rgb: hex_to_rgb(0x70947C),
    },
    NamedColor {
        name: "Nation slot 0415",
        rgb: hex_to_rgb(0x6CB084),
    },
    NamedColor {
        name: "Nation slot 0416",
        rgb: hex_to_rgb(0xC86054),
    },
    NamedColor {
        name: "Nation slot 0417",
        rgb: hex_to_rgb(0x204898),
    },
    NamedColor {
        name: "Nation slot 0418",
        rgb: hex_to_rgb(0x9C4868),
    },
    NamedColor {
        name: "Nation slot 0419",
        rgb: hex_to_rgb(0x007484),
    },
    NamedColor {
        name: "Nation slot 0420",
        rgb: hex_to_rgb(0xB44458),
    },
    NamedColor {
        name: "Nation slot 0421",
        rgb: hex_to_rgb(0x2068C4),
    },
    NamedColor {
        name: "Nation slot 0422",
        rgb: hex_to_rgb(0x9044A8),
    },
    NamedColor {
        name: "Nation slot 0423",
        rgb: hex_to_rgb(0xC4CCE4),
    },
    NamedColor {
        name: "Nation slot 0424",
        rgb: hex_to_rgb(0x745840),
    },
    NamedColor {
        name: "Nation slot 0425",
        rgb: hex_to_rgb(0x207440),
    },
    NamedColor {
        name: "Nation slot 0426",
        rgb: hex_to_rgb(0xD09418),
    },
    NamedColor {
        name: "Nation slot 0427",
        rgb: hex_to_rgb(0x9C3C90),
    },
    NamedColor {
        name: "Nation slot 0428",
        rgb: hex_to_rgb(0xAC6C54),
    },
    NamedColor {
        name: "Nation slot 0429",
        rgb: hex_to_rgb(0xFF8C24),
    },
    NamedColor {
        name: "Nation slot 0430",
        rgb: hex_to_rgb(0x949C60),
    },
    NamedColor {
        name: "Nation slot 0431",
        rgb: hex_to_rgb(0x68A0A8),
    },
    NamedColor {
        name: "Nation slot 0432",
        rgb: hex_to_rgb(0x647484),
    },
    NamedColor {
        name: "Nation slot 0433",
        rgb: hex_to_rgb(0x64CCEC),
    },
    NamedColor {
        name: "Nation slot 0434",
        rgb: hex_to_rgb(0x8CA848),
    },
    NamedColor {
        name: "Nation slot 0435",
        rgb: hex_to_rgb(0x1C9888),
    },
    NamedColor {
        name: "Nation slot 0436",
        rgb: hex_to_rgb(0x087058),
    },
    NamedColor {
        name: "Nation slot 0437",
        rgb: hex_to_rgb(0xB05C90),
    },
    NamedColor {
        name: "Nation slot 0438",
        rgb: hex_to_rgb(0x4450A8),
    },
    NamedColor {
        name: "Nation slot 0439",
        rgb: hex_to_rgb(0x583C28),
    },
    NamedColor {
        name: "Nation slot 0440",
        rgb: hex_to_rgb(0x289050),
    },
    NamedColor {
        name: "Nation slot 0441",
        rgb: hex_to_rgb(0x805864),
    },
    NamedColor {
        name: "Nation slot 0442",
        rgb: hex_to_rgb(0x089CB0),
    },
    NamedColor {
        name: "Nation slot 0443",
        rgb: hex_to_rgb(0xCC883C),
    },
    NamedColor {
        name: "Nation slot 0444",
        rgb: hex_to_rgb(0x404018),
    },
    NamedColor {
        name: "Nation slot 0445",
        rgb: hex_to_rgb(0x105414),
    },
    NamedColor {
        name: "Nation slot 0446",
        rgb: hex_to_rgb(0xC05460),
    },
    NamedColor {
        name: "Nation slot 0447",
        rgb: hex_to_rgb(0x5898F0),
    },
    NamedColor {
        name: "Nation slot 0448",
        rgb: hex_to_rgb(0x30ACBC),
    },
    NamedColor {
        name: "Nation slot 0449",
        rgb: hex_to_rgb(0x5C78E8),
    },
    NamedColor {
        name: "Nation slot 0450",
        rgb: hex_to_rgb(0xC0C07C),
    },
    NamedColor {
        name: "Nation slot 0451",
        rgb: hex_to_rgb(0x4C485C),
    },
    NamedColor {
        name: "Nation slot 0452",
        rgb: hex_to_rgb(0x447CA4),
    },
    NamedColor {
        name: "Nation slot 0453",
        rgb: hex_to_rgb(0xB07084),
    },
    NamedColor {
        name: "Nation slot 0454",
        rgb: hex_to_rgb(0x988CA0),
    },
    NamedColor {
        name: "Nation slot 0455",
        rgb: hex_to_rgb(0xDC6C50),
    },
    NamedColor {
        name: "Nation slot 0456",
        rgb: hex_to_rgb(0xD87898),
    },
    NamedColor {
        name: "Nation slot 0457",
        rgb: hex_to_rgb(0xD8641C),
    },
    NamedColor {
        name: "Nation slot 0458",
        rgb: hex_to_rgb(0xA4B834),
    },
    NamedColor {
        name: "Nation slot 0459",
        rgb: hex_to_rgb(0x3C9CD4),
    },
    NamedColor {
        name: "Nation slot 0460",
        rgb: hex_to_rgb(0x305030),
    },
    NamedColor {
        name: "Nation slot 0461",
        rgb: hex_to_rgb(0x008C5C),
    },
    NamedColor {
        name: "Nation slot 0462",
        rgb: hex_to_rgb(0xAC78F0),
    },
    NamedColor {
        name: "Nation slot 0463",
        rgb: hex_to_rgb(0x806830),
    },
    NamedColor {
        name: "Nation slot 0464",
        rgb: hex_to_rgb(0x2090D8),
    },
    NamedColor {
        name: "Nation slot 0465",
        rgb: hex_to_rgb(0x4CE8F8),
    },
    NamedColor {
        name: "Nation slot 0466",
        rgb: hex_to_rgb(0xF06484),
    },
    NamedColor {
        name: "Nation slot 0467",
        rgb: hex_to_rgb(0x742834),
    },
    NamedColor {
        name: "Nation slot 0468",
        rgb: hex_to_rgb(0x9C80AC),
    },
    NamedColor {
        name: "Nation slot 0469",
        rgb: hex_to_rgb(0x686038),
    },
    NamedColor {
        name: "Nation slot 0470",
        rgb: hex_to_rgb(0x54C8FC),
    },
    NamedColor {
        name: "Nation slot 0471",
        rgb: hex_to_rgb(0xB454A4),
    },
    NamedColor {
        name: "Nation slot 0472",
        rgb: hex_to_rgb(0xF8AC94),
    },
    NamedColor {
        name: "Nation slot 0473",
        rgb: hex_to_rgb(0x505474),
    },
    NamedColor {
        name: "Nation slot 0474",
        rgb: hex_to_rgb(0xF09C5C),
    },
    NamedColor {
        name: "Nation slot 0475",
        rgb: hex_to_rgb(0x9C6C00),
    },
    NamedColor {
        name: "Nation slot 0476",
        rgb: hex_to_rgb(0x1C4464),
    },
    NamedColor {
        name: "Nation slot 0477",
        rgb: hex_to_rgb(0x44C08C),
    },
    NamedColor {
        name: "Nation slot 0478",
        rgb: hex_to_rgb(0x4C705C),
    },
    NamedColor {
        name: "Nation slot 0479",
        rgb: hex_to_rgb(0x008894),
    },
    NamedColor {
        name: "Nation slot 0480",
        rgb: hex_to_rgb(0xE05868),
    },
    NamedColor {
        name: "Nation slot 0481",
        rgb: hex_to_rgb(0xD080F8),
    },
    NamedColor {
        name: "Nation slot 0482",
        rgb: hex_to_rgb(0x5870B8),
    },
    NamedColor {
        name: "Nation slot 0483",
        rgb: hex_to_rgb(0xB8643C),
    },
    NamedColor {
        name: "Nation slot 0484",
        rgb: hex_to_rgb(0x489874),
    },
    NamedColor {
        name: "Nation slot 0485",
        rgb: hex_to_rgb(0x7C8C38),
    },
    NamedColor {
        name: "Nation slot 0486",
        rgb: hex_to_rgb(0x40A470),
    },
    NamedColor {
        name: "Nation slot 0487",
        rgb: hex_to_rgb(0x7088A4),
    },
    NamedColor {
        name: "Nation slot 0488",
        rgb: hex_to_rgb(0x1C642C),
    },
    NamedColor {
        name: "Nation slot 0489",
        rgb: hex_to_rgb(0x383C60),
    },
    NamedColor {
        name: "Nation slot 0490",
        rgb: hex_to_rgb(0x7488C8),
    },
    NamedColor {
        name: "Nation slot 0491",
        rgb: hex_to_rgb(0x903470),
    },
    NamedColor {
        name: "Nation slot 0492",
        rgb: hex_to_rgb(0x9C60B4),
    },
    NamedColor {
        name: "Nation slot 0493",
        rgb: hex_to_rgb(0x7C3448),
    },
    NamedColor {
        name: "Nation slot 0494",
        rgb: hex_to_rgb(0x4C60B8),
    },
    NamedColor {
        name: "Nation slot 0495",
        rgb: hex_to_rgb(0x407020),
    },
    NamedColor {
        name: "Nation slot 0496",
        rgb: hex_to_rgb(0x2C789C),
    },
    NamedColor {
        name: "Nation slot 0497",
        rgb: hex_to_rgb(0xBC60C0),
    },
    NamedColor {
        name: "Nation slot 0498",
        rgb: hex_to_rgb(0x6C6CE0),
    },
    NamedColor {
        name: "Nation slot 0499",
        rgb: hex_to_rgb(0x80508C),
    },
    NamedColor {
        name: "Nation slot 0500",
        rgb: hex_to_rgb(0x702074),
    },
    NamedColor {
        name: "Nation slot 0501",
        rgb: hex_to_rgb(0xB874A0),
    },
    NamedColor {
        name: "Nation slot 0502",
        rgb: hex_to_rgb(0x345078),
    },
    NamedColor {
        name: "Nation slot 0503",
        rgb: hex_to_rgb(0x40C87C),
    },
    NamedColor {
        name: "Nation slot 0504",
        rgb: hex_to_rgb(0x445C24),
    },
    NamedColor {
        name: "Nation slot 0505",
        rgb: hex_to_rgb(0x98A024),
    },
    NamedColor {
        name: "Nation slot 0506",
        rgb: hex_to_rgb(0xA05808),
    },
    NamedColor {
        name: "Nation slot 0507",
        rgb: hex_to_rgb(0x78485C),
    },
    NamedColor {
        name: "Nation slot 0508",
        rgb: hex_to_rgb(0xA03C6C),
    },
    NamedColor {
        name: "Nation slot 0509",
        rgb: hex_to_rgb(0xD0A054),
    },
    NamedColor {
        name: "Nation slot 0510",
        rgb: hex_to_rgb(0xB08488),
    },
    NamedColor {
        name: "Nation slot 0511",
        rgb: hex_to_rgb(0x583874),
    },
    NamedColor {
        name: "Nation slot 0512",
        rgb: hex_to_rgb(0xA48420),
    },
    NamedColor {
        name: "Nation slot 0513",
        rgb: hex_to_rgb(0xB8A02C),
    },
    NamedColor {
        name: "Nation slot 0514",
        rgb: hex_to_rgb(0x606C9C),
    },
    NamedColor {
        name: "Nation slot 0515",
        rgb: hex_to_rgb(0x4484D0),
    },
    NamedColor {
        name: "Nation slot 0516",
        rgb: hex_to_rgb(0x74701C),
    },
    NamedColor {
        name: "Nation slot 0517",
        rgb: hex_to_rgb(0x0C448C),
    },
    NamedColor {
        name: "Nation slot 0518",
        rgb: hex_to_rgb(0x4C9034),
    },
    NamedColor {
        name: "Nation slot 0519",
        rgb: hex_to_rgb(0x7CB4B8),
    },
    NamedColor {
        name: "Nation slot 0520",
        rgb: hex_to_rgb(0x6C54C4),
    },
    NamedColor {
        name: "Nation slot 0521",
        rgb: hex_to_rgb(0x30442C),
    },
    NamedColor {
        name: "Nation slot 0522",
        rgb: hex_to_rgb(0x98586C),
    },
    NamedColor {
        name: "Nation slot 0523",
        rgb: hex_to_rgb(0x44B058),
    },
    NamedColor {
        name: "Nation slot 0524",
        rgb: hex_to_rgb(0x28A05C),
    },
    NamedColor {
        name: "Nation slot 0525",
        rgb: hex_to_rgb(0x749CC0),
    },
    NamedColor {
        name: "Nation slot 0526",
        rgb: hex_to_rgb(0xA06838),
    },
    NamedColor {
        name: "Nation slot 0527",
        rgb: hex_to_rgb(0x04A8A8),
    },
    NamedColor {
        name: "Nation slot 0528",
        rgb: hex_to_rgb(0x94BCEC),
    },
    NamedColor {
        name: "Nation slot 0529",
        rgb: hex_to_rgb(0xA46C20),
    },
    NamedColor {
        name: "Nation slot 0530",
        rgb: hex_to_rgb(0x04BCA8),
    },
    NamedColor {
        name: "Nation slot 0531",
        rgb: hex_to_rgb(0x783060),
    },
    NamedColor {
        name: "Nation slot 0532",
        rgb: hex_to_rgb(0x683C90),
    },
    NamedColor {
        name: "Nation slot 0533",
        rgb: hex_to_rgb(0x98202C),
    },
    NamedColor {
        name: "Nation slot 0534",
        rgb: hex_to_rgb(0xACB468),
    },
    NamedColor {
        name: "Nation slot 0535",
        rgb: hex_to_rgb(0x546C7C),
    },
    NamedColor {
        name: "Nation slot 0536",
        rgb: hex_to_rgb(0xF8DC30),
    },
    NamedColor {
        name: "Nation slot 0537",
        rgb: hex_to_rgb(0xB498B8),
    },
    NamedColor {
        name: "Nation slot 0538",
        rgb: hex_to_rgb(0xCC7408),
    },
    NamedColor {
        name: "Nation slot 0539",
        rgb: hex_to_rgb(0x00947C),
    },
    NamedColor {
        name: "Nation slot 0540",
        rgb: hex_to_rgb(0x704C14),
    },
    NamedColor {
        name: "Nation slot 0541",
        rgb: hex_to_rgb(0x24882C),
    },
    NamedColor {
        name: "Nation slot 0542",
        rgb: hex_to_rgb(0x708418),
    },
    NamedColor {
        name: "Nation slot 0543",
        rgb: hex_to_rgb(0x8C58A4),
    },
    NamedColor {
        name: "Nation slot 0544",
        rgb: hex_to_rgb(0xB04088),
    },
    NamedColor {
        name: "Nation slot 0545",
        rgb: hex_to_rgb(0x3C5C7C),
    },
    NamedColor {
        name: "Nation slot 0546",
        rgb: hex_to_rgb(0x3CA08C),
    },
    NamedColor {
        name: "Nation slot 0547",
        rgb: hex_to_rgb(0xBC9890),
    },
    NamedColor {
        name: "Nation slot 0548",
        rgb: hex_to_rgb(0x74581C),
    },
    NamedColor {
        name: "Nation slot 0549",
        rgb: hex_to_rgb(0x944C08),
    },
    NamedColor {
        name: "Nation slot 0550",
        rgb: hex_to_rgb(0x7C5CB8),
    },
    NamedColor {
        name: "Nation slot 0551",
        rgb: hex_to_rgb(0x7C8058),
    },
    NamedColor {
        name: "Nation slot 0552",
        rgb: hex_to_rgb(0x9C9470),
    },
    NamedColor {
        name: "Nation slot 0553",
        rgb: hex_to_rgb(0x90244C),
    },
    NamedColor {
        name: "Nation slot 0554",
        rgb: hex_to_rgb(0x5C8054),
    },
    NamedColor {
        name: "Nation slot 0555",
        rgb: hex_to_rgb(0x84900C),
    },
    NamedColor {
        name: "Nation slot 0556",
        rgb: hex_to_rgb(0x0470BC),
    },
    NamedColor {
        name: "Nation slot 0557",
        rgb: hex_to_rgb(0x5C4488),
    },
    NamedColor {
        name: "Nation slot 0558",
        rgb: hex_to_rgb(0xA89024),
    },
    NamedColor {
        name: "Nation slot 0559",
        rgb: hex_to_rgb(0xFF8030),
    },
    NamedColor {
        name: "Nation slot 0560",
        rgb: hex_to_rgb(0x503498),
    },
    NamedColor {
        name: "Nation slot 0561",
        rgb: hex_to_rgb(0x806054),
    },
    NamedColor {
        name: "Nation slot 0562",
        rgb: hex_to_rgb(0xB46078),
    },
    NamedColor {
        name: "Nation slot 0563",
        rgb: hex_to_rgb(0xD05028),
    },
    NamedColor {
        name: "Nation slot 0564",
        rgb: hex_to_rgb(0x1C80CC),
    },
    NamedColor {
        name: "Nation slot 0565",
        rgb: hex_to_rgb(0x683400),
    },
    NamedColor {
        name: "Nation slot 0566",
        rgb: hex_to_rgb(0xB44C4C),
    },
    NamedColor {
        name: "Nation slot 0567",
        rgb: hex_to_rgb(0x3CECC4),
    },
    NamedColor {
        name: "Nation slot 0568",
        rgb: hex_to_rgb(0x4864A0),
    },
    NamedColor {
        name: "Nation slot 0569",
        rgb: hex_to_rgb(0xA4304C),
    },
    NamedColor {
        name: "Nation slot 0570",
        rgb: hex_to_rgb(0x5C6414),
    },
    NamedColor {
        name: "Nation slot 0571",
        rgb: hex_to_rgb(0x685C88),
    },
    NamedColor {
        name: "Nation slot 0572",
        rgb: hex_to_rgb(0xC45C88),
    },
    NamedColor {
        name: "Nation slot 0573",
        rgb: hex_to_rgb(0x8C5818),
    },
    NamedColor {
        name: "Nation slot 0574",
        rgb: hex_to_rgb(0x94583C),
    },
    NamedColor {
        name: "Nation slot 0575",
        rgb: hex_to_rgb(0xC0701C),
    },
    NamedColor {
        name: "Nation slot 0576",
        rgb: hex_to_rgb(0x285494),
    },
    NamedColor {
        name: "Nation slot 0577",
        rgb: hex_to_rgb(0x80AC90),
    },
    NamedColor {
        name: "Nation slot 0578",
        rgb: hex_to_rgb(0x944C34),
    },
    NamedColor {
        name: "Nation slot 0579",
        rgb: hex_to_rgb(0x78689C),
    },
    NamedColor {
        name: "Nation slot 0580",
        rgb: hex_to_rgb(0xAC9848),
    },
    NamedColor {
        name: "Nation slot 0581",
        rgb: hex_to_rgb(0x588090),
    },
    NamedColor {
        name: "Nation slot 0582",
        rgb: hex_to_rgb(0x9C78D4),
    },
    NamedColor {
        name: "Nation slot 0583",
        rgb: hex_to_rgb(0x88ECB8),
    },
    NamedColor {
        name: "Nation slot 0584",
        rgb: hex_to_rgb(0xA47C78),
    },
    NamedColor {
        name: "Nation slot 0585",
        rgb: hex_to_rgb(0x545C38),
    },
    NamedColor {
        name: "Nation slot 0586",
        rgb: hex_to_rgb(0xB83850),
    },
    NamedColor {
        name: "Nation slot 0587",
        rgb: hex_to_rgb(0x8C9830),
    },
    NamedColor {
        name: "Nation slot 0588",
        rgb: hex_to_rgb(0xA0783C),
    },
    NamedColor {
        name: "Nation slot 0589",
        rgb: hex_to_rgb(0x64909C),
    },
    NamedColor {
        name: "Nation slot 0590",
        rgb: hex_to_rgb(0xB8A07C),
    },
    NamedColor {
        name: "Nation slot 0591",
        rgb: hex_to_rgb(0xA05450),
    },
    NamedColor {
        name: "Nation slot 0592",
        rgb: hex_to_rgb(0x943008),
    },
    NamedColor {
        name: "Nation slot 0593",
        rgb: hex_to_rgb(0x403C94),
    },
    NamedColor {
        name: "Nation slot 0594",
        rgb: hex_to_rgb(0x804844),
    },
    NamedColor {
        name: "Nation slot 0595",
        rgb: hex_to_rgb(0x90B46C),
    },
    NamedColor {
        name: "Nation slot 0596",
        rgb: hex_to_rgb(0xD0F0E4),
    },
    NamedColor {
        name: "Nation slot 0597",
        rgb: hex_to_rgb(0x004C6C),
    },
    NamedColor {
        name: "Nation slot 0598",
        rgb: hex_to_rgb(0xC4687C),
    },
    NamedColor {
        name: "Nation slot 0599",
        rgb: hex_to_rgb(0xE0A4C4),
    },
    NamedColor {
        name: "Nation slot 0600",
        rgb: hex_to_rgb(0xA06874),
    },
    NamedColor {
        name: "Nation slot 0601",
        rgb: hex_to_rgb(0x902000),
    },
    NamedColor {
        name: "Nation slot 0602",
        rgb: hex_to_rgb(0x7468C0),
    },
    NamedColor {
        name: "Nation slot 0603",
        rgb: hex_to_rgb(0xA83C44),
    },
    NamedColor {
        name: "Nation slot 0604",
        rgb: hex_to_rgb(0x907CF4),
    },
    NamedColor {
        name: "Nation slot 0605",
        rgb: hex_to_rgb(0x20A4EC),
    },
    NamedColor {
        name: "Nation slot 0606",
        rgb: hex_to_rgb(0x88A4B4),
    },
    NamedColor {
        name: "Nation slot 0607",
        rgb: hex_to_rgb(0xBC9CFC),
    },
    NamedColor {
        name: "Nation slot 0608",
        rgb: hex_to_rgb(0xB8A84C),
    },
    NamedColor {
        name: "Nation slot 0609",
        rgb: hex_to_rgb(0xB890C4),
    },
    NamedColor {
        name: "Nation slot 0610",
        rgb: hex_to_rgb(0x884464),
    },
    NamedColor {
        name: "Nation slot 0611",
        rgb: hex_to_rgb(0xCC7CC4),
    },
    NamedColor {
        name: "Nation slot 0612",
        rgb: hex_to_rgb(0x106894),
    },
    NamedColor {
        name: "Nation slot 0613",
        rgb: hex_to_rgb(0x802C84),
    },
    NamedColor {
        name: "Nation slot 0614",
        rgb: hex_to_rgb(0xAC5020),
    },
    NamedColor {
        name: "Nation slot 0615",
        rgb: hex_to_rgb(0xEC7074),
    },
    NamedColor {
        name: "Nation slot 0616",
        rgb: hex_to_rgb(0x682C7C),
    },
    NamedColor {
        name: "Nation slot 0617",
        rgb: hex_to_rgb(0xC05008),
    },
    NamedColor {
        name: "Nation slot 0618",
        rgb: hex_to_rgb(0x30805C),
    },
    NamedColor {
        name: "Nation slot 0619",
        rgb: hex_to_rgb(0x5C4C20),
    },
    NamedColor {
        name: "Nation slot 0620",
        rgb: hex_to_rgb(0xFC80B8),
    },
    NamedColor {
        name: "Nation slot 0621",
        rgb: hex_to_rgb(0x6C98DC),
    },
    NamedColor {
        name: "Nation slot 0622",
        rgb: hex_to_rgb(0x0880B8),
    },
    NamedColor {
        name: "Nation slot 0623",
        rgb: hex_to_rgb(0x6C4C3C),
    },
    NamedColor {
        name: "Nation slot 0624",
        rgb: hex_to_rgb(0x108464),
    },
    NamedColor {
        name: "Nation slot 0625",
        rgb: hex_to_rgb(0x184C30),
    },
    NamedColor {
        name: "Nation slot 0626",
        rgb: hex_to_rgb(0x2860B0),
    },
    NamedColor {
        name: "Nation slot 0627",
        rgb: hex_to_rgb(0xCC7C38),
    },
    NamedColor {
        name: "Nation slot 0628",
        rgb: hex_to_rgb(0x4C7818),
    },
    NamedColor {
        name: "Nation slot 0629",
        rgb: hex_to_rgb(0x8C74AC),
    },
    NamedColor {
        name: "Nation slot 0630",
        rgb: hex_to_rgb(0x34B07C),
    },
    NamedColor {
        name: "Nation slot 0631",
        rgb: hex_to_rgb(0x804400),
    },
    NamedColor {
        name: "Nation slot 0632",
        rgb: hex_to_rgb(0xE4D0FC),
    },
    NamedColor {
        name: "Nation slot 0633",
        rgb: hex_to_rgb(0x945C8C),
    },
    NamedColor {
        name: "Nation slot 0634",
        rgb: hex_to_rgb(0x588C58),
    },
    NamedColor {
        name: "Nation slot 0635",
        rgb: hex_to_rgb(0x4854C0),
    },
    NamedColor {
        name: "Nation slot 0636",
        rgb: hex_to_rgb(0x706808),
    },
    NamedColor {
        name: "Nation slot 0637",
        rgb: hex_to_rgb(0x6C2854),
    },
    NamedColor {
        name: "Nation slot 0638",
        rgb: hex_to_rgb(0x40A0AC),
    },
    NamedColor {
        name: "Nation slot 0639",
        rgb: hex_to_rgb(0x00A884),
    },
    NamedColor {
        name: "Nation slot 0640",
        rgb: hex_to_rgb(0x4C90B8),
    },
    NamedColor {
        name: "Nation slot 0641",
        rgb: hex_to_rgb(0x94B03C),
    },
    NamedColor {
        name: "Nation slot 0642",
        rgb: hex_to_rgb(0x1C4078),
    },
    NamedColor {
        name: "Nation slot 0643",
        rgb: hex_to_rgb(0xA86C98),
    },
    NamedColor {
        name: "Nation slot 0644",
        rgb: hex_to_rgb(0x3C3484),
    },
    NamedColor {
        name: "Nation slot 0645",
        rgb: hex_to_rgb(0xC4B4F0),
    },
    NamedColor {
        name: "Nation slot 0646",
        rgb: hex_to_rgb(0x64A8D4),
    },
    NamedColor {
        name: "Nation slot 0647",
        rgb: hex_to_rgb(0x288488),
    },
    NamedColor {
        name: "Nation slot 0648",
        rgb: hex_to_rgb(0x387C10),
    },
    NamedColor {
        name: "Nation slot 0649",
        rgb: hex_to_rgb(0xC46CB8),
    },
    NamedColor {
        name: "Nation slot 0650",
        rgb: hex_to_rgb(0xF8BC38),
    },
    NamedColor {
        name: "Nation slot 0651",
        rgb: hex_to_rgb(0xC48CA4),
    },
    NamedColor {
        name: "Nation slot 0652",
        rgb: hex_to_rgb(0x24AC8C),
    },
    NamedColor {
        name: "Nation slot 0653",
        rgb: hex_to_rgb(0x9C4484),
    },
    NamedColor {
        name: "Nation slot 0654",
        rgb: hex_to_rgb(0x908C54),
    },
    NamedColor {
        name: "Nation slot 0655",
        rgb: hex_to_rgb(0xB09868),
    },
    NamedColor {
        name: "Nation slot 0656",
        rgb: hex_to_rgb(0x3C94FC),
    },
    NamedColor {
        name: "Nation slot 0657",
        rgb: hex_to_rgb(0x00A0C8),
    },
    NamedColor {
        name: "Nation slot 0658",
        rgb: hex_to_rgb(0x005880),
    },
    NamedColor {
        name: "Nation slot 0659",
        rgb: hex_to_rgb(0x683C34),
    },
    NamedColor {
        name: "Nation slot 0660",
        rgb: hex_to_rgb(0x3C5040),
    },
    NamedColor {
        name: "Nation slot 0661",
        rgb: hex_to_rgb(0x5C783C),
    },
    NamedColor {
        name: "Nation slot 0662",
        rgb: hex_to_rgb(0xC07854),
    },
    NamedColor {
        name: "Nation slot 0663",
        rgb: hex_to_rgb(0xFFB480),
    },
    NamedColor {
        name: "Nation slot 0664",
        rgb: hex_to_rgb(0x4CB0B8),
    },
    NamedColor {
        name: "Nation slot 0665",
        rgb: hex_to_rgb(0x5C3430),
    },
    NamedColor {
        name: "Nation slot 0666",
        rgb: hex_to_rgb(0x608830),
    },
    NamedColor {
        name: "Nation slot 0667",
        rgb: hex_to_rgb(0x9C685C),
    },
    NamedColor {
        name: "Nation slot 0668",
        rgb: hex_to_rgb(0xA45478),
    },
    NamedColor {
        name: "Nation slot 0669",
        rgb: hex_to_rgb(0x784030),
    },
    NamedColor {
        name: "Nation slot 0670",
        rgb: hex_to_rgb(0x1C505C),
    },
    NamedColor {
        name: "Nation slot 0671",
        rgb: hex_to_rgb(0x6078C8),
    },
    NamedColor {
        name: "Nation slot 0672",
        rgb: hex_to_rgb(0x146454),
    },
    NamedColor {
        name: "Nation slot 0673",
        rgb: hex_to_rgb(0x54288C),
    },
    NamedColor {
        name: "Nation slot 0674",
        rgb: hex_to_rgb(0xF47C54),
    },
    NamedColor {
        name: "Nation slot 0675",
        rgb: hex_to_rgb(0x60BC8C),
    },
    NamedColor {
        name: "Nation slot 0676",
        rgb: hex_to_rgb(0xAC885C),
    },
    NamedColor {
        name: "Nation slot 0677",
        rgb: hex_to_rgb(0xC48814),
    },
    NamedColor {
        name: "Nation slot 0678",
        rgb: hex_to_rgb(0x5C3C00),
    },
    NamedColor {
        name: "Nation slot 0679",
        rgb: hex_to_rgb(0x8090A4),
    },
    NamedColor {
        name: "Nation slot 0680",
        rgb: hex_to_rgb(0x185CBC),
    },
    NamedColor {
        name: "Nation slot 0681",
        rgb: hex_to_rgb(0x941C3C),
    },
    NamedColor {
        name: "Nation slot 0682",
        rgb: hex_to_rgb(0x149498),
    },
    NamedColor {
        name: "Nation slot 0683",
        rgb: hex_to_rgb(0x94789C),
    },
    NamedColor {
        name: "Nation slot 0684",
        rgb: hex_to_rgb(0xA4AC8C),
    },
    NamedColor {
        name: "Nation slot 0685",
        rgb: hex_to_rgb(0xD4B0A8),
    },
    NamedColor {
        name: "Nation slot 0686",
        rgb: hex_to_rgb(0xD8543C),
    },
    NamedColor {
        name: "Nation slot 0687",
        rgb: hex_to_rgb(0x802424),
    },
    NamedColor {
        name: "Nation slot 0688",
        rgb: hex_to_rgb(0x2C9470),
    },
    NamedColor {
        name: "Nation slot 0689",
        rgb: hex_to_rgb(0xB0C490),
    },
    NamedColor {
        name: "Nation slot 0690",
        rgb: hex_to_rgb(0x087C50),
    },
    NamedColor {
        name: "Nation slot 0691",
        rgb: hex_to_rgb(0x287414),
    },
    NamedColor {
        name: "Nation slot 0692",
        rgb: hex_to_rgb(0x8048A0),
    },
    NamedColor {
        name: "Nation slot 0693",
        rgb: hex_to_rgb(0x386094),
    },
    NamedColor {
        name: "Nation slot 0694",
        rgb: hex_to_rgb(0x60A040),
    },
    NamedColor {
        name: "Nation slot 0695",
        rgb: hex_to_rgb(0x64AC64),
    },
    NamedColor {
        name: "Nation slot 0696",
        rgb: hex_to_rgb(0x845020),
    },
    NamedColor {
        name: "Nation slot 0697",
        rgb: hex_to_rgb(0x0C680C),
    },
    NamedColor {
        name: "Nation slot 0698",
        rgb: hex_to_rgb(0xB88C48),
    },
    NamedColor {
        name: "Nation slot 0699",
        rgb: hex_to_rgb(0x3084A4),
    },
    NamedColor {
        name: "Nation slot 0700",
        rgb: hex_to_rgb(0xA04850),
    },
    NamedColor {
        name: "Nation slot 0701",
        rgb: hex_to_rgb(0xA86CB8),
    },
    NamedColor {
        name: "Nation slot 0702",
        rgb: hex_to_rgb(0x5C84AC),
    },
    NamedColor {
        name: "Nation slot 0703",
        rgb: hex_to_rgb(0x688074),
    },
    NamedColor {
        name: "Nation slot 0704",
        rgb: hex_to_rgb(0x70A430),
    },
    NamedColor {
        name: "Nation slot 0705",
        rgb: hex_to_rgb(0xE4643C),
    },
    NamedColor {
        name: "Nation slot 0706",
        rgb: hex_to_rgb(0x609468),
    },
    NamedColor {
        name: "Nation slot 0707",
        rgb: hex_to_rgb(0x8C90C4),
    },
    NamedColor {
        name: "Nation slot 0708",
        rgb: hex_to_rgb(0x7C3890),
    },
    NamedColor {
        name: "Nation slot 0709",
        rgb: hex_to_rgb(0xDC6478),
    },
    NamedColor {
        name: "Nation slot 0710",
        rgb: hex_to_rgb(0x2C6014),
    },
    NamedColor {
        name: "Nation slot 0711",
        rgb: hex_to_rgb(0xA84000),
    },
    NamedColor {
        name: "Nation slot 0712",
        rgb: hex_to_rgb(0x1CA494),
    },
    NamedColor {
        name: "Nation slot 0713",
        rgb: hex_to_rgb(0x205428),
    },
    NamedColor {
        name: "Nation slot 0714",
        rgb: hex_to_rgb(0x707090),
    },
    NamedColor {
        name: "Nation slot 0715",
        rgb: hex_to_rgb(0xA09CB4),
    },
    NamedColor {
        name: "Nation slot 0716",
        rgb: hex_to_rgb(0x783C70),
    },
    NamedColor {
        name: "Nation slot 0717",
        rgb: hex_to_rgb(0xCC68A0),
    },
    NamedColor {
        name: "Nation slot 0718",
        rgb: hex_to_rgb(0xB84C90),
    },
    NamedColor {
        name: "Nation slot 0719",
        rgb: hex_to_rgb(0x908430),
    },
    NamedColor {
        name: "Nation slot 0720",
        rgb: hex_to_rgb(0x545090),
    },
    NamedColor {
        name: "Nation slot 0721",
        rgb: hex_to_rgb(0xC880AC),
    },
    NamedColor {
        name: "Nation slot 0722",
        rgb: hex_to_rgb(0x846418),
    },
    NamedColor {
        name: "Nation slot 0723",
        rgb: hex_to_rgb(0xC84450),
    },
    NamedColor {
        name: "Nation slot 0724",
        rgb: hex_to_rgb(0x544414),
    },
    NamedColor {
        name: "Nation slot 0725",
        rgb: hex_to_rgb(0x60B098),
    },
    NamedColor {
        name: "Nation slot 0726",
        rgb: hex_to_rgb(0x0CB0D4),
    },
    NamedColor {
        name: "Nation slot 0727",
        rgb: hex_to_rgb(0x8C7838),
    },
    NamedColor {
        name: "Nation slot 0728",
        rgb: hex_to_rgb(0x607050),
    },
    NamedColor {
        name: "Nation slot 0729",
        rgb: hex_to_rgb(0x384864),
    },
    NamedColor {
        name: "Nation slot 0730",
        rgb: hex_to_rgb(0x589078),
    },
    NamedColor {
        name: "Nation slot 0731",
        rgb: hex_to_rgb(0x34705C),
    },
    NamedColor {
        name: "Nation slot 0732",
        rgb: hex_to_rgb(0x6C5898),
    },
    NamedColor {
        name: "Nation slot 0733",
        rgb: hex_to_rgb(0xB86400),
    },
    NamedColor {
        name: "Nation slot 0734",
        rgb: hex_to_rgb(0x404474),
    },
    NamedColor {
        name: "Nation slot 0735",
        rgb: hex_to_rgb(0x4C6CC4),
    },
    NamedColor {
        name: "Nation slot 0736",
        rgb: hex_to_rgb(0x185800),
    },
    NamedColor {
        name: "Nation slot 0737",
        rgb: hex_to_rgb(0xA460CC),
    },
    NamedColor {
        name: "Nation slot 0738",
        rgb: hex_to_rgb(0x543448),
    },
    NamedColor {
        name: "Nation slot 0739",
        rgb: hex_to_rgb(0xB4B038),
    },
    NamedColor {
        name: "Nation slot 0740",
        rgb: hex_to_rgb(0xF8A4AC),
    },
    NamedColor {
        name: "Nation slot 0741",
        rgb: hex_to_rgb(0x9C382C),
    },
    NamedColor {
        name: "Nation slot 0742",
        rgb: hex_to_rgb(0x843C80),
    },
    NamedColor {
        name: "Nation slot 0743",
        rgb: hex_to_rgb(0x2C5C48),
    },
    NamedColor {
        name: "Nation slot 0744",
        rgb: hex_to_rgb(0xD87474),
    },
    NamedColor {
        name: "Nation slot 0745",
        rgb: hex_to_rgb(0xA42C0C),
    },
    NamedColor {
        name: "Nation slot 0746",
        rgb: hex_to_rgb(0x4080BC),
    },
    NamedColor {
        name: "Nation slot 0747",
        rgb: hex_to_rgb(0xDC6CB0),
    },
    NamedColor {
        name: "Nation slot 0748",
        rgb: hex_to_rgb(0x8884D0),
    },
    NamedColor {
        name: "Nation slot 0749",
        rgb: hex_to_rgb(0xD0585C),
    },
    NamedColor {
        name: "Nation slot 0750",
        rgb: hex_to_rgb(0x6C90F8),
    },
    NamedColor {
        name: "Nation slot 0751",
        rgb: hex_to_rgb(0x4C6CD8),
    },
    NamedColor {
        name: "Nation slot 0752",
        rgb: hex_to_rgb(0x68B870),
    },
    NamedColor {
        name: "Nation slot 0753",
        rgb: hex_to_rgb(0x8CE488),
    },
    NamedColor {
        name: "Nation slot 0754",
        rgb: hex_to_rgb(0x3C9848),
    },
    NamedColor {
        name: "Nation slot 0755",
        rgb: hex_to_rgb(0x38843C),
    },
    NamedColor {
        name: "Nation slot 0756",
        rgb: hex_to_rgb(0xD09444),
    },
    NamedColor {
        name: "Nation slot 0757",
        rgb: hex_to_rgb(0xA0903C),
    },
    NamedColor {
        name: "Nation slot 0758",
        rgb: hex_to_rgb(0x4C8080),
    },
    NamedColor {
        name: "Nation slot 0759",
        rgb: hex_to_rgb(0x703050),
    },
    NamedColor {
        name: "Nation slot 0760",
        rgb: hex_to_rgb(0x509CC4),
    },
    NamedColor {
        name: "Nation slot 0761",
        rgb: hex_to_rgb(0x8C3C98),
    },
    NamedColor {
        name: "Nation slot 0762",
        rgb: hex_to_rgb(0x5C8014),
    },
    NamedColor {
        name: "Nation slot 0763",
        rgb: hex_to_rgb(0x305C30),
    },
    NamedColor {
        name: "Nation slot 0764",
        rgb: hex_to_rgb(0x346428),
    },
    NamedColor {
        name: "Nation slot 0765",
        rgb: hex_to_rgb(0x247870),
    },
    NamedColor {
        name: "Nation slot 0766",
        rgb: hex_to_rgb(0x883C54),
    },
    NamedColor {
        name: "Nation slot 0767",
        rgb: hex_to_rgb(0x7490C0),
    },
    NamedColor {
        name: "Nation slot 0768",
        rgb: hex_to_rgb(0xE0C484),
    },
    NamedColor {
        name: "Nation slot 0769",
        rgb: hex_to_rgb(0x847808),
    },
    NamedColor {
        name: "Nation slot 0770",
        rgb: hex_to_rgb(0xD86048),
    },
    NamedColor {
        name: "Nation slot 0771",
        rgb: hex_to_rgb(0x705000),
    },
    NamedColor {
        name: "Nation slot 0772",
        rgb: hex_to_rgb(0x7CA09C),
    },
    NamedColor {
        name: "Nation slot 0773",
        rgb: hex_to_rgb(0xAC8C08),
    },
    NamedColor {
        name: "Nation slot 0774",
        rgb: hex_to_rgb(0xC86028),
    },
    NamedColor {
        name: "Nation slot 0775",
        rgb: hex_to_rgb(0x64BC50),
    },
    NamedColor {
        name: "Nation slot 0776",
        rgb: hex_to_rgb(0x84285C),
    },
    NamedColor {
        name: "Nation slot 0777",
        rgb: hex_to_rgb(0x902C78),
    },
    NamedColor {
        name: "Nation slot 0778",
        rgb: hex_to_rgb(0xD05068),
    },
    NamedColor {
        name: "Nation slot 0779",
        rgb: hex_to_rgb(0xB83830),
    },
    NamedColor {
        name: "Nation slot 0780",
        rgb: hex_to_rgb(0x284444),
    },
    NamedColor {
        name: "Nation slot 0781",
        rgb: hex_to_rgb(0x888414),
    },
    NamedColor {
        name: "Nation slot 0782",
        rgb: hex_to_rgb(0x347488),
    },
    NamedColor {
        name: "Nation slot 0783",
        rgb: hex_to_rgb(0x689C50),
    },
    NamedColor {
        name: "Nation slot 0784",
        rgb: hex_to_rgb(0x902C40),
    },
    NamedColor {
        name: "Nation slot 0785",
        rgb: hex_to_rgb(0x8480BC),
    },
    NamedColor {
        name: "Nation slot 0786",
        rgb: hex_to_rgb(0x8068DC),
    },
    NamedColor {
        name: "Nation slot 0787",
        rgb: hex_to_rgb(0xE47CE0),
    },
    NamedColor {
        name: "Nation slot 0788",
        rgb: hex_to_rgb(0x3490B0),
    },
    NamedColor {
        name: "Nation slot 0789",
        rgb: hex_to_rgb(0xB8A468),
    },
    NamedColor {
        name: "Nation slot 0790",
        rgb: hex_to_rgb(0xC85C3C),
    },
    NamedColor {
        name: "Nation slot 0791",
        rgb: hex_to_rgb(0x7460A4),
    },
    NamedColor {
        name: "Nation slot 0792",
        rgb: hex_to_rgb(0x345C64),
    },
    NamedColor {
        name: "Nation slot 0793",
        rgb: hex_to_rgb(0x404810),
    },
    NamedColor {
        name: "Nation slot 0794",
        rgb: hex_to_rgb(0xA088C0),
    },
    NamedColor {
        name: "Nation slot 0795",
        rgb: hex_to_rgb(0xF4B0FC),
    },
    NamedColor {
        name: "Nation slot 0796",
        rgb: hex_to_rgb(0xF48C58),
    },
    NamedColor {
        name: "Nation slot 0797",
        rgb: hex_to_rgb(0xB84838),
    },
    NamedColor {
        name: "Nation slot 0798",
        rgb: hex_to_rgb(0x803024),
    },
    NamedColor {
        name: "Nation slot 0799",
        rgb: hex_to_rgb(0xB8446C),
    },
    NamedColor {
        name: "Nation slot 0800",
        rgb: hex_to_rgb(0x448C20),
    },
    NamedColor {
        name: "Nation slot 0801",
        rgb: hex_to_rgb(0xE48C80),
    },
    NamedColor {
        name: "Nation slot 0802",
        rgb: hex_to_rgb(0x2C8C78),
    },
    NamedColor {
        name: "Nation slot 0803",
        rgb: hex_to_rgb(0x7050B0),
    },
    NamedColor {
        name: "Nation slot 0804",
        rgb: hex_to_rgb(0xC47070),
    },
    NamedColor {
        name: "Nation slot 0805",
        rgb: hex_to_rgb(0x6C4464),
    },
    NamedColor {
        name: "Nation slot 0806",
        rgb: hex_to_rgb(0xA44814),
    },
    NamedColor {
        name: "Nation slot 0807",
        rgb: hex_to_rgb(0x947414),
    },
    NamedColor {
        name: "Nation slot 0808",
        rgb: hex_to_rgb(0x387440),
    },
    NamedColor {
        name: "Nation slot 0809",
        rgb: hex_to_rgb(0x0068A4),
    },
    NamedColor {
        name: "Nation slot 0810",
        rgb: hex_to_rgb(0x688404),
    },
    NamedColor {
        name: "Nation slot 0811",
        rgb: hex_to_rgb(0x085858),
    },
    NamedColor {
        name: "Nation slot 0812",
        rgb: hex_to_rgb(0x808C70),
    },
    NamedColor {
        name: "Nation slot 0813",
        rgb: hex_to_rgb(0x88A054),
    },
    NamedColor {
        name: "Nation slot 0814",
        rgb: hex_to_rgb(0x8878D8),
    },
    NamedColor {
        name: "Nation slot 0815",
        rgb: hex_to_rgb(0x1C6464),
    },
    NamedColor {
        name: "Nation slot 0816",
        rgb: hex_to_rgb(0x94A86C),
    },
    NamedColor {
        name: "Nation slot 0817",
        rgb: hex_to_rgb(0x6CFF88),
    },
    NamedColor {
        name: "Nation slot 0818",
        rgb: hex_to_rgb(0x807050),
    },
    NamedColor {
        name: "Nation slot 0819",
        rgb: hex_to_rgb(0x08783C),
    },
    NamedColor {
        name: "Nation slot 0820",
        rgb: hex_to_rgb(0xCC808C),
    },
    NamedColor {
        name: "Nation slot 0821",
        rgb: hex_to_rgb(0xBC8474),
    },
    NamedColor {
        name: "Nation slot 0822",
        rgb: hex_to_rgb(0x28BCA8),
    },
    NamedColor {
        name: "Nation slot 0823",
        rgb: hex_to_rgb(0xB05844),
    },
    NamedColor {
        name: "Nation slot 0824",
        rgb: hex_to_rgb(0xA45830),
    },
    NamedColor {
        name: "Nation slot 0825",
        rgb: hex_to_rgb(0x006C74),
    },
    NamedColor {
        name: "Nation slot 0826",
        rgb: hex_to_rgb(0x089458),
    },
    NamedColor {
        name: "Nation slot 0827",
        rgb: hex_to_rgb(0x10A86C),
    },
    NamedColor {
        name: "Nation slot 0828",
        rgb: hex_to_rgb(0x1CC498),
    },
    NamedColor {
        name: "Nation slot 0829",
        rgb: hex_to_rgb(0x4C982C),
    },
    NamedColor {
        name: "Nation slot 0830",
        rgb: hex_to_rgb(0x406048),
    },
    NamedColor {
        name: "Nation slot 0831",
        rgb: hex_to_rgb(0x5C88D4),
    },
    NamedColor {
        name: "Nation slot 0832",
        rgb: hex_to_rgb(0xC07434),
    },
    NamedColor {
        name: "Nation slot 0833",
        rgb: hex_to_rgb(0x285014),
    },
    NamedColor {
        name: "Nation slot 0834",
        rgb: hex_to_rgb(0x90688C),
    },
    NamedColor {
        name: "Nation slot 0835",
        rgb: hex_to_rgb(0x1CA4CC),
    },
    NamedColor {
        name: "Nation slot 0836",
        rgb: hex_to_rgb(0x0C6840),
    },
    NamedColor {
        name: "Nation slot 0837",
        rgb: hex_to_rgb(0x843414),
    },
    NamedColor {
        name: "Nation slot 0838",
        rgb: hex_to_rgb(0xEC8468),
    },
    NamedColor {
        name: "Nation slot 0839",
        rgb: hex_to_rgb(0x946034),
    },
    NamedColor {
        name: "Nation slot 0840",
        rgb: hex_to_rgb(0x24D4E8),
    },
    NamedColor {
        name: "Nation slot 0841",
        rgb: hex_to_rgb(0x9454C0),
    },
    NamedColor {
        name: "Nation slot 0842",
        rgb: hex_to_rgb(0x2C6488),
    },
    NamedColor {
        name: "Nation slot 0843",
        rgb: hex_to_rgb(0x749C74),
    },
    NamedColor {
        name: "Nation slot 0844",
        rgb: hex_to_rgb(0x40804C),
    },
    NamedColor {
        name: "Nation slot 0845",
        rgb: hex_to_rgb(0x707CC0),
    },
    NamedColor {
        name: "Nation slot 0846",
        rgb: hex_to_rgb(0xF49420),
    },
    NamedColor {
        name: "Nation slot 0847",
        rgb: hex_to_rgb(0xB084C8),
    },
    NamedColor {
        name: "Nation slot 0848",
        rgb: hex_to_rgb(0xBCFCE0),
    },
    NamedColor {
        name: "Nation slot 0849",
        rgb: hex_to_rgb(0x04A8E0),
    },
    NamedColor {
        name: "Nation slot 0850",
        rgb: hex_to_rgb(0x2470A0),
    },
    NamedColor {
        name: "Nation slot 0851",
        rgb: hex_to_rgb(0x2898CC),
    },
    NamedColor {
        name: "Nation slot 0852",
        rgb: hex_to_rgb(0x58648C),
    },
    NamedColor {
        name: "Nation slot 0853",
        rgb: hex_to_rgb(0x748C84),
    },
    NamedColor {
        name: "Nation slot 0854",
        rgb: hex_to_rgb(0x50B48C),
    },
    NamedColor {
        name: "Nation slot 0855",
        rgb: hex_to_rgb(0x5478B0),
    },
    NamedColor {
        name: "Nation slot 0856",
        rgb: hex_to_rgb(0x687004),
    },
    NamedColor {
        name: "Nation slot 0857",
        rgb: hex_to_rgb(0x949CCC),
    },
    NamedColor {
        name: "Nation slot 0858",
        rgb: hex_to_rgb(0x747C30),
    },
    NamedColor {
        name: "Nation slot 0859",
        rgb: hex_to_rgb(0x78C0D8),
    },
    NamedColor {
        name: "Nation slot 0860",
        rgb: hex_to_rgb(0x8C4434),
    },
    NamedColor {
        name: "Nation slot 0861",
        rgb: hex_to_rgb(0x945C58),
    },
    NamedColor {
        name: "Nation slot 0862",
        rgb: hex_to_rgb(0x38A4CC),
    },
    NamedColor {
        name: "Nation slot 0863",
        rgb: hex_to_rgb(0x54A490),
    },
    NamedColor {
        name: "Nation slot 0864",
        rgb: hex_to_rgb(0xC458A8),
    },
    NamedColor {
        name: "Nation slot 0865",
        rgb: hex_to_rgb(0xA884A0),
    },
    NamedColor {
        name: "Nation slot 0866",
        rgb: hex_to_rgb(0x684C54),
    },
    NamedColor {
        name: "Nation slot 0867",
        rgb: hex_to_rgb(0x58444C),
    },
    NamedColor {
        name: "Nation slot 0868",
        rgb: hex_to_rgb(0x9CB87C),
    },
    NamedColor {
        name: "Nation slot 0869",
        rgb: hex_to_rgb(0xB46458),
    },
    NamedColor {
        name: "Nation slot 0870",
        rgb: hex_to_rgb(0x1844A4),
    },
    NamedColor {
        name: "Nation slot 0871",
        rgb: hex_to_rgb(0xA4A838),
    },
    NamedColor {
        name: "Nation slot 0872",
        rgb: hex_to_rgb(0x941C10),
    },
    NamedColor {
        name: "Nation slot 0873",
        rgb: hex_to_rgb(0x208098),
    },
    NamedColor {
        name: "Nation slot 0874",
        rgb: hex_to_rgb(0xBC9428),
    },
    NamedColor {
        name: "Nation slot 0875",
        rgb: hex_to_rgb(0x1088B4),
    },
    NamedColor {
        name: "Nation slot 0876",
        rgb: hex_to_rgb(0x784820),
    },
    NamedColor {
        name: "Nation slot 0877",
        rgb: hex_to_rgb(0x943C10),
    },
    NamedColor {
        name: "Nation slot 0878",
        rgb: hex_to_rgb(0x60348C),
    },
    NamedColor {
        name: "Nation slot 0879",
        rgb: hex_to_rgb(0x048448),
    },
    NamedColor {
        name: "Nation slot 0880",
        rgb: hex_to_rgb(0x9CD894),
    },
    NamedColor {
        name: "Nation slot 0881",
        rgb: hex_to_rgb(0x883864),
    },
    NamedColor {
        name: "Nation slot 0882",
        rgb: hex_to_rgb(0x54ACFC),
    },
    NamedColor {
        name: "Nation slot 0883",
        rgb: hex_to_rgb(0x44685C),
    },
    NamedColor {
        name: "Nation slot 0884",
        rgb: hex_to_rgb(0x38680C),
    },
    NamedColor {
        name: "Nation slot 0885",
        rgb: hex_to_rgb(0x902420),
    },
    NamedColor {
        name: "Nation slot 0886",
        rgb: hex_to_rgb(0x046004),
    },
    NamedColor {
        name: "Nation slot 0887",
        rgb: hex_to_rgb(0x3850B8),
    },
    NamedColor {
        name: "Nation slot 0888",
        rgb: hex_to_rgb(0xA8508C),
    },
    NamedColor {
        name: "Nation slot 0889",
        rgb: hex_to_rgb(0x006428),
    },
    NamedColor {
        name: "Nation slot 0890",
        rgb: hex_to_rgb(0xC05034),
    },
    NamedColor {
        name: "Nation slot 0891",
        rgb: hex_to_rgb(0xAC7068),
    },
    NamedColor {
        name: "Nation slot 0892",
        rgb: hex_to_rgb(0x406874),
    },
    NamedColor {
        name: "Nation slot 0893",
        rgb: hex_to_rgb(0x7C7894),
    },
    NamedColor {
        name: "Nation slot 0894",
        rgb: hex_to_rgb(0xA84060),
    },
    NamedColor {
        name: "Nation slot 0895",
        rgb: hex_to_rgb(0x206000),
    },
    NamedColor {
        name: "Nation slot 0896",
        rgb: hex_to_rgb(0xB0B850),
    },
    NamedColor {
        name: "Nation slot 0897",
        rgb: hex_to_rgb(0xAC7CB8),
    },
    NamedColor {
        name: "Nation slot 0898",
        rgb: hex_to_rgb(0x987C64),
    },
    NamedColor {
        name: "Nation slot 0899",
        rgb: hex_to_rgb(0x005838),
    },
    NamedColor {
        name: "Nation slot 0900",
        rgb: hex_to_rgb(0x18CCC4),
    },
    NamedColor {
        name: "Nation slot 0901",
        rgb: hex_to_rgb(0x944048),
    },
    NamedColor {
        name: "Nation slot 0902",
        rgb: hex_to_rgb(0x946810),
    },
    NamedColor {
        name: "Nation slot 0903",
        rgb: hex_to_rgb(0xD0A848),
    },
    NamedColor {
        name: "Nation slot 0904",
        rgb: hex_to_rgb(0x044C9C),
    },
    NamedColor {
        name: "Nation slot 0905",
        rgb: hex_to_rgb(0x4070B4),
    },
    NamedColor {
        name: "Nation slot 0906",
        rgb: hex_to_rgb(0xA84828),
    },
    NamedColor {
        name: "Nation slot 0907",
        rgb: hex_to_rgb(0x68A874),
    },
    NamedColor {
        name: "Nation slot 0908",
        rgb: hex_to_rgb(0x589C60),
    },
    NamedColor {
        name: "Nation slot 0909",
        rgb: hex_to_rgb(0xC8844C),
    },
    NamedColor {
        name: "Nation slot 0910",
        rgb: hex_to_rgb(0x94505C),
    },
    NamedColor {
        name: "Nation slot 0911",
        rgb: hex_to_rgb(0x605078),
    },
    NamedColor {
        name: "Nation slot 0912",
        rgb: hex_to_rgb(0x785C7C),
    },
    NamedColor {
        name: "Nation slot 0913",
        rgb: hex_to_rgb(0x6044A4),
    },
    NamedColor {
        name: "Nation slot 0914",
        rgb: hex_to_rgb(0xAC7800),
    },
    NamedColor {
        name: "Nation slot 0915",
        rgb: hex_to_rgb(0x685464),
    },
    NamedColor {
        name: "Nation slot 0916",
        rgb: hex_to_rgb(0x5C4060),
    },
    NamedColor {
        name: "Nation slot 0917",
        rgb: hex_to_rgb(0x304078),
    },
    NamedColor {
        name: "Nation slot 0918",
        rgb: hex_to_rgb(0xACFF70),
    },
    NamedColor {
        name: "Nation slot 0919",
        rgb: hex_to_rgb(0xB0742C),
    },
    NamedColor {
        name: "Nation slot 0920",
        rgb: hex_to_rgb(0x9C848C),
    },
    NamedColor {
        name: "Nation slot 0921",
        rgb: hex_to_rgb(0x285874),
    },
    NamedColor {
        name: "Nation slot 0922",
        rgb: hex_to_rgb(0x38905C),
    },
    NamedColor {
        name: "Nation slot 0923",
        rgb: hex_to_rgb(0x9060A0),
    },
    NamedColor {
        name: "Nation slot 0924",
        rgb: hex_to_rgb(0x24A0B0),
    },
    NamedColor {
        name: "Nation slot 0925",
        rgb: hex_to_rgb(0x7038A0),
    },
    NamedColor {
        name: "Nation slot 0926",
        rgb: hex_to_rgb(0x50BCB8),
    },
    NamedColor {
        name: "Nation slot 0927",
        rgb: hex_to_rgb(0xC06068),
    },
    NamedColor {
        name: "Nation slot 0928",
        rgb: hex_to_rgb(0x146820),
    },
    NamedColor {
        name: "Nation slot 0929",
        rgb: hex_to_rgb(0xD45894),
    },
    NamedColor {
        name: "Nation slot 0930",
        rgb: hex_to_rgb(0xC88C70),
    },
    NamedColor {
        name: "Nation slot 0931",
        rgb: hex_to_rgb(0xC46010),
    },
    NamedColor {
        name: "Nation slot 0932",
        rgb: hex_to_rgb(0x006050),
    },
    NamedColor {
        name: "Nation slot 0933",
        rgb: hex_to_rgb(0x5CBCA0),
    },
    NamedColor {
        name: "Nation slot 0934",
        rgb: hex_to_rgb(0xE06C28),
    },
    NamedColor {
        name: "Nation slot 0935",
        rgb: hex_to_rgb(0x485810),
    },
    NamedColor {
        name: "Nation slot 0936",
        rgb: hex_to_rgb(0x447870),
    },
    NamedColor {
        name: "Nation slot 0937",
        rgb: hex_to_rgb(0x64ECA8),
    },
    NamedColor {
        name: "Nation slot 0938",
        rgb: hex_to_rgb(0x585C00),
    },
    NamedColor {
        name: "Nation slot 0939",
        rgb: hex_to_rgb(0x548038),
    },
    NamedColor {
        name: "Nation slot 0940",
        rgb: hex_to_rgb(0x743C48),
    },
    NamedColor {
        name: "Nation slot 0941",
        rgb: hex_to_rgb(0x44AC68),
    },
    NamedColor {
        name: "Nation slot 0942",
        rgb: hex_to_rgb(0x3470C4),
    },
    NamedColor {
        name: "Nation slot 0943",
        rgb: hex_to_rgb(0x18609C),
    },
    NamedColor {
        name: "Nation slot 0944",
        rgb: hex_to_rgb(0x987054),
    },
    NamedColor {
        name: "Nation slot 0945",
        rgb: hex_to_rgb(0x4C6810),
    },
    NamedColor {
        name: "Nation slot 0946",
        rgb: hex_to_rgb(0xB87880),
    },
    NamedColor {
        name: "Nation slot 0947",
        rgb: hex_to_rgb(0x4444A0),
    },
    NamedColor {
        name: "Nation slot 0948",
        rgb: hex_to_rgb(0xC87094),
    },
    NamedColor {
        name: "Nation slot 0949",
        rgb: hex_to_rgb(0x98804C),
    },
    NamedColor {
        name: "Nation slot 0950",
        rgb: hex_to_rgb(0x7C48B4),
    },
    NamedColor {
        name: "Nation slot 0951",
        rgb: hex_to_rgb(0xECE4AC),
    },
    NamedColor {
        name: "Nation slot 0952",
        rgb: hex_to_rgb(0x54486C),
    },
    NamedColor {
        name: "Nation slot 0953",
        rgb: hex_to_rgb(0x5C6828),
    },
    NamedColor {
        name: "Nation slot 0954",
        rgb: hex_to_rgb(0x0CC0BC),
    },
    NamedColor {
        name: "Nation slot 0955",
        rgb: hex_to_rgb(0x44C4B0),
    },
    NamedColor {
        name: "Nation slot 0956",
        rgb: hex_to_rgb(0xD07060),
    },
    NamedColor {
        name: "Nation slot 0957",
        rgb: hex_to_rgb(0x48BC74),
    },
    NamedColor {
        name: "Nation slot 0958",
        rgb: hex_to_rgb(0x444054),
    },
    NamedColor {
        name: "Nation slot 0959",
        rgb: hex_to_rgb(0x408894),
    },
    NamedColor {
        name: "Nation slot 0960",
        rgb: hex_to_rgb(0x0094C8),
    },
    NamedColor {
        name: "Nation slot 0961",
        rgb: hex_to_rgb(0xE4645C),
    },
    NamedColor {
        name: "Nation slot 0962",
        rgb: hex_to_rgb(0x246C54),
    },
    NamedColor {
        name: "Nation slot 0963",
        rgb: hex_to_rgb(0xB05410),
    },
    NamedColor {
        name: "Nation slot 0964",
        rgb: hex_to_rgb(0xBC4C68),
    },
    NamedColor {
        name: "Nation slot 0965",
        rgb: hex_to_rgb(0x882068),
    },
    NamedColor {
        name: "Nation slot 0966",
        rgb: hex_to_rgb(0xD08028),
    },
    NamedColor {
        name: "Nation slot 0967",
        rgb: hex_to_rgb(0x80B44C),
    },
    NamedColor {
        name: "Nation slot 0968",
        rgb: hex_to_rgb(0xA864A4),
    },
    NamedColor {
        name: "Nation slot 0969",
        rgb: hex_to_rgb(0x3878B0),
    },
    NamedColor {
        name: "Nation slot 0970",
        rgb: hex_to_rgb(0x6498C8),
    },
    NamedColor {
        name: "Nation slot 0971",
        rgb: hex_to_rgb(0x9874BC),
    },
    NamedColor {
        name: "Nation slot 0972",
        rgb: hex_to_rgb(0x4C5868),
    },
    NamedColor {
        name: "Nation slot 0973",
        rgb: hex_to_rgb(0x6858AC),
    },
    NamedColor {
        name: "Nation slot 0974",
        rgb: hex_to_rgb(0x64702C),
    },
    NamedColor {
        name: "Nation slot 0975",
        rgb: hex_to_rgb(0x1874C4),
    },
    NamedColor {
        name: "Nation slot 0976",
        rgb: hex_to_rgb(0x805054),
    },
    NamedColor {
        name: "Nation slot 0977",
        rgb: hex_to_rgb(0x28485C),
    },
    NamedColor {
        name: "Nation slot 0978",
        rgb: hex_to_rgb(0xA0747C),
    },
    NamedColor {
        name: "Nation slot 0979",
        rgb: hex_to_rgb(0x7C7848),
    },
    NamedColor {
        name: "Nation slot 0980",
        rgb: hex_to_rgb(0x3C5488),
    },
    NamedColor {
        name: "Nation slot 0981",
        rgb: hex_to_rgb(0xB470B0),
    },
    NamedColor {
        name: "Nation slot 0982",
        rgb: hex_to_rgb(0x742C14),
    },
    NamedColor {
        name: "Nation slot 0983",
        rgb: hex_to_rgb(0xC8FC54),
    },
    NamedColor {
        name: "Nation slot 0984",
        rgb: hex_to_rgb(0x3C3870),
    },
    NamedColor {
        name: "Nation slot 0985",
        rgb: hex_to_rgb(0x406438),
    },
    NamedColor {
        name: "Nation slot 0986",
        rgb: hex_to_rgb(0x583068),
    },
    NamedColor {
        name: "Nation slot 0987",
        rgb: hex_to_rgb(0xD06C10),
    },
    NamedColor {
        name: "Nation slot 0988",
        rgb: hex_to_rgb(0x389480),
    },
    NamedColor {
        name: "Nation slot 0989",
        rgb: hex_to_rgb(0xAC4440),
    },
    NamedColor {
        name: "Nation slot 0990",
        rgb: hex_to_rgb(0x3CB0A0),
    },
    NamedColor {
        name: "Nation slot 0991",
        rgb: hex_to_rgb(0x6C4480),
    },
    NamedColor {
        name: "Nation slot 0992",
        rgb: hex_to_rgb(0x3C7860),
    },
    NamedColor {
        name: "Nation slot 0993",
        rgb: hex_to_rgb(0xCC64BC),
    },
    NamedColor {
        name: "Nation slot 0994",
        rgb: hex_to_rgb(0xA4A448),
    },
    NamedColor {
        name: "Nation slot 0995",
        rgb: hex_to_rgb(0x603050),
    },
    NamedColor {
        name: "Nation slot 0996",
        rgb: hex_to_rgb(0x788C60),
    },
    NamedColor {
        name: "Nation slot 0997",
        rgb: hex_to_rgb(0x18BC88),
    },
    NamedColor {
        name: "Nation slot 0998",
        rgb: hex_to_rgb(0x885880),
    },
    NamedColor {
        name: "Nation slot 0999",
        rgb: hex_to_rgb(0x882800),
    },
    NamedColor {
        name: "Nation slot 1000",
        rgb: hex_to_rgb(0xC484C0),
    },
    NamedColor {
        name: "Nation slot 1001",
        rgb: hex_to_rgb(0x509048),
    },
    NamedColor {
        name: "Nation slot 1002",
        rgb: hex_to_rgb(0x94FFE0),
    },
    NamedColor {
        name: "Nation slot 1003",
        rgb: hex_to_rgb(0x50542C),
    },
    NamedColor {
        name: "Nation slot 1004",
        rgb: hex_to_rgb(0x6878D8),
    },
    NamedColor {
        name: "Nation slot 1005",
        rgb: hex_to_rgb(0x7C6888),
    },
    NamedColor {
        name: "Nation slot 1006",
        rgb: hex_to_rgb(0x709838),
    },
    NamedColor {
        name: "Nation slot 1007",
        rgb: hex_to_rgb(0xB0642C),
    },
    NamedColor {
        name: "Nation slot 1008",
        rgb: hex_to_rgb(0x987034),
    },
    NamedColor {
        name: "Nation slot 1009",
        rgb: hex_to_rgb(0x1858A4),
    },
    NamedColor {
        name: "Nation slot 1010",
        rgb: hex_to_rgb(0x985098),
    },
    NamedColor {
        name: "Nation slot 1011",
        rgb: hex_to_rgb(0x808CF0),
    },
    NamedColor {
        name: "Nation slot 1012",
        rgb: hex_to_rgb(0x188474),
    },
    NamedColor {
        name: "Nation slot 1013",
        rgb: hex_to_rgb(0xE47840),
    },
    NamedColor {
        name: "Nation slot 1014",
        rgb: hex_to_rgb(0x9CAC5C),
    },
    NamedColor {
        name: "Nation slot 1015",
        rgb: hex_to_rgb(0x708C0C),
    },
    NamedColor {
        name: "Nation slot 1016",
        rgb: hex_to_rgb(0x68A84C),
    },
    NamedColor {
        name: "Nation slot 1017",
        rgb: hex_to_rgb(0x7CB0A4),
    },
    NamedColor {
        name: "Nation slot 1018",
        rgb: hex_to_rgb(0x689C94),
    },
    NamedColor {
        name: "Nation slot 1019",
        rgb: hex_to_rgb(0x18B4C4),
    },
    NamedColor {
        name: "Nation slot 1020",
        rgb: hex_to_rgb(0x783804),
    },
    NamedColor {
        name: "Nation slot 1021",
        rgb: hex_to_rgb(0x1C7C38),
    },
    NamedColor {
        name: "Nation slot 1022",
        rgb: hex_to_rgb(0x3C5CB4),
    },
    NamedColor {
        name: "Nation slot 1023",
        rgb: hex_to_rgb(0x944878),
    },
    NamedColor {
        name: "Nation slot 1024",
        rgb: hex_to_rgb(0x6488BC),
    },
];
