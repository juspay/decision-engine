use crate::types::country::country_iso::CountryISO2;

/// Normalizes a free-text country name for lookup: keeps only ASCII alphanumerics and
/// uppercases them, so `"United States"`, `"UNITED STATES"` and `"UNITEDSTATES"` all collapse
/// to the same key. Must stay in sync with the normalization used to generate the match arms.
pub fn normalize(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_uppercase)
        .collect()
}

pub fn country_name_to_iso2(name: &str) -> Option<CountryISO2> {
    match normalize(name).as_str() {
        "ANDORRA" => Some(CountryISO2::AD),
        "UNITEDARABEMIRATES" => Some(CountryISO2::AE),
        "AFGHANISTAN" => Some(CountryISO2::AF),
        "ANTIGUAANDBARBUDA" | "ANTIGUABARBUDA" => Some(CountryISO2::AG),
        "ANGUILLA" => Some(CountryISO2::AI),
        "ALBANIA" => Some(CountryISO2::AL),
        "ARMENIA" => Some(CountryISO2::AM),
        "ANGOLA" => Some(CountryISO2::AO),
        "ARGENTINA" => Some(CountryISO2::AR),
        "AMERICANSAMOA" => Some(CountryISO2::AS),
        "AUSTRIA" => Some(CountryISO2::AT),
        "AUSTRALIA" => Some(CountryISO2::AU),
        "ARUBA" => Some(CountryISO2::AW),
        "AZERBAIJAN" => Some(CountryISO2::AZ),
        "BOSNIAANDHERZEGOVINA" | "BOSNIAHERZEGOVINA" => Some(CountryISO2::BA),
        "BARBADOS" => Some(CountryISO2::BB),
        "BANGLADESH" => Some(CountryISO2::BD),
        "BELGIUM" => Some(CountryISO2::BE),
        "BURKINAFASO" => Some(CountryISO2::BF),
        "BULGARIA" => Some(CountryISO2::BG),
        "BAHRAIN" => Some(CountryISO2::BH),
        "BURUNDI" => Some(CountryISO2::BI),
        "BENIN" => Some(CountryISO2::BJ),
        "BERMUDA" => Some(CountryISO2::BM),
        "BRUNEI" | "BRUNEIDARUSSALAM" => Some(CountryISO2::BN),
        "BOLIVIA" | "BOLIVIAPLURINATIONALSTATEOF" => Some(CountryISO2::BO),
        "BONAIRESINTEUSTATIUSANDSABA" | "CARIBBEANNETHERLANDS" => Some(CountryISO2::BQ),
        "BRAZIL" => Some(CountryISO2::BR),
        "BAHAMAS" => Some(CountryISO2::BS),
        "BHUTAN" => Some(CountryISO2::BT),
        "BOTSWANA" => Some(CountryISO2::BW),
        "BELARUS" => Some(CountryISO2::BY),
        "BELIZE" => Some(CountryISO2::BZ),
        "CANADA" => Some(CountryISO2::CA),
        "CONGODEMOCRATICREPUBLICOFTHE"
        | "CONGOKINSHASA"
        | "CONGOTHEDEMOCRATICREPUBLICOFTHE"
        | "DEMOCRATICREPUBLICOFTHECONGO" => Some(CountryISO2::CD),
        "CENTRALAFRICANREPUBLIC" => Some(CountryISO2::CF),
        "CONGO" | "CONGOBRAZZAVILLE" => Some(CountryISO2::CG),
        "SWITZERLAND" => Some(CountryISO2::CH),
        "COTEDIVOIRE" => Some(CountryISO2::CI),
        "COOKISLANDS" => Some(CountryISO2::CK),
        "CHILE" => Some(CountryISO2::CL),
        "CAMEROON" => Some(CountryISO2::CM),
        "CHINA" => Some(CountryISO2::CN),
        "COLOMBIA" => Some(CountryISO2::CO),
        "COSTARICA" => Some(CountryISO2::CR),
        "CUBA" => Some(CountryISO2::CU),
        "CABOVERDE" | "CAPEVERDE" => Some(CountryISO2::CV),
        "CURACAO" => Some(CountryISO2::CW),
        "CYPRUS" => Some(CountryISO2::CY),
        "CZECHIA" | "CZECHREPUBLIC" => Some(CountryISO2::CZ),
        "GERMANY" => Some(CountryISO2::DE),
        "DJIBOUTI" => Some(CountryISO2::DJ),
        "DENMARK" => Some(CountryISO2::DK),
        "DOMINICA" => Some(CountryISO2::DM),
        "DOMINICANREPUBLIC" => Some(CountryISO2::DO),
        "ALGERIA" => Some(CountryISO2::DZ),
        "ECUADOR" => Some(CountryISO2::EC),
        "ESTONIA" => Some(CountryISO2::EE),
        "EGYPT" => Some(CountryISO2::EG),
        "SPAIN" => Some(CountryISO2::ES),
        "ETHIOPIA" => Some(CountryISO2::ET),
        "FINLAND" => Some(CountryISO2::FI),
        "FIJI" => Some(CountryISO2::FJ),
        "FALKLANDISLANDSMALVINAS" => Some(CountryISO2::FK),
        "MICRONESIAFEDERATEDSTATESOF" => Some(CountryISO2::FM),
        "FRANCE" => Some(CountryISO2::FR),
        "GABON" => Some(CountryISO2::GA),
        "UNITEDKINGDOM" | "UNITEDKINGDOMOFGREATBRITAINANDNORTHERNIRELAND" => Some(CountryISO2::GB),
        "GRENADA" => Some(CountryISO2::GD),
        "GEORGIA" => Some(CountryISO2::GE),
        "GUERNSEY" => Some(CountryISO2::GG),
        "GHANA" => Some(CountryISO2::GH),
        "GIBRALTAR" => Some(CountryISO2::GI),
        "GAMBIA" => Some(CountryISO2::GM),
        "GUINEA" => Some(CountryISO2::GN),
        "GUADELOUPE" => Some(CountryISO2::GP),
        "EQUATORIALGUINEA" => Some(CountryISO2::GQ),
        "GREECE" => Some(CountryISO2::GR),
        "GUATEMALA" => Some(CountryISO2::GT),
        "GUAM" => Some(CountryISO2::GU),
        "GUINEABISSAU" => Some(CountryISO2::GW),
        "GUYANA" => Some(CountryISO2::GY),
        "HONGKONG" | "HONGKONGSARCHINA" => Some(CountryISO2::HK),
        "HONDURAS" => Some(CountryISO2::HN),
        "CROATIA" => Some(CountryISO2::HR),
        "HAITI" => Some(CountryISO2::HT),
        "HUNGARY" => Some(CountryISO2::HU),
        "INDONESIA" => Some(CountryISO2::ID),
        "IRELAND" => Some(CountryISO2::IE),
        "ISRAEL" => Some(CountryISO2::IL),
        "CARIBBEANCOUNTRIES" | "INDIA" => Some(CountryISO2::IN),
        "IRAQ" => Some(CountryISO2::IQ),
        "IRANISLAMICREPUBLICOF" => Some(CountryISO2::IR),
        "ICELAND" => Some(CountryISO2::IS),
        "ITALY" => Some(CountryISO2::IT),
        "JERSEY" => Some(CountryISO2::JE),
        "JAMAICA" => Some(CountryISO2::JM),
        "JORDAN" => Some(CountryISO2::JO),
        "JAPAN" => Some(CountryISO2::JP),
        "KENYA" => Some(CountryISO2::KE),
        "KYRGYZSTAN" => Some(CountryISO2::KG),
        "CAMBODIA" => Some(CountryISO2::KH),
        "KIRIBATI" => Some(CountryISO2::KI),
        "COMOROS" => Some(CountryISO2::KM),
        "SAINTKITTSANDNEVIS" | "STKITTSNEVIS" => Some(CountryISO2::KN),
        "KOREADEMOCRATICPEOPLESREPUBLICOF" => Some(CountryISO2::KP),
        "KOREAREPUBLICOF" | "SOUTHKOREA" => Some(CountryISO2::KR),
        "KUWAIT" => Some(CountryISO2::KW),
        "CAYMANISLANDS" => Some(CountryISO2::KY),
        "KAZAKHSTAN" => Some(CountryISO2::KZ),
        "LAOPEOPLESDEMOCRATICREPUBLIC" | "LAOS" => Some(CountryISO2::LA),
        "LEBANON" => Some(CountryISO2::LB),
        "SAINTLUCIA" | "STLUCIA" => Some(CountryISO2::LC),
        "LIECHTENSTEIN" => Some(CountryISO2::LI),
        "SRILANKA" => Some(CountryISO2::LK),
        "LIBERIA" => Some(CountryISO2::LR),
        "LESOTHO" => Some(CountryISO2::LS),
        "LITHUANIA" => Some(CountryISO2::LT),
        "LUXEMBOURG" => Some(CountryISO2::LU),
        "LATVIA" => Some(CountryISO2::LV),
        "LIBYA" | "LIBYANARABJAMAHIRIYA" => Some(CountryISO2::LY),
        "MOROCCO" => Some(CountryISO2::MA),
        "MONACO" => Some(CountryISO2::MC),
        "MOLDOVA" | "MOLDOVAREPUBLICOF" => Some(CountryISO2::MD),
        "MONTENEGRO" => Some(CountryISO2::ME),
        "MADAGASCAR" => Some(CountryISO2::MG),
        "MARSHALLISLANDS" => Some(CountryISO2::MH),
        "MACEDONIATHEFORMERYUGOSLAVREPUBLICOF" | "NORTHMACEDONIA" => Some(CountryISO2::MK),
        "MALI" => Some(CountryISO2::ML),
        "MYANMAR" | "MYANMARBURMA" => Some(CountryISO2::MM),
        "MONGOLIA" => Some(CountryISO2::MN),
        "MACAO" | "MACAOSARCHINA" => Some(CountryISO2::MO),
        "NORTHERNMARIANAISLANDS" => Some(CountryISO2::MP),
        "MARTINIQUE" => Some(CountryISO2::MQ),
        "MAURITANIA" => Some(CountryISO2::MR),
        "MONTSERRAT" => Some(CountryISO2::MS),
        "MALTA" => Some(CountryISO2::MT),
        "MAURITIUS" => Some(CountryISO2::MU),
        "MALDIVES" => Some(CountryISO2::MV),
        "MALAWI" => Some(CountryISO2::MW),
        "MEXICO" => Some(CountryISO2::MX),
        "MALAYSIA" => Some(CountryISO2::MY),
        "MOZAMBIQUE" => Some(CountryISO2::MZ),
        "NAMIBIA" => Some(CountryISO2::NA),
        "NEWCALEDONIA" => Some(CountryISO2::NC),
        "NIGER" => Some(CountryISO2::NE),
        "NIGERIA" => Some(CountryISO2::NG),
        "NICARAGUA" => Some(CountryISO2::NI),
        "NETHERLANDS" => Some(CountryISO2::NL),
        "NORWAY" => Some(CountryISO2::NO),
        "NEPAL" => Some(CountryISO2::NP),
        "NIUE" => Some(CountryISO2::NU),
        "NEWZEALAND" => Some(CountryISO2::NZ),
        "OMAN" => Some(CountryISO2::OM),
        "PANAMA" => Some(CountryISO2::PA),
        "PERU" => Some(CountryISO2::PE),
        "FRENCHPOLYNESIA" => Some(CountryISO2::PF),
        "PAPUANEWGUINEA" => Some(CountryISO2::PG),
        "PHILIPPINES" => Some(CountryISO2::PH),
        "PAKISTAN" => Some(CountryISO2::PK),
        "POLAND" => Some(CountryISO2::PL),
        "PUERTORICO" => Some(CountryISO2::PR),
        "PALESTINE"
        | "PALESTINESTATEOF"
        | "PALESTINIANTERRITORIES"
        | "PALESTINIANTERRITORYOCCUPIED" => Some(CountryISO2::PS),
        "PORTUGAL" => Some(CountryISO2::PT),
        "PALAU" => Some(CountryISO2::PW),
        "PARAGUAY" => Some(CountryISO2::PY),
        "QATAR" => Some(CountryISO2::QA),
        "KOSOVO" | "KOSOVOREPUBLICOF" => Some(CountryISO2::QZ),
        "REUNION" => Some(CountryISO2::RE),
        "ROMANIA" => Some(CountryISO2::RO),
        "SERBIA" => Some(CountryISO2::RS),
        "RUSSIA" | "RUSSIANFEDERATION" => Some(CountryISO2::RU),
        "RWANDA" => Some(CountryISO2::RW),
        "SAUDIARABIA" => Some(CountryISO2::SA),
        "SOLOMONISLANDS" => Some(CountryISO2::SB),
        "SEYCHELLES" => Some(CountryISO2::SC),
        "SUDAN" => Some(CountryISO2::SD),
        "SWEDEN" => Some(CountryISO2::SE),
        "SINGAPORE" => Some(CountryISO2::SG),
        "SAINTHELENAASCENSIONANDTRISTANDACUNHA" => Some(CountryISO2::SH),
        "SLOVENIA" => Some(CountryISO2::SI),
        "SLOVAKIA" => Some(CountryISO2::SK),
        "SIERRALEONE" => Some(CountryISO2::SL),
        "SANMARINO" => Some(CountryISO2::SM),
        "SENEGAL" => Some(CountryISO2::SN),
        "SOMALIA" => Some(CountryISO2::SO),
        "SURINAME" => Some(CountryISO2::SR),
        "SOUTHSUDAN" => Some(CountryISO2::SS),
        "SAOTOMEANDPRINCIPE" => Some(CountryISO2::ST),
        "ELSALVADOR" => Some(CountryISO2::SV),
        "SINTMAARTEN" | "SINTMAARTENDUTCHPART" => Some(CountryISO2::SX),
        "SYRIA" | "SYRIANARABREPUBLIC" => Some(CountryISO2::SY),
        "ESWATINI" | "SWAZILAND" => Some(CountryISO2::SZ),
        "TURKSANDCAICOSISLANDS" | "TURKSCAICOSISLANDS" => Some(CountryISO2::TC),
        "CHAD" => Some(CountryISO2::TD),
        "TOGO" => Some(CountryISO2::TG),
        "THAILAND" => Some(CountryISO2::TH),
        "TAJIKISTAN" => Some(CountryISO2::TJ),
        "TOKELAU" => Some(CountryISO2::TK),
        "TIMORLESTE" => Some(CountryISO2::TL),
        "TURKMENISTAN" => Some(CountryISO2::TM),
        "TUNISIA" => Some(CountryISO2::TN),
        "TONGA" => Some(CountryISO2::TO),
        "TURKEY" => Some(CountryISO2::TR),
        "TRINIDADANDTOBAGO" | "TRINIDADTOBAGO" => Some(CountryISO2::TT),
        "TAIWAN" | "TAIWANPROVINCEOFCHINA" => Some(CountryISO2::TW),
        "TANZANIA" | "TANZANIAUNITEDREPUBLICOF" => Some(CountryISO2::TZ),
        "UKRAINE" => Some(CountryISO2::UA),
        "UGANDA" => Some(CountryISO2::UG),
        "UNITEDSTATESMINOROUTLYINGISLANDS" => Some(CountryISO2::UM),
        "UNITEDSTATES" | "UNITEDSTATESOFAMERICA" => Some(CountryISO2::US),
        "URUGUAY" => Some(CountryISO2::UY),
        "UZBEKISTAN" => Some(CountryISO2::UZ),
        "HOLYSEE" | "HOLYSEEVATICANCITYSTATE" | "VATICANCITY" => Some(CountryISO2::VA),
        "SAINTVINCENTANDTHEGRENADINES" | "STVINCENTGRENADINES" => Some(CountryISO2::VC),
        "VENEZUELA" | "VENEZUELABOLIVARIANREPUBLICOF" => Some(CountryISO2::VE),
        "BRITISHVIRGINISLANDS" | "VIRGINISLANDSBRITISH" => Some(CountryISO2::VG),
        "USVIRGINISLANDS" | "VIRGINISLANDSUS" => Some(CountryISO2::VI),
        "VIETNAM" => Some(CountryISO2::VN),
        "VANUATU" => Some(CountryISO2::VU),
        "SAMOA" => Some(CountryISO2::WS),
        "YEMEN" => Some(CountryISO2::YE),
        "MAYOTTE" => Some(CountryISO2::YT),
        "SOUTHAFRICA" => Some(CountryISO2::ZA),
        "ZAMBIA" => Some(CountryISO2::ZM),
        "ZIMBABWE" => Some(CountryISO2::ZW),
        _ => None,
    }
}

