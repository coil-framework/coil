use crate::error::CommerceModelError;
use crate::identifiers::{CheckoutId, CurrencyCode, OrderId, Sku};
use crate::model::{CheckoutStatus, Money, OrderStatus, ProductKind};
use crate::orders::Order;
use crate::pricing::{PriceQuote, PricingPolicy, ensure_same_currency};
use crate::validation::require_non_empty;
use davenda_data::{DomainWrite, TransactionIsolation, TransactionPlan};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckoutLine {
    pub product_id: crate::ProductId,
    pub product_kind: ProductKind,
    pub product_title: String,
    pub sku: Sku,
    pub variant_title: String,
    pub quantity: u32,
    pub unit_price: Money,
}

impl CheckoutLine {
    pub fn new(
        product_id: crate::ProductId,
        product_kind: ProductKind,
        product_title: impl Into<String>,
        sku: Sku,
        variant_title: impl Into<String>,
        quantity: u32,
        unit_price: Money,
    ) -> Result<Self, CommerceModelError> {
        if quantity == 0 {
            return Err(CommerceModelError::ZeroQuantity { field: "quantity" });
        }

        Ok(Self {
            product_id,
            product_kind,
            product_title: require_non_empty("product_title", product_title.into())?,
            sku,
            variant_title: require_non_empty("variant_title", variant_title.into())?,
            quantity,
            unit_price,
        })
    }

