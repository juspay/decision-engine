use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// Implements the `ToSql` and `FromSql` traits on a type to allow it to be serialized/deserialized
/// to/from TEXT data in MySQL using `ToString`/`FromStr`.
#[macro_export]
macro_rules! impl_to_sql_from_sql_text_mysql {
    ($type:ty) => {
        impl ::diesel::serialize::ToSql<::diesel::sql_types::Text, ::diesel::mysql::Mysql>
            for $type
        {
            fn to_sql<'b>(
                &'b self,
                out: &mut ::diesel::serialize::Output<'b, '_, ::diesel::mysql::Mysql>,
            ) -> ::diesel::serialize::Result {
                use ::std::io::Write;
                out.write_all(self.to_string().as_bytes())?;
                Ok(::diesel::serialize::IsNull::No)
            }
        }

        impl ::diesel::deserialize::FromSql<::diesel::sql_types::Text, ::diesel::mysql::Mysql>
            for $type
        {
            fn from_sql(value: ::diesel::mysql::MysqlValue) -> ::diesel::deserialize::Result<Self> {
                use ::core::str::FromStr;
                let s = ::core::str::from_utf8(value.as_bytes())?;
                <$type>::from_str(s).map_err(|_| "Unrecognized enum variant".into())
            }
        }
    };
}
#[macro_export]
macro_rules! impl_to_sql_from_sql_text_pg {
    ($type:ty) => {
        impl ::diesel::serialize::ToSql<::diesel::sql_types::Text, ::diesel::pg::Pg>
            for $type
        {
            fn to_sql<'b>(
                &'b self,
                out: &mut ::diesel::serialize::Output<'b, '_, ::diesel::pg::Pg>,
            ) -> ::diesel::serialize::Result {
                use ::std::io::Write;
                out.write_all(self.to_string().as_bytes())?;
                Ok(::diesel::serialize::IsNull::No)
            }
        }

        impl ::diesel::deserialize::FromSql<::diesel::sql_types::Text, ::diesel::pg::Pg>
            for $type
        {
            fn from_sql(value: ::diesel::pg::PgValue) -> ::diesel::deserialize::Result<Self> {
                use ::core::str::FromStr;
                let s = ::core::str::from_utf8(value.as_bytes())?;
                <$type>::from_str(s).map_err(|_| "Unrecognized enum variant".into())
            }
        }
    };
}

pub fn deserialize_hashmap<'a, D, K, V>(deserializer: D) -> Result<HashMap<K, HashSet<V>>, D::Error>
where
    D: serde::Deserializer<'a>,
    K: Eq + std::str::FromStr + std::hash::Hash,
    V: Eq + std::str::FromStr + std::hash::Hash,
    <K as std::str::FromStr>::Err: std::fmt::Display,
    <V as std::str::FromStr>::Err: std::fmt::Display,
{
    use serde::de::Error;
    deserialize_hashmap_inner(<HashMap<String, String>>::deserialize(deserializer)?)
        .map_err(D::Error::custom)
}

fn deserialize_hashmap_inner<K, V>(
    value: HashMap<String, String>,
) -> Result<HashMap<K, HashSet<V>>, String>
where
    K: Eq + std::str::FromStr + std::hash::Hash,
    V: Eq + std::str::FromStr + std::hash::Hash,
    <K as std::str::FromStr>::Err: std::fmt::Display,
    <V as std::str::FromStr>::Err: std::fmt::Display,
{
    let (values, errors) = value
        .into_iter()
        .map(
            |(k, v)| match (K::from_str(k.trim()), deserialize_hashset_inner(v)) {
                (Err(error), _) => Err(format!(
                    "Unable to deserialize `{}` as `{}`: {error}",
                    k,
                    std::any::type_name::<K>()
                )),
                (_, Err(error)) => Err(error),
                (Ok(key), Ok(value)) => Ok((key, value)),
            },
        )
        .fold(
            (HashMap::new(), Vec::new()),
            |(mut values, mut errors), result| match result {
                Ok((key, value)) => {
                    values.insert(key, value);
                    (values, errors)
                }
                Err(error) => {
                    errors.push(error);
                    (values, errors)
                }
            },
        );
    if !errors.is_empty() {
        Err(format!("Some errors occurred:\n{}", errors.join("\n")))
    } else {
        Ok(values)
    }
}

fn deserialize_hashset_inner<T>(value: impl AsRef<str>) -> Result<HashSet<T>, String>
where
    T: Eq + std::str::FromStr + std::hash::Hash,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    let (values, errors) = value
        .as_ref()
        .trim()
        .split(',')
        .map(|s| {
            T::from_str(s.trim()).map_err(|error| {
                format!(
                    "Unable to deserialize `{}` as `{}`: {error}",
                    s.trim(),
                    std::any::type_name::<T>()
                )
            })
        })
        .fold(
            (HashSet::new(), Vec::new()),
            |(mut values, mut errors), result| match result {
                Ok(t) => {
                    values.insert(t);
                    (values, errors)
                }
                Err(error) => {
                    errors.push(error);
                    (values, errors)
                }
            },
        );
    if !errors.is_empty() {
        Err(format!("Some errors occurred:\n{}", errors.join("\n")))
    } else {
        Ok(values)
    }
}

pub fn deserialize_hashset<'a, D, T>(deserializer: D) -> Result<HashSet<T>, D::Error>
where
    D: serde::Deserializer<'a>,
    T: Eq + std::str::FromStr + std::hash::Hash,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    use serde::de::Error;

    deserialize_hashset_inner(<String>::deserialize(deserializer)?).map_err(D::Error::custom)
}
