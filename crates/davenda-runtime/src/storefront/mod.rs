use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::*;

const DEFAULT_CURRENCY: &str = "GBP";
const INITIAL_ORDER_SEQUENCE: i64 = 10_042;
const STOREFRONT_FORM_STATE_PREFIX: &str = "__davenda_storefront_form_state__:";

#[derive(Debug, Error)]
pub enum StorefrontStateError {
    #[error("storefront state store is poisoned")]
    Poisoned,
    #[error("unknown storefront sku `{sku}`")]
    UnknownSku { sku: String },
    #[error("quantity must be greater than zero")]
    InvalidQuantity,
    #[error("checkout requires a payment method")]
    MissingPaymentMethod,
    #[error("checkout requires a billing email")]
    MissingCheckoutEmail,
    #[error("card payments require a 4-digit last4 value")]
    InvalidPaymentLast4,
    #[error("checkout completion requires a payment intent reference")]
    MissingPaymentIntent,
    #[error("checkout intent `{received}` does not match the reserved payment intent `{expected}`")]
    PaymentIntentMismatch { expected: String, received: String },
    #[error("checkout for session `{session_id}` is not ready for payment")]
    CheckoutNotReady { session_id: String },
    #[error("cart for session `{session_id}` is empty")]
    EmptyCart { session_id: String },
    #[error("payment reference `{payment_reference}` does not match any storefront order")]
    UnknownPaymentReference { payment_reference: String },
    #[error("unknown storefront payment webhook event `{event}`")]
    UnknownPaymentWebhookEvent { event: String },
    #[error("payment webhook verification failed")]
    InvalidPaymentWebhookSignature,
    #[error("payment webhook secret is not configured")]
    MissingPaymentWebhookSecret,
    #[error("failed to serialize storefront state: {reason}")]
    Serialization { reason: String },
    #[error("failed to initialize storefront state store `{path}`: {reason}")]
    Initialization { path: String, reason: String },
    #[error("storefront state query failed: {reason}")]
    Query { reason: String },
}

