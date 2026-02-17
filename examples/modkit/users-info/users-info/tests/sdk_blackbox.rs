#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::sync::Arc;

use authz_resolver_sdk::{
    AuthZResolverClient, AuthZResolverError,
    constraints::{Constraint, InPredicate, Predicate},
    models::{EvaluationRequest, EvaluationResponse, EvaluationResponseContext},
};
use modkit::config::ConfigProvider;
use modkit::{ClientHub, DatabaseCapability, Module, ModuleCtx};
use modkit_db::migration_runner::run_migrations_for_module;
use modkit_db::{ConnectOpts, DBProvider, Db, DbError, connect_db};
use modkit_security::{SecurityContext, pep_properties};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use users_info::UsersInfo;
use users_info_sdk::{NewUser, UsersInfoClientV1};

/// Mock `AuthZ` resolver for tests (`allow_all` mode).
///
/// Tenant resolution: `context.tenant_context.root_id` if present, otherwise
/// `subject.properties.tenant_id` (like a real PDP).
struct MockAuthZResolver;

#[async_trait::async_trait]
impl AuthZResolverClient for MockAuthZResolver {
    async fn evaluate(
        &self,
        request: EvaluationRequest,
    ) -> Result<EvaluationResponse, AuthZResolverError> {
        // Resolve tenant: explicit context > subject property (like a real PDP)
        let root_id = request
            .context
            .tenant_context
            .as_ref()
            .and_then(|tc| tc.root_id)
            .or_else(|| {
                request
                    .subject
                    .properties
                    .get("tenant_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
            });

        let constraints = if request.context.require_constraints {
            match root_id {
                Some(id) => vec![Constraint {
                    predicates: vec![Predicate::In(InPredicate::new(
                        pep_properties::OWNER_TENANT_ID,
                        [id],
                    ))],
                }],
                None => vec![],
            }
        } else {
            vec![]
        };

        Ok(EvaluationResponse {
            decision: true,
            context: EvaluationResponseContext {
                constraints,
                ..Default::default()
            },
        })
    }
}

struct MockConfigProvider {
    modules: HashMap<String, serde_json::Value>,
}

impl MockConfigProvider {
    fn new_users_info_default() -> Self {
        let mut modules = HashMap::new();
        // ModuleCtx::raw_config expects: modules.<name> = { database: ..., config: ... }
        // For this test we supply config only; DB handle is injected directly.
        modules.insert(
            "users_info".to_owned(),
            json!({
                "config": {
                    "default_page_size": 50,
                    "max_page_size": 1000,
                    "audit_base_url": "http://audit.local",
                    "notifications_base_url": "http://notifications.local",
                }
            }),
        );
        Self { modules }
    }
}

impl ConfigProvider for MockConfigProvider {
    fn get_module_config(&self, module_name: &str) -> Option<&serde_json::Value> {
        self.modules.get(module_name)
    }
}

#[tokio::test]
async fn users_info_registers_sdk_client_and_handles_basic_crud() {
    // Arrange: build a real Db for sqlite in-memory, run module migrations, then init module.
    let db: Db = connect_db(
        "sqlite::memory:",
        ConnectOpts {
            max_conns: Some(1),
            ..Default::default()
        },
    )
    .await
    .expect("db connect");
    let dbp: DBProvider<DbError> = DBProvider::new(db.clone());

    let hub = Arc::new(ClientHub::new());

    // Register mock AuthZ resolver before initializing the module
    hub.register::<dyn AuthZResolverClient>(Arc::new(MockAuthZResolver));

    let ctx = ModuleCtx::new(
        "users_info",
        Uuid::new_v4(),
        Arc::new(MockConfigProvider::new_users_info_default()),
        hub.clone(),
        CancellationToken::new(),
        Some(dbp),
    );

    let module = UsersInfo::default();
    run_migrations_for_module(&db, "users_info", module.migrations())
        .await
        .expect("migrate");
    module.init(&ctx).await.expect("init");

    // Act: resolve SDK client from hub and do basic CRUD.
    let client = ctx
        .client_hub()
        .get::<dyn UsersInfoClientV1>()
        .expect("UsersInfoClientV1 must be registered");

    // Create a security context with tenant access
    let tenant_id = Uuid::new_v4();
    let sec = SecurityContext::builder()
        .subject_id(Uuid::new_v4())
        .subject_tenant_id(tenant_id)
        .build()
        .unwrap();

    let created = client
        .create_user(
            sec.clone(),
            NewUser {
                id: None,
                tenant_id,
                email: "test@example.com".to_owned(),
                display_name: "Test".to_owned(),
            },
        )
        .await
        .unwrap();

    let fetched = client.get_user(sec.clone(), created.id).await.unwrap();
    assert_eq!(fetched.email, "test@example.com");

    client.delete_user(sec, created.id).await.unwrap();
}
