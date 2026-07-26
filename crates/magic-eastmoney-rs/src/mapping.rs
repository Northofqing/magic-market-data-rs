use crate::EastmoneyError;
use magic_market_core::{
    FiniteNumber, IsoDate, Money, NonEmptyText, Price, Quantity, Ratio, RatioUnit,
};
use serde_json::Value;

pub(crate) fn required_string(object: &Value, key: &'static str) -> Result<String, EastmoneyError> {
    optional_string(object.get(key))?
        .ok_or_else(|| EastmoneyError::Protocol(format!("required response field {key} is absent")))
}

pub(crate) fn optional_string(value: Option<&Value>) -> Result<Option<String>, EastmoneyError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            if trimmed.is_empty() || matches!(trimmed, "-" | "--") {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_owned()))
            }
        }
        Some(Value::Number(number)) => Ok(Some(number.to_string())),
        Some(other) => Err(EastmoneyError::Protocol(format!(
            "expected string-compatible field, received {other}"
        ))),
    }
}

pub(crate) fn optional_f64(value: Option<&Value>) -> Result<Option<f64>, EastmoneyError> {
    let parsed = match value {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Number(number)) => number.as_f64().ok_or_else(|| {
            EastmoneyError::Protocol(format!("number {number} cannot be represented as f64"))
        })?,
        Some(Value::String(text)) => {
            let trimmed = text.trim().trim_end_matches('%');
            if trimmed.is_empty() || matches!(trimmed, "-" | "--") {
                return Ok(None);
            }
            trimmed.parse::<f64>().map_err(|error| {
                EastmoneyError::Protocol(format!("invalid numeric field {trimmed}: {error}"))
            })?
        }
        Some(other) => {
            return Err(EastmoneyError::Protocol(format!(
                "expected numeric field, received {other}"
            )))
        }
    };
    if !parsed.is_finite() {
        return Err(EastmoneyError::Protocol(
            "numeric response field is not finite".into(),
        ));
    }
    Ok(Some(parsed))
}

pub(crate) fn optional_u32(value: Option<&Value>) -> Result<Option<u32>, EastmoneyError> {
    optional_f64(value)?
        .map(|number| {
            if number < 0.0 || number.fract() != 0.0 || number > f64::from(u32::MAX) {
                return Err(EastmoneyError::Protocol(format!(
                    "{number} is not a valid u32"
                )));
            }
            Ok(number as u32)
        })
        .transpose()
}

pub(crate) fn non_empty(value: Option<String>) -> Result<Option<NonEmptyText>, EastmoneyError> {
    value.map(NonEmptyText::new).transpose().map_err(Into::into)
}

pub(crate) fn finite(value: Option<f64>) -> Result<Option<FiniteNumber>, EastmoneyError> {
    value.map(FiniteNumber::new).transpose().map_err(Into::into)
}

pub(crate) fn money(value: Option<f64>) -> Result<Option<Money>, EastmoneyError> {
    value.map(Money::new).transpose().map_err(Into::into)
}

pub(crate) fn quantity(value: Option<f64>) -> Result<Option<Quantity>, EastmoneyError> {
    value.map(Quantity::new).transpose().map_err(Into::into)
}

pub(crate) fn price(value: Option<f64>) -> Result<Option<Price>, EastmoneyError> {
    value.map(Price::new).transpose().map_err(Into::into)
}

pub(crate) fn percent(value: Option<f64>) -> Result<Option<Ratio>, EastmoneyError> {
    value
        .map(|number| Ratio::new(number, RatioUnit::Percent))
        .transpose()
        .map_err(Into::into)
}

pub(crate) fn decimal(value: Option<f64>) -> Result<Option<Ratio>, EastmoneyError> {
    value
        .map(|number| Ratio::new(number, RatioUnit::Decimal))
        .transpose()
        .map_err(Into::into)
}

