use std::str::FromStr;

use serde::{Deserialize, de::Visitor};

#[derive(Clone, Debug, Deserialize)]
pub struct MemoryLimits {
    #[serde(default)]
    pub memory_size_bytes: MemoryLimitBytes,
    #[serde(default)]
    pub table_elements: TableLimit,
    #[serde(default)]
    pub instances: ResourceLimit,
    #[serde(default)]
    pub tables: ResourceLimit,
    #[serde(default)]
    pub memories: ResourceLimit,
    #[serde(default)]
    pub trap_on_grow_failure: bool,
    pub stdout_bytes: MemorySizeBytes,
    pub stderr_bytes: MemorySizeBytes,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            memory_size_bytes: Default::default(),
            table_elements: Default::default(),
            instances: Default::default(),
            tables: Default::default(),
            memories: Default::default(),
            trap_on_grow_failure: Default::default(),
            stdout_bytes: Default::default(),
            stderr_bytes: Default::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemorySizeBytes(usize);
impl Default for MemorySizeBytes {
    fn default() -> Self {
        MemorySizeBytes(10 * 1024 * 1024) // 10 MB
    }
}
impl From<usize> for MemorySizeBytes {
    fn from(value: usize) -> Self {
        MemorySizeBytes(value)
    }
}
impl Into<usize> for MemorySizeBytes {
    fn into(self) -> usize {
        self.0
    }
}
impl FromStr for MemorySizeBytes {
    type Err = ();
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut result: usize = 0;
        let mut multiple: usize = 1;
        let mut got_end_suffix = false;
        for char in value.chars() {
            if got_end_suffix {
                return Err(());
            }
            if char == 'b' || char == 'B' {
                got_end_suffix = true;
            } else if multiple != 1 {
                return Err(());
            } else {
                match char {
                    'G' | 'g' => {
                        multiple = 1024 * 1024 * 1024;
                    }
                    'M' | 'm' => {
                        multiple = 1024 * 1024;
                    }
                    'K' | 'k' => {
                        multiple = 1024;
                    }
                    ',' | '_' | ' ' => {}
                    _ => {
                        if let Some(digit) = char.to_digit(10) {
                            result = result
                                .checked_mul(10)
                                .and_then(|r| r.checked_add(digit as usize))
                                .ok_or_else(|| ())?;
                        } else {
                            return Err(());
                        }
                    }
                }
            }
        }
        if result > 0 && (multiple == 1 || got_end_suffix) {
            Ok(MemorySizeBytes(result * multiple))
        } else {
            Err(())
        }
    }
}

impl<'de> Deserialize<'de> for MemorySizeBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MemorySizeBytesVisitor;
        impl<'de> Visitor<'de> for MemorySizeBytesVisitor {
            type Value = MemorySizeBytes;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a positive integer or a string like '128MB'")
            }
            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value <= 0 {
                    return Err(E::custom("memory size must be greater than zero"));
                }
                Ok(MemorySizeBytes(value as usize))
            }
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value == 0 {
                    return Err(E::custom("memory size must be greater than zero"));
                }
                Ok(MemorySizeBytes(value as usize))
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value.parse().map_err(|_| {
                    E::custom("expected 'UNBOUNDED' or a positive integer or a string like '128MB'")
                })
            }
        }
        deserializer.deserialize_any(MemorySizeBytesVisitor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryLimitBytes {
    Limited(usize),
    Unbounded,
}

impl Default for MemoryLimitBytes {
    fn default() -> Self {
        MemoryLimitBytes::Limited(128 * 1024 * 1024) // 128 MB
    }
}

impl From<usize> for MemoryLimitBytes {
    fn from(value: usize) -> Self {
        MemoryLimitBytes::Limited(value)
    }
}

impl FromStr for MemoryLimitBytes {
    type Err = ();
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "UNBOUNDED" {
            Ok(MemoryLimitBytes::Unbounded)
        } else {
            let MemorySizeBytes(limit) = value.parse()?;
            Ok(MemoryLimitBytes::Limited(limit))
        }
    }
}

impl<'de> Deserialize<'de> for MemoryLimitBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MemorySizeBytesVisitor;
        impl<'de> Visitor<'de> for MemorySizeBytesVisitor {
            type Value = MemoryLimitBytes;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "a positive integer or the string 'UNBOUNDED' or a string like '128MB'",
                )
            }
            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value <= 0 {
                    return Err(E::custom("memory size must be greater than zero"));
                }
                Ok(MemoryLimitBytes::Limited(value as usize))
            }
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value == 0 {
                    return Err(E::custom("memory size must be greater than zero"));
                }
                Ok(MemoryLimitBytes::Limited(value as usize))
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value.parse().map_err(|_| {
                    E::custom("expected 'UNBOUNDED' or a positive integer or a string like '128MB'")
                })
            }
        }
        deserializer.deserialize_any(MemorySizeBytesVisitor)
    }
}

