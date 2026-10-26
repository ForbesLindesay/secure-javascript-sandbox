use serde::{Deserialize, de::Visitor};
use std::{marker::PhantomData, str::FromStr, time::Duration};

#[derive(Clone, Copy)]
enum ScalePrefix {
    K,
    M,
    G,
    T,
}
impl ScalePrefix {
    fn parse_char(value: char) -> Option<ScalePrefix> {
        match value {
            'K' | 'k' => Some(ScalePrefix::K),
            'M' | 'm' => Some(ScalePrefix::M),
            'G' | 'g' => Some(ScalePrefix::G),
            'T' | 't' => Some(ScalePrefix::T),
            _ => None,
        }
    }
    fn into_pow(self) -> u32 {
        match self {
            ScalePrefix::K => 1,
            ScalePrefix::M => 2,
            ScalePrefix::G => 3,
            ScalePrefix::T => 4,
        }
    }
    fn into_si_multiplier(self) -> u64 {
        u64::pow(1000, self.into_pow())
    }
    fn into_byte_multiplier(self) -> u64 {
        u64::pow(1024, self.into_pow())
    }
}

trait SuffixParser {
    fn parse(suffix: &str) -> Option<u64>;
}

struct MemorySuffix;
impl SuffixParser for MemorySuffix {
    fn parse(suffix: &str) -> Option<u64> {
        let mut chars = suffix.chars();
        match (chars.next()?, chars.as_str()) {
            ('B' | 'b', "") => Some(1),
            (scale_prefix, "B" | "b") => {
                Some(ScalePrefix::parse_char(scale_prefix)?.into_byte_multiplier())
            }
            _ => None,
        }
    }
}

struct NoUnitSuffix;
impl SuffixParser for NoUnitSuffix {
    fn parse(suffix: &str) -> Option<u64> {
        let mut chars = suffix.chars();
        let scale_prefix = ScalePrefix::parse_char(chars.next()?)?;
        if !chars.as_str().is_empty() {
            return None;
        }
        Some(scale_prefix.into_si_multiplier())
    }
}

