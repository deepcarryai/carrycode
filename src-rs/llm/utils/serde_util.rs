use serde::{Deserialize, Deserializer};

pub fn deserialize_usize_lax<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wrapper {
        Str(String),
        Num(usize),
    }

    match Wrapper::deserialize(deserializer)? {
        Wrapper::Num(n) => Ok(n),
        Wrapper::Str(s) => s.parse::<usize>().map_err(serde::de::Error::custom),
    }
}

pub fn deserialize_usize_opt_lax<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wrapper {
        Str(String),
        Num(usize),
        None,
    }

    match Option::<Wrapper>::deserialize(deserializer)? {
        Some(Wrapper::Num(n)) => Ok(Some(n)),
        Some(Wrapper::Str(s)) => s
            .parse::<usize>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        Some(Wrapper::None) | None => Ok(None),
    }
}

pub fn deserialize_u64_lax<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wrapper {
        Str(String),
        Num(u64),
    }

    match Wrapper::deserialize(deserializer)? {
        Wrapper::Num(n) => Ok(n),
        Wrapper::Str(s) => s.parse::<u64>().map_err(serde::de::Error::custom),
    }
}

pub fn deserialize_u64_opt_lax<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wrapper {
        Str(String),
        Num(u64),
        None,
    }

    match Option::<Wrapper>::deserialize(deserializer)? {
        Some(Wrapper::Num(n)) => Ok(Some(n)),
        Some(Wrapper::Str(s)) => s
            .parse::<u64>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        Some(Wrapper::None) | None => Ok(None),
    }
}

pub fn deserialize_bool_lax<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wrapper {
        Str(String),
        Bool(bool),
    }

    match Wrapper::deserialize(deserializer)? {
        Wrapper::Bool(b) => Ok(b),
        Wrapper::Str(s) => match s.to_lowercase().as_str() {
            "true" | "yes" | "1" => Ok(true),
            "false" | "no" | "0" => Ok(false),
            _ => Err(serde::de::Error::custom(format!(
                "invalid boolean string: {}",
                s
            ))),
        },
    }
}

use serde::de::DeserializeOwned;

pub fn deserialize_vec_or_str_lax<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    T: DeserializeOwned,
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wrapper<T> {
        Vec(Vec<T>),
        Str(String),
    }

    match Wrapper::<T>::deserialize(deserializer)? {
        Wrapper::Vec(v) => Ok(v),
        Wrapper::Str(s) => serde_json::from_str(&s).map_err(serde::de::Error::custom),
    }
}
