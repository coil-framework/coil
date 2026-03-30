use super::*;
use crate::sqlx_postgres::bind_query;
use coil_config::{DatabaseConfig, DatabaseDriver, SecretRef};
use std::env;
use std::time::Duration;

#[test]
fn data_runtime_maps_database_config_into_pool_profile() {
    let runtime = DataRuntime::from_config(&DatabaseConfig {
        driver: DatabaseDriver::Postgres,
        url: Some(SecretRef::Env {
            var: "DATABASE_URL".to_string(),
        }),
        schema: "coil".to_string(),
        migrations_table: "_migrations".to_string(),
        min_connections: 2,
        max_connections: 16,
        statement_timeout_secs: 15,
    })
    .unwrap();

    assert_eq!(runtime.driver, DatabaseDriver::Postgres);
    assert_eq!(
        runtime.connection_secret_ref,
        Some(SecretRef::Env {
            var: "DATABASE_URL".to_string()
        })
    );
    assert_eq!(
        runtime.connection_secret.as_deref(),
        Some("env:DATABASE_URL")
    );
    assert_eq!(runtime.schema, "coil");
    assert_eq!(runtime.pool.max_connections, 16);
    assert_eq!(runtime.pool.statement_timeout, Duration::from_secs(15));
}

#[test]
fn runtime_can_create_a_lazy_postgres_client() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let var = "COIL_TEST_DATABASE_URL";
        let previous = env::var(var).ok();
        unsafe {
            env::set_var(var, "postgres://coil:coil@localhost/coil");
        }

        let runtime = DataRuntime::from_config(&DatabaseConfig {
            url: Some(SecretRef::Env {
                var: var.to_string(),
            }),
            ..DatabaseConfig::default()
        })
        .unwrap();
        let client = runtime.connect_lazy_postgres().unwrap();

        assert_eq!(client.runtime.driver, DatabaseDriver::Postgres);
        assert_eq!(client.connection_url, "postgres://coil:coil@localhost/coil");

        match previous {
            Some(value) => unsafe {
                env::set_var(var, value);
            },
            None => unsafe {
                env::remove_var(var);
            },
        }
    });
}

#[test]
fn connect_lazy_postgres_requires_a_resolvable_connection_secret() {
    let runtime = DataRuntime::from_config(&DatabaseConfig {
        url: None,
        ..DatabaseConfig::default()
    })
    .unwrap();

    assert_eq!(
        runtime.connect_lazy_postgres().unwrap_err(),
        DataModelError::MissingConnectionSecret
    );
}

#[test]
fn bind_query_rejects_unsigned_values_that_exceed_bigint() {
    match bind_query(sqlx::query("SELECT $1"), &[DataValue::UInt(u64::MAX)]) {
        Err(error) => assert_eq!(
            error,
            DataModelError::UnsupportedUnsignedBindValue { value: u64::MAX }
        ),
        Ok(_) => panic!("expected oversized unsigned bind to be rejected"),
    }
}

#[test]
fn migration_plan_orders_steps_by_owner_and_order() {
    let mut plan = MigrationPlan::new();
    plan.insert(
        MigrationStep::new(
            MigrationId::new("001_core").unwrap(),
            MigrationOwner::Core,
            1,
            "core tables",
        )
        .unwrap(),
    )
    .unwrap();
    plan.insert(
        MigrationStep::new(
            MigrationId::new("010_events").unwrap(),
            MigrationOwner::Module("events".to_string()),
            10,
            "events tables",
        )
        .unwrap(),
    )
    .unwrap();
    plan.insert(
        MigrationStep::new(
            MigrationId::new("900_customer").unwrap(),
            MigrationOwner::CustomerApp("showcase".to_string()),
            900,
            "customer app projection",
        )
        .unwrap(),
    )
    .unwrap();

    let owners = plan
        .ordered_steps()
        .iter()
        .map(|step| step.owner.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        owners,
        vec![
            "core".to_string(),
            "module:events".to_string(),
            "customer_app:showcase".to_string(),
        ]
    );
}

