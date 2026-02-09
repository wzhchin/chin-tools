use std::fmt::Display;

use chin_tools_types::SharedStr;
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{ChinSqlError, SqlValue};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Varchar<const LIMIT: usize>(pub(crate) SharedStr);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Text(pub(crate) SharedStr);

impl<const LIMIT: usize> Serialize for Varchar<LIMIT> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de, const LIMIT: usize> Deserialize<'de> for Varchar<LIMIT> {
    fn deserialize<D>(deserializer: D) -> Result<Varchar<LIMIT>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let o: String = String::deserialize(deserializer)?;
        if o.len() > LIMIT {
            return Err(de::Error::custom(format!(
                "out of ranch: {} > {}",
                o.len(),
                LIMIT
            )));
        }
        Ok(Self(o.into()))
    }
}

impl<const LIMIT: usize> TryFrom<String> for Varchar<LIMIT> {
    type Error = ChinSqlError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() > LIMIT {
            return Err(ChinSqlError::TransformError(format!(
                "out of ranch: {} > {}",
                value.len(),
                LIMIT
            )));
        }

        Ok(Self(SharedStr::from(value)))
    }
}

impl<const LIMIT: usize> TryFrom<&'static str> for Varchar<LIMIT> {
    type Error = ChinSqlError;

    fn try_from(value: &'static str) -> Result<Self, Self::Error> {
        if value.len() > LIMIT {
            return Err(ChinSqlError::TransformError(format!(
                "out of ranch: {} > {}",
                value.len(),
                LIMIT
            )));
        }

        Ok(Self(SharedStr::from(value)))
    }
}

impl<const LIMIT: usize> From<Varchar<LIMIT>> for String {
    fn from(value: Varchar<LIMIT>) -> Self {
        value.0.to_string()
    }
}

impl Serialize for Text {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D>(deserializer: D) -> Result<Text, D::Error>
    where
        D: Deserializer<'de>,
    {
        let o: String = String::deserialize(deserializer)?;
        Ok(Self(o.into()))
    }
}

impl From<String> for Text {
    fn from(value: String) -> Self {
        Self(SharedStr::from(value))
    }
}

impl From<Text> for String {
    fn from(value: Text) -> Self {
        value.0.to_string()
    }
}

impl<const LIMIT: usize> Varchar<LIMIT> {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn limit<S: AsRef<str>>(s: S) -> Self {
        Self(s.as_ref()[0..LIMIT].into())
    }
}

impl Text {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<const LIMIT: usize> Display for Varchar<LIMIT> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Display for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'a, const LIMIT: usize> TryFrom<SqlValue<'a>> for Varchar<LIMIT> {
    type Error = ChinSqlError;

    fn try_from(value: SqlValue<'a>) -> Result<Self, Self::Error> {
        match value {
            SqlValue::Str(cow) => {
                let s = cow.to_string();
                Ok(s.try_into()?)
            }
            other => Err(ChinSqlError::TransformError(format!(
                "Cannot transform {other:?} into Varchar"
            ))),
        }
    }
}
