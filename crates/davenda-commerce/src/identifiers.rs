use crate::validation::validate_token;
use crate::CommerceModelError;
use std::fmt;

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, CommerceModelError> {
                Ok(Self(validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(ProductId, "product_id");
token_type!(ProductHandle, "product_handle");
token_type!(CollectionId, "collection_id");
token_type!(CollectionHandle, "collection_handle");
token_type!(OrderId, "order_id");
token_type!(CheckoutId, "checkout_id");
token_type!(RefundId, "refund_id");
token_type!(Sku, "sku");
token_type!(CurrencyCode, "currency");
token_type!(EntitlementKey, "entitlement_key");
