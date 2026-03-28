use crate::checkout::CheckoutLine;
use crate::error::CommerceModelError;
use crate::identifiers::{CurrencyCode, OrderId, RefundId, Sku};
use crate::model::{Money, OrderStatus, ProductKind};
use crate::pricing::{PriceQuote, ensure_same_currency};
use crate::validation::require_non_empty;
use coil_data::{DomainWrite, TransactionIsolation, TransactionPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refund {
    pub id: RefundId,
    pub amount: Money,
    pub reason: String,
}

impl Refund {
    pub fn new(
        id: RefundId,
        amount: Money,
        reason: impl Into<String>,
    ) -> Result<Self, CommerceModelError> {
        Ok(Self {
            id,
            amount,
            reason: require_non_empty("refund_reason", reason.into())?,
        })
    }
}

fn format_money(amount: &Money) -> String {
    let minor = amount.amount_minor();
    let major = minor / 100;
    let remainder = minor % 100;

    match amount.currency().as_str() {
        "GBP" => format!("£{major}.{remainder:02}"),
        code => format!("{code} {major}.{remainder:02}"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderOutcome {
    ShipPhysical {
        sku: Sku,
        quantity: u32,
    },
    DeliverDigital {
        sku: Sku,
        quantity: u32,
    },
    ScheduleService {
        sku: Sku,
        quantity: u32,
    },
    GrantMembership {
        entitlement_key: crate::EntitlementKey,
        quantity: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub id: OrderId,
    pub status: OrderStatus,
    pub currency: CurrencyCode,
    pub lines: Vec<CheckoutLine>,
    pub totals: PriceQuote,
    pub refunds: Vec<Refund>,
}

impl Order {
    pub fn refunds(&self) -> &[Refund] {
        &self.refunds
    }

    pub fn confirmation_message(&self) -> String {
        if self.outcomes().iter().any(|outcome| {
            matches!(
                outcome,
                OrderOutcome::GrantMembership {
                    entitlement_key: _,
                    quantity: _
                }
            )
        }) {
            "A confirmation email and membership activation will follow shortly.".to_string()
        } else {
            match self.status {
                OrderStatus::PendingPayment => "Complete payment to place the order.".to_string(),
                OrderStatus::Paid | OrderStatus::Fulfilled => {
                    "A confirmation email will follow shortly.".to_string()
                }
                OrderStatus::PartiallyRefunded => {
                    "A partial refund is being reconciled.".to_string()
                }
                OrderStatus::Refunded => "This order has been refunded.".to_string(),
                OrderStatus::Cancelled => {
                    "This order was cancelled before fulfillment.".to_string()
                }
            }
        }
    }

    pub fn history_status_label(&self) -> &'static str {
        match self.status {
            OrderStatus::PendingPayment => "Awaiting payment",
            OrderStatus::Paid => "Paid",
            OrderStatus::Fulfilled => "Fulfilled",
            OrderStatus::PartiallyRefunded => "Partially refunded",
            OrderStatus::Refunded => "Refunded",
            OrderStatus::Cancelled => "Cancelled",
        }
    }

    pub fn display_total(&self) -> String {
        format_money(&self.totals.total)
    }

    pub fn outcomes(&self) -> Vec<OrderOutcome> {
        self.lines
            .iter()
            .map(|line| match &line.product_kind {
                ProductKind::Physical => OrderOutcome::ShipPhysical {
                    sku: line.sku.clone(),
                    quantity: line.quantity,
                },
                ProductKind::Digital => OrderOutcome::DeliverDigital {
                    sku: line.sku.clone(),
                    quantity: line.quantity,
                },
                ProductKind::Service => OrderOutcome::ScheduleService {
                    sku: line.sku.clone(),
                    quantity: line.quantity,
                },
                ProductKind::Membership { entitlement_key } => OrderOutcome::GrantMembership {
                    entitlement_key: entitlement_key.clone(),
                    quantity: line.quantity,
                },
            })
            .collect()
    }

    pub fn fulfill(&mut self) -> Result<(), CommerceModelError> {
        if self.status != OrderStatus::Paid {
            return Err(CommerceModelError::OrderNotRefundable {
                order_id: self.id.to_string(),
                status: self.status,
            });
        }

        self.status = OrderStatus::Fulfilled;
        Ok(())
    }

    pub fn issue_refund(&mut self, refund: Refund) -> Result<(), CommerceModelError> {
        if !matches!(
            self.status,
            OrderStatus::Paid | OrderStatus::Fulfilled | OrderStatus::PartiallyRefunded
        ) {
            return Err(CommerceModelError::OrderNotRefundable {
                order_id: self.id.to_string(),
                status: self.status,
            });
        }

        ensure_same_currency(&self.currency, refund.amount.currency())?;

        let captured_minor = self.totals.total.amount_minor();
        let refunded_minor: i64 = self
            .refunds
            .iter()
            .map(|existing| existing.amount.amount_minor())
            .sum();
        let requested_minor = refund.amount.amount_minor();

        if refunded_minor + requested_minor > captured_minor {
            return Err(CommerceModelError::RefundExceedsCaptured {
                order_id: self.id.to_string(),
                captured_minor,
                refunded_minor,
                requested_minor,
            });
        }

        self.refunds.push(refund);
        let total_refunded = refunded_minor + requested_minor;
        self.status = if total_refunded == captured_minor {
            OrderStatus::Refunded
        } else {
            OrderStatus::PartiallyRefunded
        };
        Ok(())
    }

    pub fn fulfillment_transaction_plan(&self) -> Result<TransactionPlan, CommerceModelError> {
        if self.status != OrderStatus::Paid {
            return Err(CommerceModelError::OrderNotRefundable {
                order_id: self.id.to_string(),
                status: self.status,
            });
        }

        TransactionPlan::new("commerce.order.fulfill", TransactionIsolation::Serializable)?
            .with_write(DomainWrite::new("order", "update")?)
            .with_write(DomainWrite::new("fulfillment_job", "enqueue")?)
            .with_after_commit_job(format!("commerce.jobs.fulfillment.dispatch:{}", self.id))
            .and_then(|plan| {
                plan.with_after_commit_event(format!(
                    "commerce.order.fulfillment_requested:{}",
                    self.id
                ))
            })
            .map_err(Into::into)
    }

    pub fn refund_transaction_plan(
        &self,
        refund: &Refund,
    ) -> Result<TransactionPlan, CommerceModelError> {
        if !matches!(
            self.status,
            OrderStatus::Paid | OrderStatus::Fulfilled | OrderStatus::PartiallyRefunded
        ) {
            return Err(CommerceModelError::OrderNotRefundable {
                order_id: self.id.to_string(),
                status: self.status,
            });
        }

        TransactionPlan::new("commerce.order.refund", TransactionIsolation::Serializable)?
            .with_write(DomainWrite::new("order_refund", "insert")?)
            .with_write(DomainWrite::new("order", "update")?)
            .with_write(DomainWrite::new("payment_refund", "request")?)
            .with_after_commit_job(format!("commerce.jobs.refund.reconcile:{}", refund.id))
            .and_then(|plan| {
                plan.with_after_commit_event(format!("commerce.order.refund_issued:{}", refund.id))
            })
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkout::CheckoutSession;
    use crate::identifiers::{CheckoutId, CurrencyCode, EntitlementKey, OrderId, ProductId, Sku};
    use crate::model::ProductKind;
    use crate::pricing::PricingPolicy;

    fn gbp(amount_minor: i64) -> Money {
        Money::new(CurrencyCode::new("GBP").unwrap(), amount_minor).unwrap()
    }

    fn membership_order() -> Order {
        let mut checkout = CheckoutSession::new(
            CheckoutId::new("chk-order").unwrap(),
            CurrencyCode::new("GBP").unwrap(),
        );
        checkout
            .add_line(
                CheckoutLine::new(
                    ProductId::new("product-gold-membership").unwrap(),
                    ProductKind::Membership {
                        entitlement_key: EntitlementKey::new("membership.gold").unwrap(),
                    },
                    "Gold Membership",
                    Sku::new("sku-gold-membership").unwrap(),
                    "Annual plan",
                    1,
                    gbp(8_900),
                )
                .unwrap(),
            )
            .unwrap();
        checkout.ready_for_payment().unwrap();
        checkout.awaiting_payment().unwrap();
        checkout.mark_paid().unwrap();
        checkout
            .finalize(
                OrderId::new("ord-order").unwrap(),
                &PricingPolicy::new(CurrencyCode::new("GBP").unwrap()),
            )
            .unwrap()
    }

    #[test]
    fn membership_orders_expose_confirmation_and_history_copy() {
        let order = membership_order();

        assert!(
            order
                .confirmation_message()
                .contains("membership activation")
        );
        assert_eq!(order.history_status_label(), "Paid");
        assert_eq!(order.display_total(), "£89.00");
    }
}