#[derive(Debug, Clone)]
pub struct StorefrontStateStore {
    path: PathBuf,
    connection: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StorefrontPaymentSnapshot {
    pub status: String,
    pub method: Option<String>,
    pub reference: Option<String>,
    pub last4: Option<String>,
    pub checkout_email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorefrontPaymentInput {
    pub method: String,
    pub last4: Option<String>,
    pub checkout_email: String,
    pub intent_reference: String,
}

impl StorefrontPaymentInput {
    pub fn new(
        method: impl Into<String>,
        checkout_email: impl Into<String>,
        last4: Option<String>,
        intent_reference: impl Into<String>,
    ) -> Result<Self, StorefrontStateError> {
        let method = method.into().trim().to_ascii_lowercase();
        if method.is_empty() {
            return Err(StorefrontStateError::MissingPaymentMethod);
        }
        let checkout_email = checkout_email.into().trim().to_string();
        if checkout_email.is_empty() {
            return Err(StorefrontStateError::MissingCheckoutEmail);
        }
        let last4 = last4
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let has_valid_last4 = last4
            .as_deref()
            .map(|value| value.len() == 4 && value.chars().all(|ch| ch.is_ascii_digit()))
            .unwrap_or(false);
        if method == "card" && !has_valid_last4 {
            return Err(StorefrontStateError::InvalidPaymentLast4);
        }
        let intent_reference = intent_reference.into().trim().to_string();
        if intent_reference.is_empty() {
            return Err(StorefrontStateError::MissingPaymentIntent);
        }
        Ok(Self {
            method,
            last4,
            checkout_email,
            intent_reference,
        })
    }

    pub fn card(
        checkout_email: impl Into<String>,
        last4: impl Into<String>,
        intent_reference: impl Into<String>,
    ) -> Result<Self, StorefrontStateError> {
        Self::new("card", checkout_email, Some(last4.into()), intent_reference)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StorefrontCartLine {
    pub sku: String,
    pub title: String,
    pub variant_title: String,
    pub product_kind: String,
    pub entitlement_key: Option<String>,
    pub quantity: u32,
    pub unit_price_minor: i64,
    pub total_minor: i64,
    pub currency: String,
    pub total: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StorefrontCartSnapshot {
    pub status: String,
    pub payment: StorefrontPaymentSnapshot,
    pub currency: String,
    pub item_count: u32,
    pub subtotal_minor: i64,
    pub subtotal: String,
    pub lines: Vec<StorefrontCartLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StorefrontOrderLine {
    pub sku: String,
    pub title: String,
    pub variant_title: String,
    pub product_kind: String,
    pub entitlement_key: Option<String>,
    pub quantity: u32,
    pub unit_price_minor: i64,
    pub total_minor: i64,
    pub currency: String,
    pub total: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StorefrontOrderSnapshot {
    pub order_id: String,
    pub session_id: String,
    pub principal_id: Option<String>,
    pub status: String,
    pub payment: StorefrontPaymentSnapshot,
    pub currency: String,
    pub line_count: u32,
    pub subtotal_minor: i64,
    pub total_minor: i64,
    pub subtotal: String,
    pub total: String,
    pub created_at_unix_seconds: u64,
    pub lines: Vec<StorefrontOrderLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StorefrontStateSnapshot {
    pub session_id: String,
    pub principal_id: Option<String>,
    pub cart: StorefrontCartSnapshot,
    pub payment: StorefrontPaymentSnapshot,
    pub recent_orders: Vec<StorefrontOrderSnapshot>,
    pub latest_order: Option<StorefrontOrderSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StorefrontOrderHistoryResponse {
    pub session_id: String,
    pub principal_id: Option<String>,
    pub orders: Vec<StorefrontOrderSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorefrontPaymentWebhookReceipt {
    pub order: StorefrontOrderSnapshot,
    pub needs_paid_event_dispatch: bool,
}

#[derive(Debug, Clone)]
pub struct StorefrontResponseAugmentation {
    pub html_fragment: Option<String>,
    pub headers: BTreeMap<String, String>,
}

const ACCOUNT_SESSION_END_ACTION: &str = "commerce.account-session-end";
const ACCOUNT_SESSION_END_PATH: &str = "/account/session/end";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorefrontFormState {
    pub route_name: String,
    pub summary: String,
    pub field_errors: BTreeMap<String, String>,
    pub fields: BTreeMap<String, String>,
}

impl StorefrontFormState {
    pub fn new(route_name: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            route_name: route_name.into(),
            summary: summary.into(),
            field_errors: BTreeMap::new(),
            fields: BTreeMap::new(),
        }
    }

    pub fn with_field_error(
        mut self,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        self.field_errors.insert(field.into(), message.into());
        self
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    pub fn with_field_value(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(field.into(), value.into());
        self
    }

    pub fn encode(&self) -> Result<String, StorefrontStateError> {
        let payload =
            serde_json::to_string(self).map_err(|error| StorefrontStateError::Serialization {
                reason: error.to_string(),
            })?;
        Ok(format!("{STOREFRONT_FORM_STATE_PREFIX}{payload}"))
    }

    pub fn decode(payload: &str) -> Option<Self> {
        let encoded = payload.strip_prefix(STOREFRONT_FORM_STATE_PREFIX)?;
        serde_json::from_str(encoded).ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CatalogItem {
    sku: &'static str,
    title: &'static str,
    variant_title: &'static str,
    product_kind: &'static str,
    entitlement_key: Option<&'static str>,
    unit_price_minor: i64,
}

impl StorefrontStateStore {
    pub fn open_for_plan(plan: &RuntimePlan) -> Result<Self, StorefrontStateError> {
        Self::open_with_root(
            plan.shared_state_root().clone(),
            plan.shared_backend_namespace(),
        )
    }

    pub fn open_with_root(
        root: impl Into<PathBuf>,
        namespace: impl Into<String>,
    ) -> Result<Self, StorefrontStateError> {
        let root = root.into();
        let namespace = namespace.into();
        let path = database_path(&root, &namespace);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                StorefrontStateError::Initialization {
                    path: parent.display().to_string(),
                    reason: error.to_string(),
                }
            })?;
        }

        let connection =
            Connection::open(&path).map_err(|error| StorefrontStateError::Initialization {
                path: path.display().to_string(),
                reason: error.to_string(),
            })?;
        connection
            .execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = FULL;
                CREATE TABLE IF NOT EXISTS carts (
                    session_id TEXT PRIMARY KEY,
                    principal_id TEXT,
                    status TEXT NOT NULL,
                    payment_status TEXT NOT NULL DEFAULT 'not_started',
                    payment_method TEXT,
                    payment_reference TEXT,
                    payment_last4 TEXT,
                    checkout_email TEXT,
                    currency TEXT NOT NULL,
                    updated_at_unix_seconds INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS cart_lines (
                    session_id TEXT NOT NULL,
                    sku TEXT NOT NULL,
                    title TEXT NOT NULL,
                    variant_title TEXT NOT NULL,
                    product_kind TEXT NOT NULL,
                    entitlement_key TEXT,
                    quantity INTEGER NOT NULL,
                    unit_price_minor INTEGER NOT NULL,
                    currency TEXT NOT NULL,
                    PRIMARY KEY (session_id, sku)
                );
                CREATE TABLE IF NOT EXISTS storefront_sequences (
                    name TEXT PRIMARY KEY,
                    next_value INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS orders (
                    order_id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    principal_id TEXT,
                    status TEXT NOT NULL,
                    payment_status TEXT NOT NULL DEFAULT 'captured',
                    payment_method TEXT,
                    payment_reference TEXT,
                    payment_last4 TEXT,
                    checkout_email TEXT,
                    currency TEXT NOT NULL,
                    line_count INTEGER NOT NULL,
                    subtotal_minor INTEGER NOT NULL,
                    total_minor INTEGER NOT NULL,
                    order_paid_event_dispatched_at_unix_seconds INTEGER,
                    created_at_unix_seconds INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS order_lines (
                    order_id TEXT NOT NULL,
                    sku TEXT NOT NULL,
                    title TEXT NOT NULL,
                    variant_title TEXT NOT NULL,
                    product_kind TEXT NOT NULL,
                    entitlement_key TEXT,
                    quantity INTEGER NOT NULL,
                    unit_price_minor INTEGER NOT NULL,
                    currency TEXT NOT NULL,
                    PRIMARY KEY (order_id, sku)
                );
                CREATE INDEX IF NOT EXISTS orders_by_session
                    ON orders (session_id, created_at_unix_seconds DESC);
                CREATE INDEX IF NOT EXISTS orders_by_principal
                    ON orders (principal_id, created_at_unix_seconds DESC);
                INSERT INTO storefront_sequences (name, next_value)
                VALUES ('order', 10042)
                ON CONFLICT(name) DO NOTHING;
                INSERT INTO storefront_sequences (name, next_value)
                VALUES ('payment', 50001)
                ON CONFLICT(name) DO NOTHING;
                "#,
            )
            .map_err(|error| StorefrontStateError::Initialization {
                path: path.display().to_string(),
                reason: error.to_string(),
            })?;
        ensure_storefront_columns(&connection)?;

        Ok(Self {
            path,
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn snapshot(
        &self,
        session_id: &str,
        principal_id: Option<&str>,
    ) -> Result<StorefrontStateSnapshot, StorefrontStateError> {
        let mut connection = self.lock_connection()?;
        let tx = connection.transaction().map_err(|error| {
            query_error(format!("failed to start storefront snapshot: {error}"))
        })?;
        self.ensure_cart(&tx, session_id, principal_id, "active", 0)?;
        let snapshot = self.load_snapshot(&tx, session_id, principal_id, 50)?;
        tx.commit().map_err(|error| {
            query_error(format!("failed to commit storefront snapshot: {error}"))
        })?;
        Ok(snapshot)
    }

    pub fn add_to_cart(
        &self,
        session_id: &str,
        principal_id: Option<&str>,
        sku: &str,
        quantity: u32,
        now_unix_seconds: u64,
    ) -> Result<StorefrontStateSnapshot, StorefrontStateError> {
        if quantity == 0 {
            return Err(StorefrontStateError::InvalidQuantity);
        }

        let item = catalog_item(sku)?;
        let mut connection = self.lock_connection()?;
        let tx = connection.transaction().map_err(|error| {
            query_error(format!("failed to start add-to-cart transaction: {error}"))
        })?;
        self.ensure_cart(&tx, session_id, principal_id, "active", now_unix_seconds)?;
        tx.execute(
            r#"
            INSERT INTO cart_lines (
                session_id, sku, title, variant_title, product_kind, entitlement_key,
                quantity, unit_price_minor, currency
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(session_id, sku) DO UPDATE SET
                quantity = cart_lines.quantity + excluded.quantity,
                title = excluded.title,
                variant_title = excluded.variant_title,
                product_kind = excluded.product_kind,
                entitlement_key = excluded.entitlement_key,
                unit_price_minor = excluded.unit_price_minor,
                currency = excluded.currency
            "#,
            params![
                session_id,
                item.sku,
                item.title,
                item.variant_title,
                item.product_kind,
                item.entitlement_key,
                i64::from(quantity),
                item.unit_price_minor,
                DEFAULT_CURRENCY,
            ],
        )
        .map_err(|error| query_error(format!("failed to add storefront cart line: {error}")))?;
        self.touch_cart(&tx, session_id, principal_id, "active", now_unix_seconds)?;
        self.update_cart_payment(&tx, session_id, "not_started", None, None, None, None)?;
        let snapshot = self.load_snapshot(&tx, session_id, principal_id, 50)?;
        tx.commit().map_err(|error| {
            query_error(format!("failed to commit add-to-cart transaction: {error}"))
        })?;
        Ok(snapshot)
    }

    pub fn update_cart(
        &self,
        session_id: &str,
        principal_id: Option<&str>,
        sku: &str,
        quantity: u32,
        now_unix_seconds: u64,
    ) -> Result<StorefrontStateSnapshot, StorefrontStateError> {
        let mut connection = self.lock_connection()?;
        let tx = connection.transaction().map_err(|error| {
            query_error(format!("failed to start cart update transaction: {error}"))
        })?;
        self.ensure_cart(&tx, session_id, principal_id, "active", now_unix_seconds)?;
        if quantity == 0 {
            tx.execute(
                "DELETE FROM cart_lines WHERE session_id = ?1 AND sku = ?2",
                params![session_id, sku],
            )
            .map_err(|error| {
                query_error(format!("failed to remove storefront cart line: {error}"))
            })?;
        } else {
            let item = catalog_item(sku)?;
            tx.execute(
                r#"
                INSERT INTO cart_lines (
                    session_id, sku, title, variant_title, product_kind, entitlement_key,
                    quantity, unit_price_minor, currency
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(session_id, sku) DO UPDATE SET
                    quantity = excluded.quantity,
                    title = excluded.title,
                    variant_title = excluded.variant_title,
                    product_kind = excluded.product_kind,
                    entitlement_key = excluded.entitlement_key,
                    unit_price_minor = excluded.unit_price_minor,
                    currency = excluded.currency
                "#,
                params![
                    session_id,
                    item.sku,
                    item.title,
                    item.variant_title,
                    item.product_kind,
                    item.entitlement_key,
                    i64::from(quantity),
                    item.unit_price_minor,
                    DEFAULT_CURRENCY,
                ],
            )
            .map_err(|error| {
                query_error(format!("failed to update storefront cart line: {error}"))
            })?;
        }
        self.touch_cart(&tx, session_id, principal_id, "active", now_unix_seconds)?;
        self.update_cart_payment(&tx, session_id, "not_started", None, None, None, None)?;
        let snapshot = self.load_snapshot(&tx, session_id, principal_id, 50)?;
        tx.commit().map_err(|error| {
            query_error(format!("failed to commit cart update transaction: {error}"))
        })?;
        Ok(snapshot)
    }

    pub fn checkout_start(
        &self,
        session_id: &str,
        principal_id: Option<&str>,
        now_unix_seconds: u64,
    ) -> Result<StorefrontStateSnapshot, StorefrontStateError> {
        let mut connection = self.lock_connection()?;
        let tx = connection.transaction().map_err(|error| {
            query_error(format!(
                "failed to start checkout-start transaction: {error}"
            ))
        })?;
        self.ensure_cart(&tx, session_id, principal_id, "active", now_unix_seconds)?;
        if self.cart_line_count(&tx, session_id)? == 0 {
            return Err(StorefrontStateError::EmptyCart {
                session_id: session_id.to_string(),
            });
        }
        let payment_reference = self
            .load_cart_payment(&tx, session_id)?
            .reference
            .unwrap_or(next_payment_reference(&tx)?);
        self.touch_cart(&tx, session_id, principal_id, "checkout", now_unix_seconds)?;
        self.update_cart_payment(
            &tx,
            session_id,
            "ready_for_payment",
            None,
            None,
            None,
            Some(payment_reference.as_str()),
        )?;
        let snapshot = self.load_snapshot(&tx, session_id, principal_id, 50)?;
        tx.commit().map_err(|error| {
            query_error(format!(
                "failed to commit checkout-start transaction: {error}"
            ))
        })?;
        Ok(snapshot)
    }

    pub fn checkout_complete(
        &self,
        session_id: &str,
        principal_id: Option<&str>,
        payment: &StorefrontPaymentInput,
        now_unix_seconds: u64,
    ) -> Result<StorefrontStateSnapshot, StorefrontStateError> {
        let mut connection = self.lock_connection()?;
        let tx = connection.transaction().map_err(|error| {
            query_error(format!(
                "failed to start checkout-complete transaction: {error}"
            ))
        })?;
        self.ensure_cart(&tx, session_id, principal_id, "checkout", now_unix_seconds)?;
        let lines = self.load_cart_lines(&tx, session_id)?;
        if lines.is_empty() {
            return Err(StorefrontStateError::EmptyCart {
                session_id: session_id.to_string(),
            });
        }
        let (cart_status, payment_status) = self.cart_status(&tx, session_id)?;
        if cart_status != "checkout" || payment_status != "ready_for_payment" {
            return Err(StorefrontStateError::CheckoutNotReady {
                session_id: session_id.to_string(),
            });
        }

        let subtotal_minor = lines.iter().map(|line| line.total_minor).sum::<i64>();
        let line_count = lines.iter().map(|line| line.quantity).sum::<u32>();
        let order_id = next_order_id(&tx)?;
        let reserved_payment = self.load_cart_payment(&tx, session_id)?;
        let payment_reference = reserved_payment
            .reference
            .ok_or(StorefrontStateError::MissingPaymentIntent)?;
        if payment.intent_reference != payment_reference {
            return Err(StorefrontStateError::PaymentIntentMismatch {
                expected: payment_reference,
                received: payment.intent_reference.clone(),
            });
        }
        tx.execute(
            r#"
            INSERT INTO orders (
                order_id, session_id, principal_id, status, payment_status, payment_method,
                payment_reference, payment_last4, checkout_email, currency,
                line_count, subtotal_minor, total_minor, created_at_unix_seconds
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            "#,
            params![
                order_id,
                session_id,
                principal_id,
                "pending_payment",
                "provider_pending",
                payment.method.as_str(),
                payment.intent_reference.as_str(),
                payment.last4.as_deref(),
                payment.checkout_email.as_str(),
                DEFAULT_CURRENCY,
                i64::from(line_count),
                subtotal_minor,
                subtotal_minor,
                saturating_i64(now_unix_seconds),
            ],
        )
        .map_err(|error| query_error(format!("failed to create storefront order: {error}")))?;
        for line in &lines {
            tx.execute(
                r#"
                INSERT INTO order_lines (
                    order_id, sku, title, variant_title, product_kind,
                    entitlement_key, quantity, unit_price_minor, currency
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                params![
                    order_id,
                    line.sku,
                    line.title,
                    line.variant_title,
                    line.product_kind,
                    line.entitlement_key,
                    i64::from(line.quantity),
                    line.unit_price_minor,
                    line.currency,
                ],
            )
            .map_err(|error| {
                query_error(format!("failed to persist storefront order line: {error}"))
            })?;
        }
        tx.execute(
            "DELETE FROM cart_lines WHERE session_id = ?1",
            params![session_id],
        )
        .map_err(|error| query_error(format!("failed to clear storefront cart lines: {error}")))?;
        self.touch_cart(&tx, session_id, principal_id, "completed", now_unix_seconds)?;
        self.update_cart_payment(
            &tx,
            session_id,
            "provider_pending",
            Some(payment.method.as_str()),
            payment.last4.as_deref(),
            Some(payment.checkout_email.as_str()),
            Some(payment.intent_reference.as_str()),
        )?;
        let snapshot = self.load_snapshot(&tx, session_id, principal_id, 50)?;
        tx.commit().map_err(|error| {
            query_error(format!(
                "failed to commit checkout-complete transaction: {error}"
            ))
        })?;
        Ok(snapshot)
    }

    pub fn order_history(
        &self,
        session_id: &str,
        principal_id: Option<&str>,
        limit: usize,
    ) -> Result<StorefrontOrderHistoryResponse, StorefrontStateError> {
        let mut connection = self.lock_connection()?;
        let tx = connection.transaction().map_err(|error| {
            query_error(format!(
                "failed to start storefront order-history transaction: {error}"
            ))
        })?;
        self.ensure_cart(&tx, session_id, principal_id, "active", 0)?;
        let orders = self.load_orders(&tx, session_id, principal_id, limit)?;
        tx.commit().map_err(|error| {
            query_error(format!(
                "failed to commit storefront order-history transaction: {error}"
            ))
        })?;
        Ok(StorefrontOrderHistoryResponse {
            session_id: session_id.to_string(),
            principal_id: principal_id.map(ToOwned::to_owned),
            orders,
        })
    }

    pub fn apply_payment_webhook(
        &self,
        payment_reference: &str,
        event: &str,
        now_unix_seconds: u64,
    ) -> Result<StorefrontPaymentWebhookReceipt, StorefrontStateError> {
        let mut connection = self.lock_connection()?;
        let tx = connection.transaction().map_err(|error| {
            query_error(format!(
                "failed to start storefront payment-webhook transaction: {error}"
            ))
        })?;

        let Some((
            order_id,
            session_id,
            principal_id,
            current_status,
            payment_status,
            paid_event_dispatched_at,
        )) =
            self.load_order_header_by_payment_reference(&tx, payment_reference)?
        else {
            return Err(StorefrontStateError::UnknownPaymentReference {
                payment_reference: payment_reference.to_string(),
            });
        };

        let payment_is_final = payment_status == "captured";
        let (next_status, next_payment_status) = match event {
            "payment.captured" | "payment.succeeded" | "commerce.order.paid" => (
                if current_status == "fulfilled" {
                    current_status.clone()
                } else {
                    "paid".to_string()
                },
                "captured".to_string(),
            ),
            "payment.authorized" => {
                if payment_is_final {
                    (current_status.clone(), payment_status.clone())
                } else {
                    (current_status.clone(), "authorized".to_string())
                }
            }
            "payment.failed" => {
                if payment_is_final {
                    (current_status.clone(), payment_status.clone())
                } else {
                    ("pending_payment".to_string(), "failed".to_string())
                }
            }
            other => {
                return Err(StorefrontStateError::UnknownPaymentWebhookEvent {
                    event: other.to_string(),
                });
            }
        };
        let needs_paid_event_dispatch = next_payment_status == "captured"
            && matches!(next_status.as_str(), "paid" | "fulfilled")
            && paid_event_dispatched_at.is_none();

        if current_status != next_status || payment_status != next_payment_status {
            tx.execute(
                r#"
                UPDATE orders
                SET status = ?2,
                    payment_status = ?3
                WHERE order_id = ?1
                "#,
                params![order_id, next_status, next_payment_status],
            )
            .map_err(|error| {
                query_error(format!(
                    "failed to update storefront order payment state: {error}"
                ))
            })?;
        }

        let cart_payment = self.load_cart_payment(&tx, session_id.as_str())?;
        self.touch_cart(
            &tx,
            session_id.as_str(),
            principal_id.as_deref(),
            "completed",
            now_unix_seconds,
        )?;
        self.update_cart_payment(
            &tx,
            session_id.as_str(),
            next_payment_status.as_str(),
            cart_payment.method.as_deref(),
            cart_payment.last4.as_deref(),
            cart_payment.checkout_email.as_deref(),
            Some(payment_reference),
        )?;

        let order = self
            .load_order_by_id(&tx, order_id.as_str())
            .map_err(|error| {
                query_error(format!("failed to load updated storefront order: {error}"))
            })?
            .ok_or_else(|| {
                query_error(format!(
                    "updated storefront order `{order_id}` could not be reloaded"
                ))
            })?;
        tx.commit().map_err(|error| {
            query_error(format!(
                "failed to commit storefront payment-webhook transaction: {error}"
            ))
        })?;
        Ok(StorefrontPaymentWebhookReceipt {
            order,
            needs_paid_event_dispatch,
        })
    }

    pub fn mark_order_paid_event_dispatched(
        &self,
        order_id: &str,
        now_unix_seconds: u64,
    ) -> Result<(), StorefrontStateError> {
        let mut connection = self.lock_connection()?;
        let tx = connection.transaction().map_err(|error| {
            query_error(format!(
                "failed to start storefront paid-event-dispatch transaction: {error}"
            ))
        })?;
        tx.execute(
            r#"
            UPDATE orders
            SET order_paid_event_dispatched_at_unix_seconds = COALESCE(
                order_paid_event_dispatched_at_unix_seconds,
                ?2
            )
            WHERE order_id = ?1
            "#,
            params![order_id, saturating_i64(now_unix_seconds)],
        )
        .map_err(|error| {
            query_error(format!(
                "failed to mark storefront order paid event dispatched: {error}"
            ))
        })?;
        tx.commit().map_err(|error| {
            query_error(format!(
                "failed to commit storefront paid-event-dispatch transaction: {error}"
            ))
        })?;
        Ok(())
    }

    pub fn build_response_augmentation(
        &self,
        route_name: &str,
        snapshot: &StorefrontStateSnapshot,
        csrf_tokens: BTreeMap<String, String>,
    ) -> Result<StorefrontResponseAugmentation, StorefrontStateError> {
        let account_session_end_markup = csrf_tokens
            .get(ACCOUNT_SESSION_END_ACTION)
            .map(|token| account_session_end_form_markup(token));
        let payload = serde_json::to_string(&serde_json::json!({
            "route": route_name,
            "sessionId": snapshot.session_id,
            "principalId": snapshot.principal_id,
            "cart": snapshot.cart,
            "payment": snapshot.payment,
            "recentOrders": snapshot.recent_orders,
            "latestOrder": snapshot.latest_order,
            "csrf": csrf_tokens,
        }))
        .map_err(|error| StorefrontStateError::Serialization {
            reason: error.to_string(),
        })?;
        let escaped = payload.replace("</script", "<\\/script");
        let mut html_fragment = format!(
            r#"<script id="davenda-storefront-state" type="application/json">{escaped}</script>"#
        );
        if let Some(markup) = account_session_end_markup {
            html_fragment.push_str(&markup);
        }
        let mut headers = BTreeMap::new();
        headers.insert(
            "x-davenda-storefront-cart-items".to_string(),
            snapshot.cart.item_count.to_string(),
        );
        headers.insert(
            "x-davenda-storefront-cart-subtotal-minor".to_string(),
            snapshot.cart.subtotal_minor.to_string(),
        );
        headers.insert(
            "x-davenda-storefront-cart-status".to_string(),
            snapshot.cart.status.clone(),
        );
        headers.insert(
            "x-davenda-storefront-payment-status".to_string(),
            snapshot.payment.status.clone(),
        );
        if let Some(method) = &snapshot.payment.method {
            headers.insert(
                "x-davenda-storefront-payment-method".to_string(),
                method.clone(),
            );
        }
        if let Some(reference) = &snapshot.payment.reference {
            headers.insert(
                "x-davenda-storefront-payment-reference".to_string(),
                reference.clone(),
            );
        }
        headers.insert(
            "x-davenda-storefront-order-count".to_string(),
            snapshot.recent_orders.len().to_string(),
        );
        if let Some(order) = &snapshot.latest_order {
            headers.insert(
                "x-davenda-storefront-latest-order".to_string(),
                order.order_id.clone(),
            );
            headers.insert(
                "x-davenda-storefront-latest-order-status".to_string(),
                order.status.clone(),
            );
        }
        for (action, token) in csrf_tokens {
            headers.insert(
                format!("x-davenda-storefront-csrf-{}", action.replace('.', "-")),
                token,
            );
        }
        Ok(StorefrontResponseAugmentation {
            html_fragment: Some(html_fragment),
            headers,
        })
    }

    fn lock_connection(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Connection>, StorefrontStateError> {
        self.connection
            .lock()
            .map_err(|_| StorefrontStateError::Poisoned)
    }

    fn ensure_cart(
        &self,
        tx: &Transaction<'_>,
        session_id: &str,
        principal_id: Option<&str>,
        status: &str,
        now_unix_seconds: u64,
    ) -> Result<(), StorefrontStateError> {
        tx.execute(
            r#"
            INSERT INTO carts (
                session_id,
                principal_id,
                status,
                payment_status,
                payment_method,
                payment_reference,
                payment_last4,
                checkout_email,
                currency,
                updated_at_unix_seconds
            )
            VALUES (?1, ?2, ?3, 'not_started', NULL, NULL, NULL, NULL, ?4, ?5)
            ON CONFLICT(session_id) DO UPDATE SET
                principal_id = COALESCE(excluded.principal_id, carts.principal_id),
                status = CASE
                    WHEN carts.status = 'completed' AND excluded.status = 'active' THEN carts.status
                    ELSE excluded.status
                END,
                currency = excluded.currency,
                updated_at_unix_seconds = MAX(carts.updated_at_unix_seconds, excluded.updated_at_unix_seconds)
            "#,
            params![
                session_id,
                principal_id,
                status,
                DEFAULT_CURRENCY,
                saturating_i64(now_unix_seconds),
            ],
        )
        .map_err(|error| query_error(format!("failed to ensure storefront cart: {error}")))?;
        Ok(())
    }

    fn touch_cart(
        &self,
        tx: &Transaction<'_>,
        session_id: &str,
        principal_id: Option<&str>,
        status: &str,
        now_unix_seconds: u64,
    ) -> Result<(), StorefrontStateError> {
        tx.execute(
            r#"
            UPDATE carts
            SET principal_id = COALESCE(?2, principal_id),
                status = ?3,
                updated_at_unix_seconds = ?4
            WHERE session_id = ?1
            "#,
            params![
                session_id,
                principal_id,
                status,
                saturating_i64(now_unix_seconds),
            ],
        )
        .map_err(|error| query_error(format!("failed to update storefront cart: {error}")))?;
        Ok(())
    }

    fn update_cart_payment(
        &self,
        tx: &Transaction<'_>,
        session_id: &str,
        payment_status: &str,
        payment_method: Option<&str>,
        payment_last4: Option<&str>,
        checkout_email: Option<&str>,
        payment_reference: Option<&str>,
    ) -> Result<(), StorefrontStateError> {
        tx.execute(
            r#"
            UPDATE carts
            SET payment_status = ?2,
                payment_method = ?3,
                payment_reference = ?4,
                payment_last4 = ?5,
                checkout_email = ?6
            WHERE session_id = ?1
            "#,
            params![
                session_id,
                payment_status,
                payment_method,
                payment_reference,
                payment_last4,
                checkout_email,
            ],
        )
        .map_err(|error| {
            query_error(format!(
                "failed to update storefront payment state: {error}"
            ))
        })?;
        Ok(())
    }

    fn cart_status(
        &self,
        tx: &Transaction<'_>,
        session_id: &str,
    ) -> Result<(String, String), StorefrontStateError> {
        tx.query_row(
            "SELECT status, payment_status FROM carts WHERE session_id = ?1",
            params![session_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| query_error(format!("failed to read storefront cart state: {error}")))?
        .ok_or_else(|| StorefrontStateError::CheckoutNotReady {
            session_id: session_id.to_string(),
        })
    }

    fn cart_line_count(
        &self,
        tx: &Transaction<'_>,
        session_id: &str,
    ) -> Result<usize, StorefrontStateError> {
        let count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM cart_lines WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|error| {
                query_error(format!("failed to count storefront cart lines: {error}"))
            })?;
        usize::try_from(count).map_err(|_| query_error("storefront cart count overflowed".into()))
    }

    fn load_snapshot(
        &self,
        tx: &Transaction<'_>,
        session_id: &str,
        principal_id: Option<&str>,
        order_limit: usize,
    ) -> Result<StorefrontStateSnapshot, StorefrontStateError> {
        let cart = self.load_cart(tx, session_id)?;
        let recent_orders = self.load_orders(tx, session_id, principal_id, order_limit)?;
        let latest_order = recent_orders.first().cloned();
        let principal = tx
            .query_row(
                "SELECT principal_id FROM carts WHERE session_id = ?1",
                params![session_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|error| query_error(format!("failed to load storefront principal: {error}")))?
            .flatten()
            .or_else(|| principal_id.map(ToOwned::to_owned));
        let payment = cart.payment.clone();
        Ok(StorefrontStateSnapshot {
            session_id: session_id.to_string(),
            principal_id: principal,
            cart,
            payment,
            recent_orders,
            latest_order,
        })
    }

    fn load_cart(
        &self,
        tx: &Transaction<'_>,
        session_id: &str,
    ) -> Result<StorefrontCartSnapshot, StorefrontStateError> {
        let status = tx
            .query_row(
                "SELECT status FROM carts WHERE session_id = ?1",
                params![session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| {
                query_error(format!("failed to load storefront cart status: {error}"))
            })?
            .unwrap_or_else(|| "active".to_string());
        let lines = self.load_cart_lines(tx, session_id)?;
        let item_count = lines.iter().map(|line| line.quantity).sum::<u32>();
        let subtotal_minor = lines.iter().map(|line| line.total_minor).sum::<i64>();
        let payment = self.load_cart_payment(tx, session_id)?;
        Ok(StorefrontCartSnapshot {
            status,
            payment,
            currency: DEFAULT_CURRENCY.to_string(),
            item_count,
            subtotal_minor,
            subtotal: format_minor_currency(subtotal_minor),
            lines,
        })
    }

    fn load_cart_lines(
        &self,
        tx: &Transaction<'_>,
        session_id: &str,
    ) -> Result<Vec<StorefrontCartLine>, StorefrontStateError> {
        let mut statement = tx
            .prepare(
                r#"
                SELECT
                    sku, title, variant_title, product_kind, entitlement_key,
                    quantity, unit_price_minor, currency
                FROM cart_lines
                WHERE session_id = ?1
                ORDER BY sku ASC
                "#,
            )
            .map_err(|error| {
                query_error(format!(
                    "failed to prepare storefront cart line query: {error}"
                ))
            })?;
        statement
            .query_map(params![session_id], |row| {
                let quantity_i64: i64 = row.get(5)?;
                let quantity = u32::try_from(quantity_i64)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(5, quantity_i64))?;
                let unit_price_minor: i64 = row.get(6)?;
                let total_minor = unit_price_minor.saturating_mul(i64::from(quantity));
                Ok(StorefrontCartLine {
                    sku: row.get(0)?,
                    title: row.get(1)?,
                    variant_title: row.get(2)?,
                    product_kind: row.get(3)?,
                    entitlement_key: row.get(4)?,
                    quantity,
                    unit_price_minor,
                    total_minor,
                    currency: row.get(7)?,
                    total: format_minor_currency(total_minor),
                })
            })
            .map_err(|error| {
                query_error(format!("failed to query storefront cart lines: {error}"))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                query_error(format!("failed to collect storefront cart lines: {error}"))
            })
    }

    fn load_cart_payment(
        &self,
        tx: &Transaction<'_>,
        session_id: &str,
    ) -> Result<StorefrontPaymentSnapshot, StorefrontStateError> {
        tx.query_row(
            r#"
            SELECT
                payment_status,
                payment_method,
                payment_reference,
                payment_last4,
                checkout_email
            FROM carts
            WHERE session_id = ?1
            "#,
            params![session_id],
            |row| {
                Ok(StorefrontPaymentSnapshot {
                    status: row.get::<_, String>(0)?,
                    method: row.get::<_, Option<String>>(1)?,
                    reference: row.get::<_, Option<String>>(2)?,
                    last4: row.get::<_, Option<String>>(3)?,
                    checkout_email: row.get::<_, Option<String>>(4)?,
                })
            },
        )
        .optional()
        .map_err(|error| query_error(format!("failed to load storefront payment state: {error}")))?
        .ok_or_else(|| {
            query_error(format!(
                "storefront cart `{session_id}` is missing payment state"
            ))
        })
    }

    fn load_orders(
        &self,
        tx: &Transaction<'_>,
        session_id: &str,
        principal_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<StorefrontOrderSnapshot>, StorefrontStateError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let mut statement = if principal_id.is_some() {
            tx.prepare(
                r#"
                SELECT
                    order_id, session_id, principal_id, status, payment_status,
                    payment_method, payment_reference, payment_last4, checkout_email,
                    currency, line_count, subtotal_minor, total_minor, created_at_unix_seconds
                FROM orders
                WHERE session_id = ?1 OR principal_id = ?2
                ORDER BY created_at_unix_seconds DESC, order_id DESC
                LIMIT ?3
                "#,
            )
        } else {
            tx.prepare(
                r#"
                SELECT
                    order_id, session_id, principal_id, status, payment_status,
                    payment_method, payment_reference, payment_last4, checkout_email,
                    currency, line_count, subtotal_minor, total_minor, created_at_unix_seconds
                FROM orders
                WHERE session_id = ?1
                ORDER BY created_at_unix_seconds DESC, order_id DESC
                LIMIT ?2
                "#,
            )
        }
        .map_err(|error| {
            query_error(format!("failed to prepare storefront order query: {error}"))
        })?;

        let order_headers = if let Some(principal_id) = principal_id {
            let rows = statement
                .query_map(
                    params![session_id, principal_id, saturating_i64(limit as u64)],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, Option<String>>(6)?,
                            row.get::<_, Option<String>>(7)?,
                            row.get::<_, Option<String>>(8)?,
                            row.get::<_, String>(9)?,
                            row.get::<_, i64>(10)?,
                            row.get::<_, i64>(11)?,
                            row.get::<_, i64>(12)?,
                            row.get::<_, i64>(13)?,
                        ))
                    },
                )
                .map_err(|error| {
                    query_error(format!("failed to query storefront orders: {error}"))
                })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|error| {
                query_error(format!("failed to collect storefront orders: {error}"))
            })?
        } else {
            let rows = statement
                .query_map(params![session_id, saturating_i64(limit as u64)], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, i64>(12)?,
                        row.get::<_, i64>(13)?,
                    ))
                })
                .map_err(|error| {
                    query_error(format!("failed to query storefront orders: {error}"))
                })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|error| {
                query_error(format!("failed to collect storefront orders: {error}"))
            })?
        };

        let mut orders = Vec::with_capacity(order_headers.len());
        for (
            order_id,
            order_session_id,
            order_principal_id,
            status,
            payment_status,
            payment_method,
            payment_reference,
            payment_last4,
            checkout_email,
            currency,
            line_count_i64,
            subtotal_minor,
            total_minor,
            created_i64,
        ) in order_headers
        {
            let lines = self
                .load_order_lines(tx, order_id.as_str())
                .map_err(|error| {
                    query_error(format!("failed to load storefront order lines: {error}"))
                })?;
            orders.push(StorefrontOrderSnapshot {
                order_id,
                session_id: order_session_id,
                principal_id: order_principal_id,
                status,
                payment: StorefrontPaymentSnapshot {
                    status: payment_status,
                    method: payment_method,
                    reference: payment_reference,
                    last4: payment_last4,
                    checkout_email,
                },
                currency,
                line_count: u32::try_from(line_count_i64)
                    .map_err(|_| query_error("storefront order line count overflowed".into()))?,
                subtotal_minor,
                total_minor,
                subtotal: format_minor_currency(subtotal_minor),
                total: format_minor_currency(total_minor),
                created_at_unix_seconds: u64::try_from(created_i64)
                    .map_err(|_| query_error("storefront order timestamp overflowed".into()))?,
                lines,
            });
        }
        Ok(orders)
    }

    fn load_order_lines(
        &self,
        tx: &Transaction<'_>,
        order_id: &str,
    ) -> rusqlite::Result<Vec<StorefrontOrderLine>> {
        let mut statement = tx.prepare(
            r#"
            SELECT
                sku, title, variant_title, product_kind, entitlement_key,
                quantity, unit_price_minor, currency
            FROM order_lines
            WHERE order_id = ?1
            ORDER BY sku ASC
            "#,
        )?;
        statement
            .query_map(params![order_id], |row| {
                let quantity_i64: i64 = row.get(5)?;
                let quantity = u32::try_from(quantity_i64)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(5, quantity_i64))?;
                let unit_price_minor: i64 = row.get(6)?;
                let total_minor = unit_price_minor.saturating_mul(i64::from(quantity));
                Ok(StorefrontOrderLine {
                    sku: row.get(0)?,
                    title: row.get(1)?,
                    variant_title: row.get(2)?,
                    product_kind: row.get(3)?,
                    entitlement_key: row.get(4)?,
                    quantity,
                    unit_price_minor,
                    total_minor,
                    currency: row.get(7)?,
                    total: format_minor_currency(total_minor),
                })
            })?
            .collect()
    }

    fn load_order_header_by_payment_reference(
        &self,
        tx: &Transaction<'_>,
        payment_reference: &str,
    ) -> Result<
        Option<(String, String, Option<String>, String, String, Option<i64>)>,
        StorefrontStateError,
    >
    {
        tx.query_row(
            r#"
            SELECT
                order_id,
                session_id,
                principal_id,
                status,
                payment_status,
                order_paid_event_dispatched_at_unix_seconds
            FROM orders
            WHERE payment_reference = ?1
            ORDER BY created_at_unix_seconds DESC, order_id DESC
            LIMIT 1
            "#,
            params![payment_reference],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                ))
            },
        )
        .optional()
        .map_err(|error| {
            query_error(format!(
                "failed to load storefront order by payment reference: {error}"
            ))
        })
    }

    fn load_order_by_id(
        &self,
        tx: &Transaction<'_>,
        order_id: &str,
    ) -> rusqlite::Result<Option<StorefrontOrderSnapshot>> {
        let order_header = tx
            .query_row(
                r#"
                SELECT
                    order_id, session_id, principal_id, status, payment_status,
                    payment_method, payment_reference, payment_last4, checkout_email,
                    currency, line_count, subtotal_minor, total_minor, created_at_unix_seconds
                FROM orders
                WHERE order_id = ?1
                "#,
                params![order_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, i64>(12)?,
                        row.get::<_, i64>(13)?,
                    ))
                },
            )
            .optional()?;

        let Some((
            order_id,
            session_id,
            principal_id,
            status,
            payment_status,
            payment_method,
            payment_reference,
            payment_last4,
            checkout_email,
            currency,
            line_count_i64,
            subtotal_minor,
            total_minor,
            created_i64,
        )) = order_header
        else {
            return Ok(None);
        };

        let lines = self.load_order_lines(tx, order_id.as_str())?;
        Ok(Some(StorefrontOrderSnapshot {
            order_id,
            session_id,
            principal_id,
            status,
            payment: StorefrontPaymentSnapshot {
                status: payment_status,
                method: payment_method,
                reference: payment_reference,
                last4: payment_last4,
                checkout_email,
            },
            currency,
            line_count: u32::try_from(line_count_i64)
                .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(10, line_count_i64))?,
            subtotal_minor,
            total_minor,
            subtotal: format_minor_currency(subtotal_minor),
            total: format_minor_currency(total_minor),
            created_at_unix_seconds: u64::try_from(created_i64)
                .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(13, created_i64))?,
            lines,
        }))
    }
}

fn account_session_end_form_markup(token: &str) -> String {
    let escaped = escape_html_attribute(token);
    format!(
        r#"<form id="davenda-account-session-end" action="{ACCOUNT_SESSION_END_PATH}" method="post" hidden><input type="hidden" name="_csrf" value="{escaped}" /></form>"#
    )
}

fn escape_html_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn next_order_id(tx: &Transaction<'_>) -> Result<String, StorefrontStateError> {
    let next_value: i64 = tx
        .query_row(
            "SELECT next_value FROM storefront_sequences WHERE name = 'order'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| query_error(format!("failed to load storefront order sequence: {error}")))?
        .unwrap_or(INITIAL_ORDER_SEQUENCE);
    tx.execute(
        r#"
        INSERT INTO storefront_sequences (name, next_value)
        VALUES ('order', ?1)
        ON CONFLICT(name) DO UPDATE SET next_value = excluded.next_value
        "#,
        params![next_value.saturating_add(1)],
    )
    .map_err(|error| {
        query_error(format!(
            "failed to advance storefront order sequence: {error}"
        ))
    })?;
    Ok(format!("ORD-{next_value:05}"))
}

fn next_payment_reference(tx: &Transaction<'_>) -> Result<String, StorefrontStateError> {
    let next_value: i64 = tx
        .query_row(
            "SELECT next_value FROM storefront_sequences WHERE name = 'payment'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| {
            query_error(format!(
                "failed to load storefront payment sequence: {error}"
            ))
        })?
        .unwrap_or(50_001);
    tx.execute(
        r#"
        INSERT INTO storefront_sequences (name, next_value)
        VALUES ('payment', ?1)
        ON CONFLICT(name) DO UPDATE SET next_value = excluded.next_value
        "#,
        params![next_value.saturating_add(1)],
    )
    .map_err(|error| {
        query_error(format!(
            "failed to advance storefront payment sequence: {error}"
        ))
    })?;
    Ok(format!("PAY-{next_value:05}"))
}

fn ensure_storefront_columns(connection: &Connection) -> Result<(), StorefrontStateError> {
    ensure_table_columns(
        connection,
        "carts",
        &[
            "payment_status TEXT NOT NULL DEFAULT 'not_started'",
            "payment_method TEXT",
            "payment_reference TEXT",
            "payment_last4 TEXT",
            "checkout_email TEXT",
        ],
    )?;
    ensure_table_columns(
        connection,
        "orders",
        &[
            "payment_status TEXT NOT NULL DEFAULT 'captured'",
            "payment_method TEXT",
            "payment_reference TEXT",
            "payment_last4 TEXT",
            "checkout_email TEXT",
            "order_paid_event_dispatched_at_unix_seconds INTEGER",
        ],
    )?;
    Ok(())
}

fn ensure_table_columns(
    connection: &Connection,
    table: &str,
    columns: &[&str],
) -> Result<(), StorefrontStateError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| {
            query_error(format!(
                "failed to inspect storefront table `{table}`: {error}"
            ))
        })?;
    let existing = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| {
            query_error(format!(
                "failed to read storefront table `{table}` columns: {error}"
            ))
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            query_error(format!(
                "failed to collect storefront table `{table}` columns: {error}"
            ))
        })?;
    for column in columns {
        let Some(name) = column.split_whitespace().next() else {
            continue;
        };
        if existing.iter().any(|candidate| candidate == name) {
            continue;
        }
        connection
            .execute(&format!("ALTER TABLE {table} ADD COLUMN {column}"), [])
            .map_err(|error| {
                query_error(format!(
                    "failed to add storefront column `{table}.{name}`: {error}"
                ))
            })?;
    }
    Ok(())
}

fn query_error(reason: String) -> StorefrontStateError {
    StorefrontStateError::Query { reason }
}

fn database_path(root: &Path, namespace: &str) -> PathBuf {
    root.join("storefront")
        .join(format!("{}.sqlite3", sanitize_namespace(namespace)))
}

fn sanitize_namespace(namespace: &str) -> String {
    namespace
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn catalog_item(sku: &str) -> Result<CatalogItem, StorefrontStateError> {
    match sku {
        "harbor-cap" => Ok(CatalogItem {
            sku: "harbor-cap",
            title: "Harbor Cap",
            variant_title: "Standard",
            product_kind: "physical",
            entitlement_key: None,
            unit_price_minor: 2_900,
        }),
        "membership-gold" | "gold-membership" => Ok(CatalogItem {
            sku: "membership-gold",
            title: "Gold Membership",
            variant_title: "Annual",
            product_kind: "membership",
            entitlement_key: Some("membership.gold"),
            unit_price_minor: 8_900,
        }),
        "harbor-tote" => Ok(CatalogItem {
            sku: "harbor-tote",
            title: "Harbor Tote",
            variant_title: "Standard",
            product_kind: "physical",
            entitlement_key: None,
            unit_price_minor: 4_500,
        }),
        "tasting-pass" => Ok(CatalogItem {
            sku: "tasting-pass",
            title: "Spring Tasting Pass",
            variant_title: "Single pass",
            product_kind: "physical",
            entitlement_key: None,
            unit_price_minor: 4_500,
        }),
        _ => Err(StorefrontStateError::UnknownSku {
            sku: sku.to_string(),
        }),
    }
}

fn saturating_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn format_minor_currency(value: i64) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let absolute = value.saturating_abs();
    let major = absolute / 100;
    let minor = absolute % 100;
    format!("{sign}£{major}.{minor:02}")
}