/// Parses durations into a number of milliseconds
struct DurationSuffix;
impl SuffixParser for DurationSuffix {
    fn parse(suffix: &str) -> Option<u64> {
        match suffix.to_ascii_lowercase().as_str() {
            "ms" | "millisecond" | "milliseconds" => Some(1),
            "s" | "sec" | "secs" | "second" | "seconds" => Some(1000),
            "m" | "min" | "mins" | "minute" | "minutes" => Some(60 * 1000),
            "h" | "hr" | "hrs" | "hour" | "hours" => Some(60 * 60 * 1000),
            _ => None,
        }
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
}
impl PositiveNumber for u32 {}
impl PositiveNumber for u64 {}
impl PositiveNumber for usize {}

fn parse_number_parts(input: &str) -> Option<(u64, u64, Option<&str>)> {
    // Value must always start with a digit
    let mut chars = input.char_indices();
    let first_digit = chars.next()?.1.to_digit(10)?;
    let mut value: u64 = first_digit.into();
    let mut got_separator = false;

    let mut got_decimal = false;
    let mut decimal_places: u32 = 0;

    let mut suffix: Option<&str> = None;

    for (index, char) in chars {
        if let Some(digit) = char.to_digit(10) {
            if got_decimal {
                decimal_places = decimal_places.checked_add(1)?;
            } else if first_digit == 0 {
                // Do not allow leading zeroes
                return None;
            }
            got_separator = false;
            value = value.checked_mul(10)?.checked_add(digit.into())?;
        } else if char == '_' || char == ',' || char == '.' {
            if char == '.' {
                if got_decimal {
                    // You can't have more than one decimal separator
                    return None;
                }
                got_decimal = true;
            }
            if got_separator {
                // We don't allow two separators in a row
                return None;
            }
            got_separator = true;
        } else {
            // The suffix is everything after the number, optionally separated by a single space
            let rest = &input[index..];
            suffix = Some(rest.strip_prefix(' ').unwrap_or(rest));
            break;
        }
    }
    if got_separator {
        // We don't allow ending on a separator
        return None;
    }
    if got_decimal && decimal_places == 0 {
        return None;
    }
    Some((value, u64::pow(10, decimal_places), suffix))
}

fn parse_u64<TSuffix: SuffixParser>(value: &str) -> Option<u64> {
    let (result, decimal_divisor, suffix) = parse_number_parts(value)?;
    let suffix_multiplier = match suffix {
        Some(suffix) => TSuffix::parse(suffix)?,
        None => 1,
    };
    if decimal_divisor > suffix_multiplier {
        return None;
    }
    result
        .checked_mul(suffix_multiplier)?
        .checked_div(decimal_divisor)
}

fn parse_number<
    TSuffix: SuffixParser,
    TUnderlying: TryFrom<u64>,
    TWrapped: TryFrom<TUnderlying>,
>(
    value: &str,
) -> Result<TWrapped, ()> {
    let numeric_value = parse_u64::<TSuffix>(value).ok_or(())?;
    let underlying = TUnderlying::try_from(numeric_value).map_err(|_| ())?;
    TWrapped::try_from(underlying).map_err(|_| ())
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
        formatter.write_str(self.expected)
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
fn assert_parse_ok<TSuffix: SuffixParser>(string_to_parse: &str, parsed_number: u64) {
    assert_eq!(
        parse_u64::<TSuffix>(string_to_parse),
        Some(parsed_number),
        "{string_to_parse} was not parsed successfully"
    );
}

#[cfg(test)]
fn test_parse_number_without_suffix<TSuffix: SuffixParser>() {
    assert_parse_ok::<TSuffix>("0", 0);
    assert_parse_ok::<TSuffix>("0", 0);
    assert_parse_ok::<TSuffix>("1", 1);
    assert_parse_ok::<TSuffix>("12,345", 12_345);
    assert_parse_ok::<TSuffix>("1_000_000", 1_000_000);
    assert!(
        parse_u64::<TSuffix>("").is_none(),
        "expected error parsing empty string"
    );
    assert!(
        parse_u64::<TSuffix>(",100").is_none(),
        "expected error parsing string starting with separator"
    );
    assert!(
        parse_u64::<TSuffix>("100,").is_none(),
        "expected error parsing string ending with separator"
    );
    assert!(
        parse_u64::<TSuffix>("01").is_none(),
        "expected error parsing string with leading 0"
    );
    assert!(
        parse_u64::<TSuffix>("1 ").is_none(),
        "expected error parsing string with trailing space"
    );
}

#[test]
fn test_parse_number() {
    test_parse_number_without_suffix::<NoUnitSuffix>();
    test_parse_number_without_suffix::<MemorySuffix>();

    assert_parse_ok::<MemorySuffix>("0B", 0);
    assert_parse_ok::<NoUnitSuffix>("0M", 0);
    assert_parse_ok::<MemorySuffix>("0MB", 0);
    assert_parse_ok::<MemorySuffix>("1B", 1);
    assert_parse_ok::<NoUnitSuffix>("1K", 1000);
    assert_parse_ok::<MemorySuffix>("1KB", 1024);
    assert_parse_ok::<NoUnitSuffix>("2M", 2_000_000);
    assert_parse_ok::<MemorySuffix>("2MB", 2_097_152);
    assert_parse_ok::<MemorySuffix>("1 KB", 1024);
    assert_parse_ok::<NoUnitSuffix>("1 K", 1000);
    assert_parse_ok::<MemorySuffix>("1,000 KB", 1_024_000);
    assert_parse_ok::<NoUnitSuffix>("1,000 K", 1_000_000);
    assert_parse_ok::<MemorySuffix>("2.5 GB", 2_684_354_560);
    assert_parse_ok::<NoUnitSuffix>("2.5 G", 2_500_000_000);
    assert_parse_ok::<MemorySuffix>("2.5 TB", 2_748_779_069_440);
    assert_parse_ok::<NoUnitSuffix>("2.5 T", 2_500_000_000_000);
    assert!(
        parse_u64::<MemorySuffix>("1 K").is_none(),
        "expected error parsing string with incomplete suffix"
    );
    assert!(
        parse_u64::<MemorySuffix>("1 KB ").is_none(),
        "expected error parsing string with space after suffix"
    );
    assert!(
        parse_u64::<MemorySuffix>("1 KB,").is_none(),
        "expected error parsing string with separator after suffix"
    );
    assert!(
        parse_u64::<NoUnitSuffix>("1KK").is_none(),
        "expected error parsing string with duplicate suffix"
    );
    assert!(
        parse_u64::<NoUnitSuffix>("1.5006K").is_none(),
        "expected error parsing with more decimal places than the unit allows"
    );
}

#[test]
fn test_parse_duration() {
    test_parse_number_without_suffix::<DurationSuffix>();

    assert_parse_ok::<DurationSuffix>("500ms", 500);
    assert_parse_ok::<DurationSuffix>("500 milliseconds", 500);
    assert_parse_ok::<DurationSuffix>("10s", 10_000);
    assert_parse_ok::<DurationSuffix>("10 seconds", 10_000);
    assert_parse_ok::<DurationSuffix>("1 second", 1000);
    assert_parse_ok::<DurationSuffix>("0.5s", 500);
    assert_parse_ok::<DurationSuffix>("5m", 300_000);
    assert_parse_ok::<DurationSuffix>("5 minutes", 300_000);
    assert_parse_ok::<DurationSuffix>("5 Minutes", 300_000);
    assert_parse_ok::<DurationSuffix>("1.5 mins", 90_000);
    assert_parse_ok::<DurationSuffix>("2h", 7_200_000);
    assert_parse_ok::<DurationSuffix>("1 hour", 3_600_000);
    assert!(
        parse_u64::<DurationSuffix>("10 fortnights").is_none(),
        "expected error parsing unknown unit"
    );
    assert!(
        parse_u64::<DurationSuffix>("10  seconds").is_none(),
        "expected error parsing string with two spaces before suffix"
    );
    assert!(
        parse_u64::<DurationSuffix>("10 seconds ").is_none(),
        "expected error parsing string with space after suffix"
    );
    assert!(
        parse_u64::<DurationSuffix>("1.5ms").is_none(),
        "expected error parsing with more decimal places than the unit allows"
    );
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
                parse_number::<$suffix, $underlying, Self>(value)
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
                    parse_number::<$suffix, $underlying, Self>(value)
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
number_type!(
    Timeout,
    u64,
    DurationSuffix,
    name = "Timeout",
    expect = "a positive integer number of milliseconds or a string like '10s' or '5 minutes'",
    default = 10 * 60 * 1000,
    min = 1
);
impl From<Timeout> for Duration {
    fn from(value: Timeout) -> Self {
        Duration::from_millis(value.0)
    }
}

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
fn test_timeout_deserialize() {
    let timeout: Timeout =
        serde_json::from_str("\"10 seconds\"").expect("failed to deserialize timeout");
    assert_eq!(Duration::from(timeout), Duration::from_secs(10));
    let timeout: Timeout = serde_json::from_str("\"5m\"").expect("failed to deserialize timeout");
    assert_eq!(Duration::from(timeout), Duration::from_mins(5));
    let timeout: Timeout = serde_json::from_str("1500").expect("failed to deserialize timeout");
    assert_eq!(Duration::from(timeout), Duration::from_millis(1500));
    let timeout: Timeout = "10s".parse().expect("failed to parse timeout");
    assert_eq!(Duration::from(timeout), Duration::from_secs(10));
    assert!(
        serde_json::from_str::<Timeout>("0").is_err(),
        "expected error deserializing a zero timeout"
    );
}

#[test]
fn test_memory_size_bytes_deserialize() {
    let limited: MemoryLimitBytes =
        serde_json::from_str("\"1,000KB\"").expect("failed to deserialize limited memory size");
    assert_eq!(limited, MemoryLimitBytes::Limited(1_024_000));
    let limited: MemoryLimitBytes =
        serde_json::from_str("1048576").expect("failed to deserialize limited memory size");
    assert_eq!(limited, MemoryLimitBytes::Limited(1_048_576));
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