/// Convenience wrapper returning the ISO alpha-2 code as a `String` (e.g. `"NL"`), or `None`
/// when the name isn't recognized. The `CountryISO2` variant name *is* the alpha-2 code.
pub fn country_name_to_iso2_code(name: &str) -> Option<String> {
    country_name_to_iso2(name).map(|c| c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_spacing_and_case_variants() {
        for name in ["NETHERLANDS", "netherlands", "Netherlands"] {
            assert_eq!(country_name_to_iso2(name), Some(CountryISO2::NL));
        }
        // Spacing variants collapse to the same code.
        for name in ["UNITED STATES", "UNITEDSTATES", "UNITED STATES OF AMERICA"] {
            assert_eq!(country_name_to_iso2_code(name).as_deref(), Some("US"));
        }
    }

    #[test]
    fn ambiguous_name_resolves_by_max_count() {
        // "CARIBBEAN COUNTRIES" appeared under IN (21) / US (9) / GB (5) → IN.
        assert_eq!(
            country_name_to_iso2("CARIBBEAN COUNTRIES"),
            Some(CountryISO2::IN)
        );
    }

    #[test]
    fn unknown_name_is_none() {
        assert_eq!(country_name_to_iso2("ATLANTIS"), None);
        assert_eq!(country_name_to_iso2(""), None);
    }

    #[test]
    fn added_codes_resolve() {
        assert_eq!(country_name_to_iso2("KOSOVO"), Some(CountryISO2::QZ));
        assert_eq!(country_name_to_iso2("TAIWAN"), Some(CountryISO2::TW));
    }

    #[test]
    fn kosovo_and_serbia_stay_distinct() {
        // Both Kosovo spellings map to QZ; only Serbia maps to RS.
        assert_eq!(country_name_to_iso2("KOSOVO"), Some(CountryISO2::QZ));
        assert_eq!(
            country_name_to_iso2("KOSOVO.REPUBLICOF"),
            Some(CountryISO2::QZ)
        );
        assert_eq!(country_name_to_iso2("SERBIA"), Some(CountryISO2::RS));
    }

    #[test]
    fn iso2_code_uses_display() {
        assert_eq!(
            country_name_to_iso2_code("NETHERLANDS").as_deref(),
            Some("NL")
        );
        assert_eq!(country_name_to_iso2_code("KOSOVO").as_deref(), Some("QZ"));
    }
}
