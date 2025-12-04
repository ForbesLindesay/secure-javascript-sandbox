use std::str::FromStr;
use serde::{Deserialize, de::Visitor};

enum Multiple {
    Killo,
    Mega,
    Giga,
}
impl TryFrom<char> for Multiple {
    type Error = ();
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            'K' | 'k' => Ok(Multiple::Killo),
            'M' | 'm' => Ok(Multiple::Mega),
            'G' | 'g' => Ok(Multiple::Giga),
            _ => Err(()),
        }
    }
}

trait ValueSuffix: Default {
    fn add_char(&mut self, c: char) -> Result<(), ()>;
    fn finalize(self) -> Result<u32, ()>;
}
struct MemorySuffix {
    multiple: Option<Multiple>,
    done: bool,
}
impl Default for MemorySuffix {
    fn default() -> Self {
        MemorySuffix {
            multiple: None,
            done: false,
        }
    }
}
impl ValueSuffix for MemorySuffix {
    fn add_char(&mut self, c: char) -> Result<(), ()> {
        if self.done {
            Err(())
        } else if c == 'B' || c == 'b' {
            self.done = true;
            Ok(())
        } else if self.multiple.is_some() {
            Err(())
        } else {
            let m = Multiple::try_from(c)?;
            self.multiple = Some(m);
            Ok(())
        }
    }
    fn finalize(self) -> Result<u32, ()> {
        if self.done {
            match self.multiple {
                Some(multiple) => {
                    let multiple = match multiple {
                        Multiple::Killo => 1024,
                        Multiple::Mega => 1024 * 1024,
                        Multiple::Giga => 1024 * 1024 * 1024,
                    };
                    Ok(multiple)
                }
                None => Ok(1),
            }
        } else {
            match self.multiple {
                Some(_) => Err(()),
                None => Ok(1),
            }
        }
    }
}

struct NoSuffix;
impl Default for NoSuffix {
    fn default() -> Self {
        NoSuffix
    }
}
impl ValueSuffix for NoSuffix {
    fn add_char(&mut self, _c: char) -> Result<(), ()> {
        Err(())
    }
    fn finalize(self) -> Result<u32, ()> {
        Ok(1)
    }
}

trait Number {
    type Value: Copy + std::fmt::Debug + Eq;
    fn zero() -> Self::Value;
    fn checked_mul(value: Self::Value, factor: u32) -> Option<Self::Value>;
    fn checked_add_digit(value: Self::Value, digit: u32) -> Option<Self::Value>;
    #[cfg(test)]
    fn from_usize(value: usize) -> Result<Self::Value, ()>;
}
impl Number for usize {
    type Value = usize;
    #[inline]
    fn zero() -> Self::Value {
        0
    }
    #[inline]
    fn checked_mul(value: Self::Value, factor: u32) -> Option<Self::Value> {
        value.checked_mul(factor as usize)
    }
    #[inline]
    fn checked_add_digit(value: Self::Value, digit: u32) -> Option<Self::Value> {
        value.checked_add(digit as usize)
    }
    #[cfg(test)]
    fn from_usize(value: usize) -> Result<Self::Value, ()> {
        Ok(value)
    }
}
impl Number for u64 {
    type Value = u64;
    #[inline]
    fn zero() -> Self::Value {
        0
    }
    #[inline]
    fn checked_mul(value: Self::Value, factor: u32) -> Option<Self::Value> {
        value.checked_mul(factor as u64)
    }
    #[inline]
    fn checked_add_digit(value: Self::Value, digit: u32) -> Option<Self::Value> {
        value.checked_add(digit as u64)
    }
    #[cfg(test)]
    fn from_usize(value: usize) -> Result<Self::Value, ()> {
        Ok(value as u64)
    }
}

