use crate::CoreError;
use serde::{Deserialize, Serialize};
/// Unit for a ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RatioUnit {
    Decimal,
    Percent,
}
macro_rules! finite_type {
    ($name:ident,$field:literal,$pred:expr,$reason:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
        pub struct $name(f64);
        impl $name {
            pub fn new(value: f64) -> Result<Self, CoreError> {
                if !value.is_finite() || !(($pred)(value)) {
                    Err(CoreError::InvalidValue {
                        field: $field,
                        value: value.to_string(),
                        reason: $reason,
                    })
                } else {
                    Ok(Self(value))
                }
            }
            pub fn get(self) -> f64 {
                self.0
            }
        }
    };
}
finite_type!(Price, "price", |v: f64| v > 0.0, "must be positive");
finite_type!(
    Quantity,
    "quantity",
    |v: f64| v >= 0.0,
    "must be non-negative"
);
finite_type!(Money, "money", |_v: f64| true, "must be finite");
/// A finite decimal or percentage ratio.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ratio {
    value: f64,
    unit: RatioUnit,
}
impl Ratio {
    pub fn decimal(v: f64) -> Result<Self, CoreError> {
        Self::new(v, RatioUnit::Decimal)
    }
    pub fn new(v: f64, unit: RatioUnit) -> Result<Self, CoreError> {
        if v.is_finite() {
            Ok(Self { value: v, unit })
        } else {
            Err(CoreError::InvalidValue {
                field: "ratio",
                value: v.to_string(),
                reason: "must be finite",
            })
        }
    }
    pub fn get(self) -> f64 {
        self.value
    }
    pub fn unit(self) -> RatioUnit {
        self.unit
    }
}
