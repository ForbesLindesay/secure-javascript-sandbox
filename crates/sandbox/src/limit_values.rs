use serde::{Deserialize, de::Visitor};
use std::{marker::PhantomData, str::FromStr};

#[derive(Clone, Copy)]
enum ScalePrefix {
    K,
    M,
    G,
}
impl ScalePrefix {
    fn into_pow(self) -> u32 {
        match self {
            ScalePrefix::K => 1,
            ScalePrefix::M => 2,
            ScalePrefix::G => 3,
        }
    }
    fn into_si_multiplier(self) -> u32 {
        u32::pow(1000, self.into_pow())
    }
    fn into_byte_multiplier(self) -> u32 {
        u32::pow(1024, self.into_pow())
    }
}
impl TryFrom<char> for ScalePrefix {
    type Error = ();
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            'K' | 'k' => Ok(ScalePrefix::K),
            'M' | 'm' => Ok(ScalePrefix::M),
            'G' | 'g' => Ok(ScalePrefix::G),
            _ => Err(()),
        }
    }
}

trait SuffixParser {
    fn parse(a: char, b: Option<char>) -> Result<u32, ()>;
}

struct SuffixChars(Option<char>, Option<char>);
impl SuffixChars {
    fn new(a: char) -> Self {
        SuffixChars(
            match a {
                ' ' => None,
                c => Some(c),
            },
            None,
        )
    }
}
impl SuffixChars {
    fn push(&mut self, c: char) -> Result<(), ()> {
        if self.0.is_none() {
            self.0 = Some(c);
            Ok(())
        } else if self.1.is_none() {
            self.1 = Some(c);
            Ok(())
        } else {
            Err(())
        }
    }
    fn parse<T: SuffixParser>(self) -> Result<u32, ()> {
        match self.0 {
            Some(a) => T::parse(a, self.1),
            None => Err(()),
        }
    }
}

struct MemorySuffix;
impl SuffixParser for MemorySuffix {
    fn parse(a: char, b: Option<char>) -> Result<u32, ()> {
        match (a, b) {
            ('B' | 'b', None) => Ok(1),
            (scale_prefix, Some('B' | 'b')) => {
                Ok(ScalePrefix::try_from(scale_prefix)?.into_byte_multiplier())
            }
            _ => Err(()),
        }
    }
}

struct NoUnitSuffix;
impl SuffixParser for NoUnitSuffix {
    fn parse(a: char, b: Option<char>) -> Result<u32, ()> {
        if b.is_some() {
            return Err(());
        }
        Ok(ScalePrefix::try_from(a)?.into_si_multiplier())
    }
}
trait PositiveNumber:
    Copy
    + Sized
    + std::fmt::Debug
    + Eq
    + TryFrom<u32>
    + TryFrom<u64>
    + TryFrom<i32>
    + TryFrom<i64>
    + std::fmt::Display
{
    fn mul(a: Self, b: Self) -> Option<Self>;
    fn add(a: Self, b: Self) -> Option<Self>;
}

macro_rules! impl_number {
    ($id:ident) => {
        impl PositiveNumber for $id {
            #[inline]
            fn mul(a: Self, b: Self) -> Option<Self> {
                a.checked_mul(b)
            }
            #[inline]
            fn add(a: Self, b: Self) -> Option<Self> {
                a.checked_add(b)
            }
        }
    };
}
impl_number!(u64);
impl_number!(usize);