fn add_digit<TNumber: Number>(value: TNumber::Value, digit: char) -> Result<TNumber::Value, ()> {
    digit.to_digit(10).and_then(|digit| {
        TNumber::checked_mul(value, 10)
            .and_then(|r| TNumber::checked_add_digit(r, digit))
    }).ok_or(())
}
fn parse_number<TSuffix: ValueSuffix, TResult: Number>(value: &str) -> Result<TResult::Value, ()> {
    let mut suffix = TSuffix::default();
    let mut result = TResult::zero();
    let mut last_char = None;
    let mut started_suffix = false;
    for char in value.chars() {
        // println!("last_char: {:?}", last_char);
        // println!("char: {:?}", char);
        // println!("started_suffix: {:?}", started_suffix);
        // println!("result: {:?}", result);
        if let Some(last_char) = last_char {
            if started_suffix {
                // Once we've started the suffix, we can only accept suffix characters
                suffix.add_char(char)?;
            } else if result == TResult::zero() {
                // We have a char and it's zero, so it should be the only char other than an optional suffix
                started_suffix = true;
                if char != ' ' {
                    suffix.add_char(char)?;
                }
            } else if last_char == ',' || last_char == '_' {
                // After a separator, we require a digit
                result = add_digit::<TResult>(result, char)?;
            } else if char == ' ' {
                // If there's a space, that must start the suffix
                started_suffix = true;
            } else if char == ',' || char == '_' {
                // Separators are allowed between digits
            } else {
                match add_digit::<TResult>(result, char) {
                    Ok(new_result) => {
                        result = new_result;
                    }
                    Err(()) => {
                        // If we fail to add a digit, that must mean the suffix is starting
                        started_suffix = true;
                        suffix.add_char(char)?;
                    }
                }
            }
        } else {
            // Disallow suffix or separators without any digits
            result = add_digit::<TResult>(result, char)?;
        }
        last_char = Some(char);
    }
    if last_char.is_none_or(|c| c == ',' || c == '_' || c == ' ') {
        return Err(());
    }
    TResult::checked_mul(result, suffix.finalize()?).ok_or(())
}

#[cfg(test)]
fn test_parse_number_without_suffix<TSuffix: ValueSuffix, T: Number>() {
    assert_eq!(parse_number::<TSuffix, T>("0"), T::from_usize(0));
    assert_eq!(parse_number::<TSuffix, T>("1"), T::from_usize(1));
    assert_eq!(parse_number::<TSuffix, T>("12,345"), T::from_usize(12_345));
    assert_eq!(parse_number::<TSuffix, T>("1_000_000"), T::from_usize(1_000_000));
    assert!(parse_number::<TSuffix, T>("").is_err(), "expected error parsing empty string");
    assert!(parse_number::<TSuffix, T>(",100").is_err(), "expected error parsing string starting with separator");
    assert!(parse_number::<TSuffix, T>("100,").is_err(), "expected error parsing string ending with separator");
    assert!(parse_number::<TSuffix, T>("01").is_err(), "expected error parsing string with leading 0");
    assert!(parse_number::<TSuffix, T>("1 ").is_err(), "expected error parsing string with trailing space");
}
#[cfg(test)]
fn test_parse_number_t<T: Number>() {
    test_parse_number_without_suffix::<NoSuffix, T>();
    test_parse_number_without_suffix::<MemorySuffix, T>();

    assert_eq!(parse_number::<MemorySuffix, T>("0B"), T::from_usize(0));
    assert_eq!(parse_number::<MemorySuffix, T>("0MB"), T::from_usize(0));
    assert_eq!(parse_number::<MemorySuffix, T>("1B"), T::from_usize(1));
    assert_eq!(parse_number::<MemorySuffix, T>("1KB"), T::from_usize(1024));
    assert_eq!(parse_number::<MemorySuffix, T>("1 KB"), T::from_usize(1024));
    assert_eq!(parse_number::<MemorySuffix, T>("1,000 KB"), T::from_usize(1024_000));
    assert!(parse_number::<MemorySuffix, T>("1 K").is_err(), "expected error parsing string with incomplete suffix");
    assert!(parse_number::<MemorySuffix, T>("1 KB ").is_err(), "expected error parsing string with space after suffix");
    assert!(parse_number::<MemorySuffix, T>("1 KB,").is_err(), "expected error parsing string with separator after suffix");
}

#[test]
fn test_parse_number() {
    test_parse_number_t::<usize>();
    test_parse_number_t::<u64>();
}

macro_rules! number_type {
    ($id:ident, $underlying:ident, $suffix:ident, name=$name:expr, expect=$expected:expr, default=$default_value:expr, min=$min_value:expr) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $id(pub $underlying);
        impl Default for $id {
            fn default() -> Self {
                Self($default_value)
            }
        }
        impl From<$underlying> for $id {
            fn from(value: $underlying) -> Self {
                Self(value)
            }
        }
        impl Into<$underlying> for $id {
            fn into(self) -> $underlying {
                self.0
            }
        }
        impl FromStr for $id {
            type Err = ();
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                let limit = parse_number::<$suffix, $underlying>(value)?;
                if limit < $min_value {
                    return Err(());
                }
                Ok(Self(limit))
            }
        }
        impl<'de> Deserialize<'de> for $id {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct SelfVisitor;
                impl<'de> Visitor<'de> for SelfVisitor {
                    type Value = $id;
                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str($expected)
                    }
                    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        if value < $min_value {
                            return Err(E::custom(format!("{} must be at least {}", $name, $min_value)));
                        }
                        Ok($id(value as $underlying))
                    }
                    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        if value < $min_value {
                            return Err(E::custom(format!("{} must be at least {}", $name, $min_value)));
                        }
                        Ok($id(value as $underlying))
                    }
                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        value.parse().map_err(|_| {
                            E::custom(format!("expected {}", $expected))
                        })
                    }
                }
                deserializer.deserialize_any(SelfVisitor)
            }
        }

    };
}