#[test]
fn duplicate_migrations_are_rejected_per_owner() {
    let mut plan = MigrationPlan::new();
    let step = MigrationStep::new(
        MigrationId::new("001_core").unwrap(),
        MigrationOwner::Core,
        1,
        "core tables",
    )
    .unwrap();
    plan.insert(step.clone()).unwrap();

    let error = plan.insert(step).unwrap_err();
    assert_eq!(
        error,
        DataModelError::DuplicateMigration {
            owner: "core".to_string(),
            migration_id: "001_core".to_string(),
        }
    );
}

#[test]
fn query_specs_capture_filters_sorts_and_context() {
    let query = QuerySpec::new(
        PageRequest::new(1, 25).unwrap(),
        QueryContext {
            locale: Some("fr-FR".to_string()),
            principal_id: Some("user-42".to_string()),
            publication_visibility: PublicationVisibility::PublishedOnly,
            cache_scope: QueryCacheScope::UserScoped,
        },
    )
    .with_filter(
        QueryFilter::new(
            "event_slug",
            FilterOperator::Eq,
            vec!["spring-tasting".to_string()],
        )
        .unwrap(),
    )
    .with_sort(QuerySort::ascending("starts_at").unwrap());

    assert_eq!(query.page.offset(), 25);
    assert_eq!(query.filters.len(), 1);
    assert_eq!(query.sort[0].field.as_str(), "starts_at");
    assert_eq!(query.context.locale.as_deref(), Some("fr-FR"));
}

#[test]
fn transaction_plans_keep_writes_separate_from_after_commit_work() {
    let plan = TransactionPlan::new("booking.create", TransactionIsolation::Serializable)
        .unwrap()
        .with_write(DomainWrite::new("booking", "insert").unwrap())
        .with_write(DomainWrite::new("capacity", "decrement").unwrap())
        .with_after_commit_job("send-booking-email")
        .unwrap()
        .with_after_commit_event("booking.created")
        .unwrap();

    assert_eq!(plan.writes.len(), 2);
    assert_eq!(
        plan.after_commit_jobs,
        vec!["send-booking-email".to_string()]
    );
    assert_eq!(
        plan.after_commit_events,
        vec!["booking.created".to_string()]
    );
}

#[test]
fn repository_specs_compile_locale_and_publication_aware_sql() {
    let repository = RepositorySpec::new(
        "cms.pages",
        TableName::new("coil.cms_pages").unwrap(),
        vec![
            QueryField::new("page_id").unwrap(),
            QueryField::new("title").unwrap(),
            QueryField::new("live_path").unwrap(),
            QueryField::new("updated_at").unwrap(),
        ],
    )
    .unwrap()
    .with_locale_field("locale")
    .unwrap()
    .with_publication_field("workflow_status", "published")
    .unwrap()
    .with_filterable_field("slug")
    .unwrap()
    .with_default_sort(QuerySort::ascending("live_path").unwrap());

    let query = QuerySpec::new(
        PageRequest::new(0, 20).unwrap(),
        QueryContext {
            locale: Some("fr-FR".to_string()),
            principal_id: None,
            publication_visibility: PublicationVisibility::PublishedOnly,
            cache_scope: QueryCacheScope::LocaleScoped,
        },
    )
    .with_filter(QueryFilter::new("slug", FilterOperator::Eq, vec!["home".to_string()]).unwrap());

    let compiled = repository.compile_query(&query).unwrap();
    assert_eq!(
        compiled.sql,
        "SELECT \"page_id\", \"title\", \"live_path\", \"updated_at\" FROM \"coil\".\"cms_pages\" WHERE \"locale\" = $1 AND \"workflow_status\" = $2 AND \"slug\" = $3 ORDER BY \"live_path\" ASC LIMIT 20 OFFSET 0"
    );
    assert_eq!(
        compiled.bind_values,
        vec![
            DataValue::String("fr-FR".to_string()),
            DataValue::String("published".to_string()),
            DataValue::String("home".to_string()),
        ]
    );
}

