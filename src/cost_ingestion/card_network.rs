//! Card networks as cost data keys them.
//!
//! Costs are learned from connector reports and looked up for live payments, and both sides key them
//! by network — so a report's `American Express` and a request's `AmericanExpress` must produce the
//! same key. Every parser and every lookup normalises through [`network_key`].

/// A card network cost data recognises by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CardNetwork {
    Visa,
    Mastercard,
    Amex,
    Discover,
    Diners,
    Jcb,
}

impl CardNetwork {
    /// The network a name refers to, in any casing and with or without spaces: `MasterCard`,
    /// `master card`, `mc` and `master` are all [`CardNetwork::Mastercard`].
    pub fn from_name(raw: &str) -> Option<Self> {
        match compact(raw).as_str() {
            "visa" => Some(Self::Visa),
            "mastercard" | "master" | "mc" => Some(Self::Mastercard),
            "amex" | "americanexpress" => Some(Self::Amex),
            "discover" => Some(Self::Discover),
            "diners" | "dinersclub" => Some(Self::Diners),
            "jcb" => Some(Self::Jcb),
            _ => None,
        }
    }

    /// The network a Chase method-of-payment code (`VI`, `MC`, `AX`, …) refers to.
    pub fn from_chase_code(code: &str) -> Option<Self> {
        match code.trim().to_uppercase().as_str() {
            "VI" => Some(Self::Visa),
            "MC" => Some(Self::Mastercard),
            "AX" => Some(Self::Amex),
            "DI" => Some(Self::Discover),
            "JC" => Some(Self::Jcb),
            "DC" => Some(Self::Diners),
            _ => None,
        }
    }

    /// The key cost data stores this network under.
    pub fn key(self) -> &'static str {
        match self {
            Self::Visa => "visa",
            Self::Mastercard => "mc",
            Self::Amex => "amex",
            Self::Discover => "discover",
            Self::Diners => "diners",
            Self::Jcb => "jcb",
        }
    }
}

/// The key for a network named `raw`: a known network's key, or for any other network its name
/// lowercased without spaces — so a network not listed above still keys the same on both sides.
pub fn network_key(raw: &str) -> String {
    match CardNetwork::from_name(raw) {
        Some(network) => network.key().to_string(),
        None => compact(raw),
    }
}

/// The key for a Chase method-of-payment code, falling back to the code as a name.
pub fn chase_network_key(code: &str) -> String {
    match CardNetwork::from_chase_code(code) {
        Some(network) => network.key().to_string(),
        None => network_key(code),
    }
}

fn compact(raw: &str) -> String {
    raw.trim().to_lowercase().replace(' ', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spelling_of_a_network_has_one_key() {
        for (spellings, key) in [
            (&["Visa", "VISA", " visa "][..], "visa"),
            (
                &[
                    "MasterCard",
                    "Mastercard",
                    "master card",
                    "Master",
                    "mc",
                    "MC",
                ][..],
                "mc",
            ),
            (
                &[
                    "American Express",
                    "AmericanExpress",
                    "americanexpress",
                    "AMEX",
                    "amex",
                ][..],
                "amex",
            ),
            (&["Discover", "DISCOVER"][..], "discover"),
            (
                &["Diners Club", "DinersClub", "dinersclub", "Diners"][..],
                "diners",
            ),
            (&["JCB", "jcb"][..], "jcb"),
        ] {
            for spelling in spellings {
                assert_eq!(network_key(spelling), key, "{spelling:?}");
            }
        }
    }

    #[test]
    fn an_unlisted_network_keys_by_its_compacted_name() {
        assert_eq!(network_key("UnionPay"), "unionpay");
        assert_eq!(network_key("Cartes Bancaires"), "cartesbancaires");
        assert_eq!(network_key(""), "");
    }

    #[test]
    fn chase_codes_share_the_name_keys() {
        assert_eq!(chase_network_key("VI"), network_key("Visa"));
        assert_eq!(chase_network_key("mc"), network_key("Mastercard"));
        assert_eq!(chase_network_key("AX"), network_key("American Express"));
        assert_eq!(chase_network_key("DC"), network_key("Diners Club"));
        assert_eq!(chase_network_key("JC"), network_key("JCB"));
        assert_eq!(chase_network_key("DI"), network_key("Discover"));
        assert_eq!(chase_network_key(""), "");
        assert_eq!(chase_network_key("ZZ"), "zz");
    }
}
