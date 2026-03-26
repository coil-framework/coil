use crate::identifiers::CurrencyCode;
use crate::model::{CheckoutStatus, OrderStatus, ProductStatus};
use davenda_data::DataModelError;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommerceModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    InvalidRoute {
        field: &'static str,
        value: String,
    },
    DataPlan {
        error: DataModelError,
    },
    NegativeAmount {
        field: &'static str,
        amount_minor: i64,
    },
    ZeroQuantity {
        field: &'static str,
    },
    AmountOverflow {
        field: &'static str,
    },
    BasisPointsOutOfRange {
        field: &'static str,
        basis_points: u32,
    },
    CurrencyMismatch {
        expected: CurrencyCode,
        actual: CurrencyCode,
    },
    DuplicateVariant {
        sku: String,
    },
    MissingVariant {
        sku: String,
    },
    DuplicateProduct {
        product_id: String,
    },
    MissingProduct {
        product_id: String,
    },
    DuplicateCollection {
        collection_id: String,
    },
    MissingCollection {
        collection_id: String,
    },
    ProductNotSellable {
        product_id: String,
        status: ProductStatus,
    },
    MissingLine {
        sku: String,
    },
    EmptyCheckout,
    CheckoutNotReady {
        status: CheckoutStatus,
    },
    InvalidStatusTransition {
        from: CheckoutStatus,
        to: CheckoutStatus,
    },
    TotalWouldBecomeNegative {
        total_minor: i64,
    },
    OrderNotRefundable {
        order_id: String,
        status: OrderStatus,
    },
    RefundExceedsCaptured {
        order_id: String,
        captured_minor: i64,
        refunded_minor: i64,
        requested_minor: i64,
    },
    MissingModuleSetting {
        module: String,
        field: String,
    },
    InvalidModuleSetting {
        module: String,
        field: String,
        reason: String,
    },
    UnsupportedModuleSetting {
        module: String,
        field: String,
        value: String,
    },
}

impl fmt::Display for CommerceModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::InvalidRoute { field, value } => {
                write!(f, "`{field}` must start with `/`, got `{value}`")
            }
            Self::DataPlan { error } => write!(f, "{error}"),
            Self::NegativeAmount {
                field,
                amount_minor,
            } => write!(f, "`{field}` cannot be negative, got `{amount_minor}`"),
            Self::ZeroQuantity { field } => write!(f, "`{field}` must be greater than zero"),
            Self::AmountOverflow { field } => {
                write!(f, "arithmetic overflow while calculating `{field}`")
            }
            Self::BasisPointsOutOfRange {
                field,
                basis_points,
            } => {
                write!(
                    f,
                    "`{field}` must be between 0 and 10000, got `{basis_points}`"
                )
            }
            Self::CurrencyMismatch { expected, actual } => {
                write!(
                    f,
                    "currency mismatch: expected `{expected}`, got `{actual}`"
                )
            }
            Self::DuplicateVariant { sku } => write!(f, "variant `{sku}` is duplicated"),
            Self::MissingVariant { sku } => write!(f, "variant `{sku}` was not found"),
            Self::DuplicateProduct { product_id } => {
                write!(f, "catalog product `{product_id}` is duplicated")
            }
            Self::MissingProduct { product_id } => {
                write!(f, "catalog product `{product_id}` was not found")
            }
            Self::DuplicateCollection { collection_id } => {
                write!(f, "catalog collection `{collection_id}` is duplicated")
            }
            Self::MissingCollection { collection_id } => {
                write!(f, "catalog collection `{collection_id}` was not found")
            }
            Self::ProductNotSellable { product_id, status } => {
                write!(f, "product `{product_id}` is not sellable while `{status}`")
            }
            Self::MissingLine { sku } => write!(f, "checkout line for `{sku}` was not found"),
            Self::EmptyCheckout => f.write_str("checkout must contain at least one line"),
            Self::CheckoutNotReady { status } => {
                write!(f, "checkout cannot advance from status `{status}`")
            }
            Self::InvalidStatusTransition { from, to } => {
                write!(f, "cannot transition checkout from `{from}` to `{to}`")
            }
            Self::TotalWouldBecomeNegative { total_minor } => {
                write!(f, "priced total would become negative: `{total_minor}`")
            }
            Self::OrderNotRefundable { order_id, status } => {
                write!(f, "order `{order_id}` cannot be refunded while `{status}`")
            }
            Self::RefundExceedsCaptured {
                order_id,
                captured_minor,
                refunded_minor,
                requested_minor,
            } => write!(
                f,
                "refund for order `{order_id}` exceeds captured amount: captured={captured_minor} refunded={refunded_minor} requested={requested_minor}"
            ),
            Self::MissingModuleSetting { module, field } => {
                write!(f, "module `{module}` requires setting `{field}`")
            }
            Self::InvalidModuleSetting {
                module,
                field,
                reason,
            } => {
                write!(
                    f,
                    "module `{module}` has invalid setting `{field}`: {reason}"
                )
            }
            Self::UnsupportedModuleSetting {
                module,
                field,
                value,
            } => {
                write!(
                    f,
                    "module `{module}` does not support `{field} = {value}` in the current checkout contract"
                )
            }
        }
    }
}

impl Error for CommerceModelError {}

impl From<DataModelError> for CommerceModelError {
    fn from(error: DataModelError) -> Self {
        Self::DataPlan { error }
    }
}