#[test]
fn runtime_compiles_mutations_and_outbox_delivery_sql() {
    let runtime = DataRuntime::from_config(&DatabaseConfig::default()).unwrap();
    let plan = TransactionPlan::new("events.booking.confirm", TransactionIsolation::Serializable)
        .unwrap()
        .with_write(DomainWrite::new("events.bookings", "update").unwrap())
        .with_write(DomainWrite::new("events.reservations", "delete").unwrap())
        .with_after_commit_job("events.jobs.notifications.booking_confirmed")
        .unwrap()
        .with_after_commit_event("events.booking.confirmed")
        .unwrap();
    let mutations = vec![
        MutationSpec::new("coil.events_bookings", MutationAction::Update)
            .unwrap()
            .with_assignment("status", "confirmed")
            .unwrap()
            .with_predicate(
                QueryFilter::new(
                    "booking_id",
                    FilterOperator::Eq,
                    vec!["booking-1".to_string()],
                )
                .unwrap(),
            ),
        MutationSpec::new("coil.events_reservations", MutationAction::Delete)
            .unwrap()
            .with_predicate(
                QueryFilter::new(
                    "reservation_id",
                    FilterOperator::Eq,
                    vec!["reservation-1".to_string()],
                )
                .unwrap(),
            ),
    ];

    let compiled = runtime.compile_transaction(&plan, &mutations).unwrap();
    assert_eq!(compiled.begin_sql, "BEGIN");
    assert_eq!(compiled.commit_sql, "COMMIT");
    assert_eq!(
        compiled.statements[0].sql,
        "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE"
    );
    assert_eq!(
        compiled.statements[1].sql,
        "UPDATE \"coil\".\"events_bookings\" SET \"status\" = $1 WHERE \"booking_id\" = $2"
    );
    assert_eq!(
        compiled.statements[2].sql,
        "DELETE FROM \"coil\".\"events_reservations\" WHERE \"reservation_id\" = $3"
    );
    assert!(
        compiled
            .statements
            .iter()
            .any(|statement| statement.sql.contains("\"public\".\"job_outbox\""))
    );
    assert!(
        compiled
            .statements
            .iter()
            .any(|statement| statement.sql.contains("\"public\".\"event_outbox\""))
    );
}

#[test]
fn migration_registry_compiles_apply_batch_with_ledger_entries() {
    let runtime = DataRuntime::from_config(&DatabaseConfig::default()).unwrap();
    let mut plan = MigrationPlan::new();
    plan.insert(
        MigrationStep::new(
            MigrationId::new("001_pages").unwrap(),
            MigrationOwner::Module("cms".to_string()),
            10,
            "create cms pages table",
        )
        .unwrap()
        .with_statement("CREATE TABLE coil.cms_pages (page_id TEXT PRIMARY KEY)")
        .unwrap(),
    )
    .unwrap();
    let mut registry = MigrationRegistry::new();
    registry.register(&plan).unwrap();

    let batch = registry.compile_apply_batch(&runtime).unwrap();
    assert!(
        batch.statements[0]
            .sql
            .contains("\"public\".\"_coil_migrations\"")
    );
    assert_eq!(
        batch.statements[1].sql,
        "CREATE TABLE coil.cms_pages (page_id TEXT PRIMARY KEY)"
    );
    assert!(batch.statements[2].sql.contains("ON CONFLICT"));
    assert_eq!(
        batch.statements[2].bind_values,
        vec![
            DataValue::String("module:cms".to_string()),
            DataValue::String("001_pages".to_string()),
            DataValue::String("create cms pages table".to_string()),
        ]
    );
}
