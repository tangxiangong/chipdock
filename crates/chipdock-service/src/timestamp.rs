//! A native Swift Date bridge which avoids UniFFI's signed-magnitude timestamp
//! encoding (which cannot represent the half-second before the Unix epoch).

use chipdock_core::error::{AppError, Result};
use chrono::{DateTime, Utc};

const REFERENCE_EPOCH_SECONDS: i64 = 978_307_200;

/// Seconds since Foundation's 2001 reference date, represented as Swift `Date`.
/// Keeping this epoch also preserves Date's native subsecond precision.
#[derive(Debug, Clone, Copy)]
pub struct AppTimestamp(f64);

impl From<f64> for AppTimestamp {
    fn from(seconds: f64) -> Self {
        Self(seconds)
    }
}

impl From<AppTimestamp> for f64 {
    fn from(value: AppTimestamp) -> Self {
        value.0
    }
}

impl From<DateTime<Utc>> for AppTimestamp {
    fn from(time: DateTime<Utc>) -> Self {
        Self(
            (time.timestamp() - REFERENCE_EPOCH_SECONDS) as f64
                + f64::from(time.timestamp_subsec_nanos()) / 1.0e9,
        )
    }
}

impl TryFrom<AppTimestamp> for DateTime<Utc> {
    type Error = AppError;

    fn try_from(time: AppTimestamp) -> Result<Self> {
        let invalid = || AppError::InvalidUserInput {
            detail: "时间戳超出支持范围".into(),
        };
        let whole_seconds = time.0.floor();
        if !time.0.is_finite()
            || whole_seconds < i64::MIN as f64
            || whole_seconds >= i64::MAX as f64
        {
            return Err(invalid());
        }
        let mut seconds = (whole_seconds as i64)
            .checked_add(REFERENCE_EPOCH_SECONDS)
            .ok_or_else(invalid)?;
        let mut nanos = ((time.0 - whole_seconds) * 1.0e9).round() as u32;
        if nanos == 1_000_000_000 {
            seconds = seconds.checked_add(1).ok_or_else(invalid)?;
            nanos = 0;
        }
        DateTime::from_timestamp(seconds, nanos).ok_or_else(invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_half_second_before_unix_epoch_should_keep_its_sign() {
        let time = AppTimestamp::from(-978_307_200.5);
        assert_eq!(
            DateTime::try_from(time).unwrap().to_rfc3339(),
            "1969-12-31T23:59:59.500+00:00"
        );
    }

    #[test]
    fn nonfinite_and_out_of_range_dates_should_return_structured_errors() {
        for seconds in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1.0e14, -1.0e14] {
            assert!(matches!(
                DateTime::<Utc>::try_from(AppTimestamp::from(seconds)),
                Err(AppError::InvalidUserInput { .. })
            ));
        }
    }

    #[test]
    fn reference_date_should_round_trip_exactly() {
        let date = DateTime::try_from(AppTimestamp::from(0.25)).unwrap();
        assert_eq!(f64::from(AppTimestamp::from(date)), 0.25);
    }
}