pub(crate) fn iso_date(value: &str) -> Result<IsoDate, EastmoneyError> {
    validate_date_or_datetime(value, "timestamp")?;
    let date = value.get(..10).ok_or_else(|| {
        EastmoneyError::Protocol(format!("timestamp {value} has no YYYY-MM-DD prefix"))
    })?;
    Ok(IsoDate::new(date)?)
}

pub(crate) fn validate_date_or_datetime(
    value: &str,
    field: &'static str,
) -> Result<(), EastmoneyError> {
    let date = value.get(..10).ok_or_else(|| {
        EastmoneyError::Protocol(format!("{field} {value:?} has no YYYY-MM-DD prefix"))
    })?;
    IsoDate::new(date).map_err(|error| {
        EastmoneyError::Protocol(format!(
            "{field} {value:?} has an invalid calendar date: {error}"
        ))
    })?;
    let suffix = value.get(10..).ok_or_else(|| {
        EastmoneyError::Protocol(format!("{field} {value:?} is not valid UTF-8 text"))
    })?;
    if suffix.is_empty() {
        return Ok(());
    }
    let time = suffix.strip_prefix(' ').ok_or_else(|| {
        EastmoneyError::Protocol(format!(
            "{field} {value:?} must separate date and time with one space"
        ))
    })?;
    validate_clock_time(time, field, value)
}

pub(crate) fn validate_minute_timestamp(
    value: &str,
    field: &'static str,
) -> Result<(), EastmoneyError> {
    let date = value.get(..10).ok_or_else(|| {
        EastmoneyError::Protocol(format!("{field} {value:?} has no YYYY-MM-DD prefix"))
    })?;
    IsoDate::new(date).map_err(|error| {
        EastmoneyError::Protocol(format!(
            "{field} {value:?} has an invalid calendar date: {error}"
        ))
    })?;
    let time = value
        .get(11..)
        .filter(|_| value.as_bytes().get(10) == Some(&b' '))
        .ok_or_else(|| {
            EastmoneyError::Protocol(format!("{field} {value:?} must use YYYY-MM-DD HH:MM"))
        })?;
    if time.len() != 5 {
        return Err(EastmoneyError::Protocol(format!(
            "{field} {value:?} must use YYYY-MM-DD HH:MM"
        )));
    }
    validate_clock_time(time, field, value)
}

fn validate_clock_time(
    time: &str,
    field: &'static str,
    original: &str,
) -> Result<(), EastmoneyError> {
    let (clock, fraction) = match time.split_once('.') {
        Some((clock, fraction)) => (clock, Some(fraction)),
        None => (time, None),
    };
    if clock.len() != 5 && clock.len() != 8 {
        return Err(EastmoneyError::Protocol(format!(
            "{field} {original:?} has an invalid clock shape"
        )));
    }
    let bytes = clock.as_bytes();
    if bytes.get(2) != Some(&b':')
        || (clock.len() == 8 && bytes.get(5) != Some(&b':'))
        || !bytes.iter().enumerate().all(|(index, byte)| {
            index == 2 || (clock.len() == 8 && index == 5) || byte.is_ascii_digit()
        })
    {
        return Err(EastmoneyError::Protocol(format!(
            "{field} {original:?} has an invalid clock shape"
        )));
    }
    if fraction.is_some_and(|fraction| {
        fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        return Err(EastmoneyError::Protocol(format!(
            "{field} {original:?} has invalid fractional seconds"
        )));
    }
    if fraction.is_some() && clock.len() != 8 {
        return Err(EastmoneyError::Protocol(format!(
            "{field} {original:?} has fractional seconds without seconds"
        )));
    }
    let hour = clock[0..2].parse::<u32>().unwrap_or(u32::MAX);
    let minute = clock[3..5].parse::<u32>().unwrap_or(u32::MAX);
    let second = if clock.len() == 8 {
        clock[6..8].parse::<u32>().unwrap_or(u32::MAX)
    } else {
        0
    };
    if hour > 23 || minute > 59 || second > 59 {
        return Err(EastmoneyError::Protocol(format!(
            "{field} {original:?} is outside calendar/time bounds"
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/internal/mapping_tests.rs"]
mod tests;