fn parse_number<TSuffix: SuffixParser, TResult: PositiveNumber>(value: &str) -> Option<TResult> {
    let ten = TResult::try_from(10)
        .ok()
        .expect("Must be able to convert 10 to number type");

    // Value must always start with a digit
    let mut chars = value.chars();
    let first_digit = chars.next()?.to_digit(10)?;
    let mut value = TResult::try_from(first_digit).ok()?;
    let mut got_separator = false;
    let mut suffix: Option<SuffixChars> = None;

    for char in chars {
        if let Some(s) = &mut suffix {
            s.push(char).ok()?;
        } else if let Some(digit) = char.to_digit(10) {
            if first_digit == 0 {
                // Do not allow leading zeroes
                return None;
            }
            got_separator = false;
            value = TResult::add(TResult::mul(value, ten)?, TResult::try_from(digit).ok()?)?;
        } else if char == '_' || char == ',' {
            if got_separator {
                // We don't allow two separators in a row
                return None;
            }
            got_separator = true;
        } else {
            suffix = Some(SuffixChars::new(char));
        }
    }
    if got_separator {
        // We don't allow ending on a separator
        return None;
    }
    if let Some(s) = suffix {
        TResult::mul(value, TResult::try_from(s.parse::<TSuffix>().ok()?).ok()?)
    } else {
        Some(value)
    }
}

struct NumberVisitor<TNumber: PositiveNumber, TResult: TryFrom<TNumber>> {
    name: &'static str,
    expected: &'static str,
    number: PhantomData<TNumber>,
    result: PhantomData<TResult>,
}
impl<TNumber: PositiveNumber, TResult: TryFrom<TNumber> + FromStr> NumberVisitor<TNumber, TResult> {
    #[inline]
    fn visit<TUnderlying, E>(&self, value: TUnderlying) -> Result<TResult, E>
    where
        TNumber: TryFrom<TUnderlying>,
        E: serde::de::Error,
    {
        TNumber::try_from(value)
            .ok()
            .and_then(|v| TResult::try_from(v).ok())
            .ok_or_else(|| {
                E::custom(format!(
                    "Invalid value for {}, expected {}",
                    self.name, self.expected
                ))
            })
    }
}
impl<TNumber: PositiveNumber, TResult: TryFrom<TNumber> + FromStr> Visitor<'_>
    for NumberVisitor<TNumber, TResult>
{
    type Value = TResult;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("")
    }
    fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit(value)
    }
    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit(value)
    }
    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit(value)
    }
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        value.parse().map_err(|_| {
            E::custom(format!(
                "Invalid value for {}, expected {}",
                self.name, self.expected
            ))
        })
    }
}

#[cfg(test)]
fn assert_parse_ok<TSuffix: SuffixParser, T: PositiveNumber>(
    string_to_parse: &str,
    parsed_number: u64,
) {
    assert_eq!(
        parse_number::<TSuffix, T>(string_to_parse)
            .expect(&format!("{} was not passed successfully", string_to_parse)),
        T::try_from(parsed_number).ok().expect(&format!(
            "{} can't be parsed to the right type",
            parsed_number
        ))
    );
}

