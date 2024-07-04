use crate::error::Error;

pub const HARDENED: u32 = 0x80000000;

#[derive(Debug, Clone, PartialEq)]
pub struct Keypath(Vec<u32>);

impl Keypath {
    pub fn to_vec(&self) -> Vec<u32> {
        self.0.clone()
    }

    pub(crate) fn hardened_prefix(&self) -> Keypath {
        Keypath(
            self.0
                .iter()
                .cloned()
                .take_while(|&el| el >= HARDENED)
                .collect(),
        )
    }
}

fn parse_bip32_keypath(keypath: &str) -> Option<Vec<u32>> {
    let keypath = keypath.strip_prefix("m/")?;
    if keypath.is_empty() {
        return Some(vec![]);
    }
    let parts: Vec<&str> = keypath.split('/').collect();
    let mut res = Vec::new();

    for part in parts {
        let mut add_prime = 0;
        let number = if part.ends_with('\'') {
            add_prime = HARDENED;
            part[0..part.len() - 1].parse::<u32>()
        } else {
            part.parse::<u32>()
        };

        match number {
            Ok(n) if n < HARDENED => {
                res.push(n + add_prime);
            }
            _ => return None,
        }
    }

    Some(res)
}

impl TryFrom<&str> for Keypath {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Keypath(
            parse_bip32_keypath(value).ok_or(Error::KeypathParse(value.into()))?,
        ))
    }
}

impl From<&bitcoin::bip32::DerivationPath> for Keypath {
    fn from(value: &bitcoin::bip32::DerivationPath) -> Self {
        Keypath(value.to_u32_vec())
    }
}

impl From<&Keypath> for crate::pb::Keypath {
    fn from(value: &Keypath) -> Self {
        crate::pb::Keypath {
            keypath: value.to_vec(),
        }
    }
}

#[cfg(feature = "wasm")]
impl<'de> serde::Deserialize<'de> for Keypath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct KeypathVisitor;

        impl<'de> serde::de::Visitor<'de> for KeypathVisitor {
            type Value = Keypath;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or a number sequence")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value.try_into().map_err(serde::de::Error::custom)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut vec = Vec::<u32>::new();
                while let Some(elem) = seq.next_element()? {
                    vec.push(elem);
                }
                Ok(Keypath(vec))
            }
        }

        deserializer.deserialize_any(KeypathVisitor)
    }
}

#[cfg(feature = "wasm")]
pub fn serde_deserialize<'de, D>(deserializer: D) -> Result<Vec<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    Ok(Keypath::deserialize(deserializer)?.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bip32_keypath() {
        // Test regular cases
        assert_eq!(parse_bip32_keypath("m/44/0/0/0"), Some(vec![44, 0, 0, 0]));
        assert_eq!(
            parse_bip32_keypath("m/44'/0'/0'/0'"),
            Some(vec![HARDENED + 44, HARDENED, HARDENED, HARDENED])
        );

        // Test edge cases
        assert_eq!(parse_bip32_keypath("m/0/0/0"), Some(vec![0, 0, 0]));
        assert_eq!(
            parse_bip32_keypath("m/0'/0'/0'"),
            Some(vec![HARDENED, HARDENED, HARDENED])
        );
        assert_eq!(
            parse_bip32_keypath("m/2147483647/2147483647/2147483647"),
            Some(vec![2147483647, 2147483647, 2147483647])
        );
        assert_eq!(
            parse_bip32_keypath("m/2147483647'/2147483647'/2147483647'"),
            Some(vec![
                HARDENED + 2147483647,
                HARDENED + 2147483647,
                HARDENED + 2147483647
            ])
        );
        assert_eq!(parse_bip32_keypath("m/"), Some(vec![]));

        // Test failure cases
        assert_eq!(parse_bip32_keypath("m/2147483648/0/0"), None);
        assert_eq!(parse_bip32_keypath("m/0/2147483648/0"), None);
        assert_eq!(parse_bip32_keypath("m/0/0/2147483648"), None);
        assert_eq!(parse_bip32_keypath("m/2147483648'/0/0"), None);
        assert_eq!(parse_bip32_keypath("m/0/2147483648'/0"), None);
        assert_eq!(parse_bip32_keypath("m/0/0/2147483648'"), None);
        assert_eq!(parse_bip32_keypath("m/abcd/0/0"), None);
        assert_eq!(parse_bip32_keypath("m/0'/abcd'/0'"), None);
        assert_eq!(parse_bip32_keypath("m/0/0'/abcd'"), None);
        assert_eq!(parse_bip32_keypath("m//0/0"), None);
        assert_eq!(parse_bip32_keypath("m/0//0"), None);
        assert_eq!(parse_bip32_keypath("m/0/0//"), None);
        assert_eq!(parse_bip32_keypath("/0/0/0"), None);
        assert_eq!(parse_bip32_keypath("44/0/0/0"), None);
    }

    #[test]
    fn test_from_derivation_path() {
        let derivation_path: bitcoin::bip32::DerivationPath =
            std::str::FromStr::from_str("m/84'/0'/0'/0/1").unwrap();
        let keypath = Keypath::from(&derivation_path);
        assert_eq!(
            keypath.to_vec().as_slice(),
            &[84 + HARDENED, HARDENED, HARDENED, 0, 1]
        );
    }
}