macro_rules! optional_bound {
    ($id:ident, $underlying:ident, $suffix:ident, name=$name:expr, expect=$expected:expr, default=$default_value:expr, min=$min_value:expr) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $id {
            Limited($underlying),
            Unbounded,
        }
        impl Default for $id {
            fn default() -> Self {
                Self::Limited($default_value)
            }
        }
        impl From<$underlying> for $id {
            fn from(value: $underlying) -> Self {
                Self::Limited(value)
            }
        }
        impl From<Option<$underlying>> for $id {
            fn from(value: Option<$underlying>) -> Self {
                match value {
                    Some(value) => Self::Limited(value),
                    None => Self::Unbounded,
                }
            }
        }
        impl Into<Option<$underlying>> for $id {
            fn into(self) -> Option<$underlying> {
                match self {
                    $id::Limited(v) => Some(v),
                    $id::Unbounded => None,
                }
            }
        }
        impl FromStr for $id {
            type Err = ();
            #[allow(unused_comparisons)]
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                if value == "UNBOUNDED" {
                    Ok(Self::Unbounded)
                } else {
                    let limit = parse_number::<$suffix, $underlying>(value)?;
                    if limit < $min_value {
                        return Err(());
                    }
                    Ok(Self::Limited(limit))
                }
            }
        }
        impl<'de> Deserialize<'de> for $id {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct SelfVisitor;
                #[allow(unused_comparisons)]
                impl<'de> Visitor<'de> for SelfVisitor {
                    type Value = $id;
                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str($expected)
                    }
                    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        if value < $min_value {
                            return Err(E::custom(format!("{} must be at least {}", $name, $min_value)));
                        }
                        Ok($id::Limited(value as $underlying))
                    }
                    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        if value < $min_value {
                            return Err(E::custom(format!("{} must be at least {}", $name, $min_value)));
                        }
                        Ok($id::Limited(value as $underlying))
                    }
                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        value.parse().map_err(|_| {
                            E::custom(format!("expected {}", $expected))
                        })
                    }
                }
                deserializer.deserialize_any(SelfVisitor)
            }
        }

    };
}

number_type!(MemorySizeBytes, usize, MemorySuffix, name="Memory Size", expect="an integer or string like '128MB'", default=10 * 1024 * 1024, min=1);
number_type!(ResourceLimit, usize, NoSuffix, name="Resource Limit", expect="a positive integer", default=10_000, min=1);
number_type!(CpuFuel, u64, NoSuffix, name="CPU Fuel", expect="a positive integer", default=440_000_000, min=1);

optional_bound!(MemoryLimitBytes, usize, MemorySuffix, name="Memory Limit Bytes", expect="a positive integer or the string 'UNBOUNDED' or a string like '128MB'", default=128 * 1024 * 1024, min=1);
optional_bound!(TableLimit, usize, NoSuffix, name="Table Limit", expect="a positive integer or the string 'UNBOUNDED'", default=100_000, min=1);
optional_bound!(RequestLimit, usize, NoSuffix, name="Request Limit", expect="a positive integer or the string 'UNBOUNDED'", default=1_000, min=0);

#[test]
fn test_memory_size_bytes_deserialize() {
    let limited: MemoryLimitBytes =
        serde_json::from_str("\"1,000KB\"").expect("failed to deserialize limited memory size");
    assert_eq!(limited, MemoryLimitBytes::Limited(1024000));
    let limited: MemoryLimitBytes =
        serde_json::from_str("1048576").expect("failed to deserialize limited memory size");
    assert_eq!(limited, MemoryLimitBytes::Limited(1048576));
    let unbounded: MemoryLimitBytes = serde_json::from_str("\"UNBOUNDED\"")
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

    assert_eq!("UNBOUNDED".parse::<MemoryLimitBytes>().unwrap(), MemoryLimitBytes::Unbounded);
    assert_eq!("1".parse::<MemoryLimitBytes>().unwrap(), MemoryLimitBytes::Limited(1));
    assert_eq!("1KB".parse::<MemoryLimitBytes>().unwrap(), MemoryLimitBytes::Limited(1024));
    assert_eq!("1,000".parse::<MemoryLimitBytes>().unwrap(), MemoryLimitBytes::Limited(1000));
}