    pub fn subtotal(&self) -> Result<Money, CommerceModelError> {
        self.unit_price.checked_mul(self.quantity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckoutSession {
    pub id: CheckoutId,
    pub currency: CurrencyCode,
    pub status: CheckoutStatus,
    lines: BTreeMap<Sku, CheckoutLine>,
}

impl CheckoutSession {
    pub fn new(id: CheckoutId, currency: CurrencyCode) -> Self {
        Self {
            id,
            currency,
            status: CheckoutStatus::Draft,
            lines: BTreeMap::new(),
        }
    }

    pub fn lines(&self) -> impl Iterator<Item = &CheckoutLine> {
        self.lines.values()
    }

    pub fn line(&self, sku: &Sku) -> Result<&CheckoutLine, CommerceModelError> {
        self.lines
            .get(sku)
            .ok_or_else(|| CommerceModelError::MissingLine {
                sku: sku.to_string(),
            })
    }

    pub fn add_line(&mut self, line: CheckoutLine) -> Result<(), CommerceModelError> {
        ensure_same_currency(&self.currency, line.unit_price.currency())?;
        if let Some(existing) = self.lines.get_mut(&line.sku) {
            existing.quantity = existing.quantity.checked_add(line.quantity).ok_or(
                CommerceModelError::AmountOverflow {
                    field: "checkout_line_quantity",
                },
            )?;
        } else {
            self.lines.insert(line.sku.clone(), line);
        }
        Ok(())
    }

    pub fn remove_line(&mut self, sku: &Sku) -> Option<CheckoutLine> {
        self.lines.remove(sku)
    }

    pub fn replace_quantity(&mut self, sku: &Sku, quantity: u32) -> Result<(), CommerceModelError> {
        if quantity == 0 {
            return Err(CommerceModelError::ZeroQuantity { field: "quantity" });
        }

        let line = self
            .lines
            .get_mut(sku)
            .ok_or_else(|| CommerceModelError::MissingLine {
                sku: sku.to_string(),
            })?;
        line.quantity = quantity;
        Ok(())
    }

    pub fn price(&self, policy: &PricingPolicy) -> Result<PriceQuote, CommerceModelError> {
        ensure_same_currency(&self.currency, &policy.currency)?;

        let mut subtotal = Money::zero(self.currency.clone());
        for line in self.lines() {
            subtotal = subtotal.checked_add(&line.subtotal()?)?;
        }

        let adjustments = policy.adjustments_for_subtotal(&subtotal)?;
        PriceQuote::new(subtotal, adjustments)
    }

    pub fn ready_for_payment(&mut self) -> Result<(), CommerceModelError> {
        if self.lines.is_empty() {
            return Err(CommerceModelError::EmptyCheckout);
        }

        self.transition_to(CheckoutStatus::ReadyForPayment)
    }

    pub fn awaiting_payment(&mut self) -> Result<(), CommerceModelError> {
        if self.status != CheckoutStatus::ReadyForPayment {
            return Err(CommerceModelError::CheckoutNotReady {
                status: self.status,
            });
        }

        self.transition_to(CheckoutStatus::AwaitingPayment)
    }

    pub fn mark_paid(&mut self) -> Result<(), CommerceModelError> {
        if self.status != CheckoutStatus::AwaitingPayment {
            return Err(CommerceModelError::CheckoutNotReady {
                status: self.status,
            });
        }

        self.transition_to(CheckoutStatus::Paid)
    }

    pub fn complete(&mut self) -> Result<(), CommerceModelError> {
        if self.status != CheckoutStatus::Paid {
            return Err(CommerceModelError::CheckoutNotReady {
                status: self.status,
            });
        }

        self.transition_to(CheckoutStatus::Completed)
    }

    pub fn cancel(&mut self) -> Result<(), CommerceModelError> {
        self.transition_to(CheckoutStatus::Cancelled)
    }

    pub fn to_order(
        &self,
        order_id: OrderId,
        pricing: &PricingPolicy,
    ) -> Result<Order, CommerceModelError> {
        if self.lines.is_empty() {
            return Err(CommerceModelError::EmptyCheckout);
        }

        let status = match self.status {
            CheckoutStatus::Paid | CheckoutStatus::Completed => OrderStatus::Paid,
            CheckoutStatus::Cancelled => OrderStatus::Cancelled,
            _ => OrderStatus::PendingPayment,
        };

        Ok(Order {
            id: order_id,
            status,
            currency: self.currency.clone(),
            lines: self.lines.values().cloned().collect(),
            totals: self.price(pricing)?,
            refunds: Vec::new(),
        })
    }

    pub fn completion_transaction_plan(
        &self,
        order: &Order,
    ) -> Result<TransactionPlan, CommerceModelError> {
        TransactionPlan::new(
            "commerce.checkout.complete",
            TransactionIsolation::Serializable,
        )?
        .with_write(DomainWrite::new("checkout_session", "update")?)
        .with_write(DomainWrite::new("checkout_line", "replace")?)
        .with_write(DomainWrite::new("order", "insert")?)
        .with_write(DomainWrite::new("inventory_reservation", "insert")?)
        .with_after_commit_job(format!("commerce.jobs.fulfillment.prepare:{}", order.id))
        .and_then(|plan| {
            plan.with_after_commit_event(format!("commerce.order.created:{}", order.id))
        })
        .map_err(Into::into)
    }

    fn transition_to(&mut self, next: CheckoutStatus) -> Result<(), CommerceModelError> {
        let valid = matches!(
            (self.status, next),
            (CheckoutStatus::Draft, CheckoutStatus::ReadyForPayment)
                | (
                    CheckoutStatus::ReadyForPayment,
                    CheckoutStatus::AwaitingPayment
                )
                | (CheckoutStatus::AwaitingPayment, CheckoutStatus::Paid)
                | (CheckoutStatus::Paid, CheckoutStatus::Completed)
                | (
                    CheckoutStatus::Draft
                        | CheckoutStatus::ReadyForPayment
                        | CheckoutStatus::AwaitingPayment
                        | CheckoutStatus::Paid,
                    CheckoutStatus::Cancelled
                )
        );

        if valid {
            self.status = next;
            Ok(())
        } else {
            Err(CommerceModelError::InvalidStatusTransition {
                from: self.status,
                to: next,
            })
        }
    }
}