#[cfg(test)]
fn test_parse_number_without_suffix<TSuffix: SuffixParser, T: PositiveNumber>() {
    assert_parse_ok::<TSuffix, T>("0", 0);
    assert_parse_ok::<TSuffix, T>("0", 0);
    assert_parse_ok::<TSuffix, T>("1", 1);
    assert_parse_ok::<TSuffix, T>("12,345", 12_345);
    assert_parse_ok::<TSuffix, T>("1_000_000", 1_000_000);
    assert!(
        parse_number::<TSuffix, T>("").is_none(),
        "expected error parsing empty string"
    );
    assert!(
        parse_number::<TSuffix, T>(",100").is_none(),
        "expected error parsing string starting with separator"
    );
    assert!(
        parse_number::<TSuffix, T>("100,").is_none(),
        "expected error parsing string ending with separator"
    );
    assert!(
        parse_number::<TSuffix, T>("01").is_none(),
        "expected error parsing string with leading 0"
    );
    assert!(
        parse_number::<TSuffix, T>("1 ").is_none(),
        "expected error parsing string with trailing space"
    );
}
#[cfg(test)]
fn test_parse_number_t<T: PositiveNumber>() {
    test_parse_number_without_suffix::<NoUnitSuffix, T>();
    test_parse_number_without_suffix::<MemorySuffix, T>();

    assert_parse_ok::<MemorySuffix, T>("0B", 0);
    assert_parse_ok::<NoUnitSuffix, T>("0M", 0);
    assert_parse_ok::<MemorySuffix, T>("0MB", 0);
    assert_parse_ok::<MemorySuffix, T>("1B", 1);
    assert_parse_ok::<NoUnitSuffix, T>("1K", 1000);
    assert_parse_ok::<MemorySuffix, T>("1KB", 1024);
    assert_parse_ok::<NoUnitSuffix, T>("2M", 2_000_000);
    assert_parse_ok::<MemorySuffix, T>("2MB", 2_097_152);
    assert_parse_ok::<MemorySuffix, T>("1 KB", 1024);
    assert_parse_ok::<NoUnitSuffix, T>("1 K", 1000);
    assert_parse_ok::<MemorySuffix, T>("1,000 KB", 1024_000);
    assert_parse_ok::<NoUnitSuffix, T>("1,000 K", 1_000_000);
    assert!(
        parse_number::<MemorySuffix, T>("1 K").is_none(),
        "expected error parsing string with incomplete suffix"
    );
    assert!(
        parse_number::<MemorySuffix, T>("1 KB ").is_none(),
        "expected error parsing string with space after suffix"
    );
    assert!(
        parse_number::<MemorySuffix, T>("1 KB,").is_none(),
        "expected error parsing string with separator after suffix"
    );
    assert!(
        parse_number::<NoUnitSuffix, T>("1KK").is_none(),
        "expected error parsing string with duplicate suffix"
    );
}

#[test]
fn test_parse_number() {
    test_parse_number_t::<usize>();
    test_parse_number_t::<u64>();
}

macro_rules! number_type {
    ($id:ident, $underlying:ty, $suffix:ident, name=$name:expr, expect=$expected:expr, default=$default_value:expr, min=$min_value:expr) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $id(pub $underlying);
        impl Default for $id {
            fn default() -> Self {
                Self($default_value)
            }
        }
        impl TryFrom<$underlying> for $id {
            type Error = ();
            fn try_from(value: $underlying) -> Result<Self, Self::Error> {
                if value < $min_value {
                    Err(())
                } else {
                    Ok(Self(value))
                }
            }
        }
        impl From<$id> for $underlying {
            fn from(v: $id) -> $underlying {
                v.0
            }
        }
        impl FromStr for $id {
            type Err = ();
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                let limit = parse_number::<$suffix, $underlying>(value).ok_or(())?;
                Self::try_from(limit)
            }
        }
        impl<'de> Deserialize<'de> for $id {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                deserializer.deserialize_any(NumberVisitor::<$underlying, $id> {
                    name: $name,
                    expected: $expected,
                    result: Default::default(),
                    number: Default::default(),
                })
            }
        }
    };
}