#[test]
fn test_memory_size_bytes_deserialize() {
    let limited: MemoryLimitBytes =
        serde_json::from_str("\"1,000KB\"").expect("failed to deserialize limited memory size");
    assert_eq!(limited, MemoryLimitBytes::Limited(1024000));
    let limited: MemoryLimitBytes =
        serde_json::from_str("1048576").expect("failed to deserialize limited memory size");
    assert_eq!(limited, MemoryLimitBytes::Limited(1048576));
    let unbounded: MemoryLimitBytes = serde_json::from_str(r#""UNBOUNDED""#)
        .expect("failed to deserialize unbounded memory size");
    assert_eq!(unbounded, MemoryLimitBytes::Unbounded);

    let unbounded: MemoryLimitBytes = "UNBOUNDED"
        .parse()
        .expect("failed to parse unbounded memory size");
    assert_eq!(unbounded, MemoryLimitBytes::Unbounded);

    let limited: MemoryLimitBytes = "128MB"
        .parse()
        .expect("failed to parse limited memory size");
    assert_eq!(limited, MemoryLimitBytes::Limited(128 * 1024 * 1024));

    let limited: MemoryLimitBytes = "1024".parse().expect("failed to parse limited memory size");
    assert_eq!(limited, MemoryLimitBytes::Limited(1024));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableLimit {
    Limited(usize),
    Unbounded,
}

impl Default for TableLimit {
    fn default() -> Self {
        TableLimit::Limited(100_000)
    }
}

impl From<usize> for TableLimit {
    fn from(value: usize) -> Self {
        TableLimit::Limited(value)
    }
}

impl FromStr for TableLimit {
    type Err = ();
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "UNBOUNDED" {
            Ok(TableLimit::Unbounded)
        } else {
            let ResourceLimit(limit) = value.parse()?;
            Ok(TableLimit::Limited(limit))
        }
    }
}

impl<'de> Deserialize<'de> for TableLimit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TableLimitVisitor;
        impl<'de> Visitor<'de> for TableLimitVisitor {
            type Value = TableLimit;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a positive integer or the string 'UNBOUNDED'")
            }
            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value <= 0 {
                    return Err(E::custom("table limit must be greater than zero"));
                }
                Ok(TableLimit::Limited(value as usize))
            }
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value == 0 {
                    return Err(E::custom("table limit must be greater than zero"));
                }
                Ok(TableLimit::Limited(value as usize))
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value
                    .parse()
                    .map_err(|_| E::custom("expected 'UNBOUNDED' or a positive integer"))
            }
        }
        deserializer.deserialize_any(TableLimitVisitor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceLimit(usize);
impl From<usize> for ResourceLimit {
    fn from(value: usize) -> Self {
        ResourceLimit(value)
    }
}
impl Into<usize> for ResourceLimit {
    fn into(self) -> usize {
        self.0
    }
}

impl Default for ResourceLimit {
    fn default() -> Self {
        ResourceLimit(10_000)
    }
}

impl FromStr for ResourceLimit {
    type Err = ();
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut result: usize = 0;
        for char in value.chars() {
            match char {
                ',' | '_' | ' ' => {}
                _ => {
                    if let Some(digit) = char.to_digit(10) {
                        result = result
                            .checked_mul(10)
                            .and_then(|r| r.checked_add(digit as usize))
                            .ok_or_else(|| ())?;
                    } else {
                        return Err(());
                    }
                }
            }
        }
        if result > 0 {
            Ok(ResourceLimit(result))
        } else {
            Err(())
        }
    }
}

impl<'de> Deserialize<'de> for ResourceLimit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MemoryLimitVisitor;
        impl<'de> Visitor<'de> for MemoryLimitVisitor {
            type Value = ResourceLimit;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a positive integer")
            }
            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value <= 0 {
                    return Err(E::custom("limit must be greater than zero"));
                }
                Ok(ResourceLimit(value as usize))
            }
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value == 0 {
                    return Err(E::custom("limit must be greater than zero"));
                }
                Ok(ResourceLimit(value as usize))
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value
                    .parse()
                    .map_err(|_| E::custom("expected a positive integer"))
            }
        }
        deserializer.deserialize_any(MemoryLimitVisitor)
    }
}