macro_rules! optional_bound {
    ($id:ident, $underlying:ty, $suffix:ident, name=$name:expr, expect=$expected:expr, default=$default_value:expr, min=$min_value:expr) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $id {
            Limited($underlying),
            Unbounded,
        }
        impl $id {
            pub fn is_within_bound(&self, requested: $underlying) -> bool {
                match self {
                    Self::Limited(max) => *max >= requested,
                    Self::Unbounded => true,
                }
            }
        }
        impl Default for $id {
            fn default() -> Self {
                Self::Limited($default_value)
            }
        }
        impl TryFrom<$underlying> for $id {
            type Error = ();
            #[allow(unused_comparisons)]
            fn try_from(value: $underlying) -> Result<Self, Self::Error> {
                if value < $min_value {
                    Err(())
                } else {
                    Ok(Self::Limited(value))
                }
            }
        }
        impl TryFrom<Option<$underlying>> for $id {
            type Error = ();
            fn try_from(value: Option<$underlying>) -> Result<Self, Self::Error> {
                match value {
                    Some(value) => Self::try_from(value),
                    None => Ok(Self::Unbounded),
                }
            }
        }
        impl From<$id> for Option<$underlying> {
            fn from(v: $id) -> Option<$underlying> {
                match v {
                    $id::Limited(v) => Some(v),
                    $id::Unbounded => None,
                }
            }
        }
        impl FromStr for $id {
            type Err = ();
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                if value == "UNBOUNDED" {
                    Ok(Self::Unbounded)
                } else {
                    let limit = parse_number::<$suffix, $underlying>(value).ok_or(())?;
                    Self::try_from(limit)
                }
            }
        }
        impl<'de> Deserialize<'de> for $id {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                deserializer.deserialize_any(NumberVisitor::<$underlying, $id> {
                    name: $name,
                    expected: $expected,
                    result: Default::default(),
                    number: Default::default(),
                })
            }
        }
    };
}

number_type!(
    MemorySizeBytes,
    usize,
    MemorySuffix,
    name = "Memory Size",
    expect = "an integer or string like '128MB'",
    default = 10 * 1024 * 1024,
    min = 1
);
number_type!(
    ResourceLimit,
    usize,
    NoUnitSuffix,
    name = "Resource Limit",
    expect = "a positive integer",
    default = 10_000,
    min = 1
);
number_type!(
    CpuFuel,
    u64,
    NoUnitSuffix,
    name = "CPU Fuel",
    expect = "a positive integer",
    default = 440_000_000,
    min = 1
);

optional_bound!(
    ApiRequestBodyLimit,
    usize,
    MemorySuffix,
    name = "API Request Body Limit Bytes",
    expect = "a positive integer or the string 'UNBOUNDED' or a string like '8MB'",
    default = 2 * 1024 * 1024,
    min = 1
);

optional_bound!(
    MemoryLimitBytes,
    usize,
    MemorySuffix,
    name = "Memory Limit Bytes",
    expect = "a positive integer or the string 'UNBOUNDED' or a string like '128MB'",
    default = 128 * 1024 * 1024,
    min = 1
);
optional_bound!(
    TableLimit,
    usize,
    NoUnitSuffix,
    name = "Table Limit",
    expect = "a positive integer or the string 'UNBOUNDED'",
    default = 100_000,
    min = 1
);
optional_bound!(
    RequestLimit,
    usize,
    NoUnitSuffix,
    name = "Request Limit",
    expect = "a positive integer or the string 'UNBOUNDED'",
    default = 1_000,
    min = 0
);

#[test]
fn test_memory_size_bytes_deserialize() {
    let limited: MemoryLimitBytes =
        serde_json::from_str("\"1,000KB\"").expect("failed to deserialize limited memory size");
    assert_eq!(limited, MemoryLimitBytes::Limited(1024000));
    let limited: MemoryLimitBytes =
        serde_json::from_str("1048576").expect("failed to deserialize limited memory size");
    assert_eq!(limited, MemoryLimitBytes::Limited(1048576));
    let unbounded: MemoryLimitBytes =
        serde_json::from_str("\"UNBOUNDED\"").expect("failed to deserialize unbounded memory size");
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

    assert_eq!(
        "UNBOUNDED".parse::<MemoryLimitBytes>().unwrap(),
        MemoryLimitBytes::Unbounded
    );
    assert_eq!(
        "1".parse::<MemoryLimitBytes>().unwrap(),
        MemoryLimitBytes::Limited(1)
    );
    assert_eq!(
        "1KB".parse::<MemoryLimitBytes>().unwrap(),
        MemoryLimitBytes::Limited(1024)
    );
    assert_eq!(
        "1,000".parse::<MemoryLimitBytes>().unwrap(),
        MemoryLimitBytes::Limited(1000)
    );
}
