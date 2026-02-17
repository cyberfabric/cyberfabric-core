#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use async_trait::async_trait;
use authz_resolver_sdk::{
    AuthZResolverClient, AuthZResolverError,
    constraints::{Constraint, EqPredicate, InPredicate, Predicate},
    models::{EvaluationRequest, EvaluationResponse, EvaluationResponseContext},
};
use modkit_db::migration_runner::run_migrations_for_testing;
use modkit_db::secure::DBRunner;
use modkit_db::secure::{AccessScope, secure_insert};
use modkit_db::{ConnectOpts, DBProvider, Db, DbError, connect_db};
use modkit_security::{SecurityContext, pep_properties};
use sea_orm_migration::MigratorTrait;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::domain::events::UserDomainEvent;
use crate::domain::ports::{AuditPort, EventPublisher};
use crate::domain::service::ServiceConfig;
use crate::infra::storage::{OrmAddressesRepository, OrmCitiesRepository, OrmUsersRepository};
use crate::module::ConcreteAppServices;

#[must_use]
pub fn ctx_allow_tenants(tenants: &[Uuid]) -> SecurityContext {
    let tenant_id = tenants.first().copied().unwrap_or_else(Uuid::new_v4);
    SecurityContext::builder()
        .subject_id(Uuid::new_v4())
        .subject_tenant_id(tenant_id)
        .build()
        .unwrap()
}

/// Create a security context with a specific `subject_id` and tenant.
/// Useful for owner-based authorization tests where `subject_id` must match `user_id`.
#[must_use]
pub fn ctx_for_subject(subject_id: Uuid, tenant_id: Uuid) -> SecurityContext {
    SecurityContext::builder()
        .subject_id(subject_id)
        .subject_tenant_id(tenant_id)
        .build()
        .unwrap()
}

#[must_use]
pub fn ctx_deny_all() -> SecurityContext {
    SecurityContext::anonymous()
}

/// Create an in-memory database for testing.
pub async fn inmem_db() -> Db {
    let opts = ConnectOpts {
        max_conns: Some(1),
        min_conns: Some(1),
        ..Default::default()
    };
    let db = connect_db("sqlite::memory:", opts)
        .await
        .expect("Failed to connect to in-memory database");

    run_migrations_for_testing(
        &db,
        crate::infra::storage::migrations::Migrator::migrations(),
    )
    .await
    .map_err(|e| e.to_string())
    .expect("Failed to run migrations");

    db
}

pub async fn seed_user(
    db: &impl DBRunner,
    id: Uuid,
    tenant_id: Uuid,
    email: &str,
    display_name: &str,
) {
    use crate::infra::storage::entity::user::ActiveModel;
    use crate::infra::storage::entity::user::Entity as UserEntity;
    use sea_orm::Set;

    let now = OffsetDateTime::now_utc();
    let user = ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        email: Set(email.to_owned()),
        display_name: Set(display_name.to_owned()),
        created_at: Set(now),
        updated_at: Set(now),
    };

    let scope = AccessScope::for_tenants(vec![tenant_id]);
    let _ = secure_insert::<UserEntity>(user, &scope, db)
        .await
        .expect("Failed to seed user");
}

pub struct MockEventPublisher;
pub struct MockAuditPort;

impl EventPublisher<UserDomainEvent> for MockEventPublisher {
    fn publish(&self, _event: &UserDomainEvent) {}
}

#[async_trait::async_trait]
impl AuditPort for MockAuditPort {
    async fn get_user_access(&self, _id: Uuid) -> Result<(), crate::domain::error::DomainError> {
        Ok(())
    }

    async fn notify_user_created(&self) -> Result<(), crate::domain::error::DomainError> {
        Ok(())
    }
}

/// Mock `AuthZ` resolver that allows all requests and returns the context's tenant
/// as a constraint, mimicking the `static_authz_plugin` `allow_all` behavior.
///
/// Tenant resolution: `context.tenant_context.root_id` if present, otherwise
/// `subject.properties.tenant_id` (like a real PDP).
///
/// Decision logic:
/// - If no resolved tenant → `decision=true`, empty constraints (anonymous/root).
/// - If resolved tenant and `require_constraints=true` → `decision=true`
///   with tenant constraint (standard path for LIST/UPDATE/DELETE).
/// - If resolved tenant and `require_constraints=false` → `decision=true`,
///   no constraints (GET prefetch / CREATE path — PDP checked tenant ownership
///   via resource properties; no row-level filter needed).
pub struct MockAuthZResolver;

#[async_trait]
impl AuthZResolverClient for MockAuthZResolver {
    async fn evaluate(
        &self,
        request: EvaluationRequest,
    ) -> Result<EvaluationResponse, AuthZResolverError> {
        // Resolve tenant: explicit context > subject property (like a real PDP)
        // Filter out nil UUIDs — they represent anonymous/unset context.
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
            })
            .filter(|id| !id.is_nil());

        if request.context.require_constraints {
            // Standard path: return tenant constraint for row-level filtering.
            let constraints = match root_id {
                Some(id) => vec![Constraint {
                    predicates: vec![Predicate::In(InPredicate::new(
                        pep_properties::OWNER_TENANT_ID,
                        [id],
                    ))],
                }],
                None => vec![],
            };
            Ok(EvaluationResponse {
                decision: true,
                context: EvaluationResponseContext {
                    constraints,
                    ..Default::default()
                },
            })
        } else {
            // Prefetch / CREATE path: PDP evaluates resource properties
            // against the subject's tenant. Deny if they don't match.
            let decision = match (
                root_id,
                request
                    .resource
                    .properties
                    .get(pep_properties::OWNER_TENANT_ID)
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok()),
            ) {
                (Some(subject_tenant), Some(resource_tenant)) => subject_tenant == resource_tenant,
                _ => true, // no tenant info → allow (anonymous/root)
            };
            Ok(EvaluationResponse {
                decision,
                context: EvaluationResponseContext::default(),
            })
        }
    }
}

pub fn build_services(db: Db, config: ServiceConfig) -> Arc<ConcreteAppServices> {
    build_services_with_authz(db, config, Arc::new(MockAuthZResolver))
}

pub fn build_services_with_authz(
    db: Db,
    config: ServiceConfig,
    authz: Arc<dyn AuthZResolverClient>,
) -> Arc<ConcreteAppServices> {
    let limit_cfg = config.limit_cfg();

    let users_repo = OrmUsersRepository::new(limit_cfg);
    let cities_repo = OrmCitiesRepository::new(limit_cfg);
    let addresses_repo = OrmAddressesRepository::new(limit_cfg);

    let db: Arc<DBProvider<DbError>> = Arc::new(DBProvider::new(db));

    Arc::new(ConcreteAppServices::new(
        users_repo,
        cities_repo,
        addresses_repo,
        db,
        Arc::new(MockEventPublisher),
        Arc::new(MockAuditPort),
        authz,
        config,
    ))
}

/// Mock `AuthZ` resolver that returns `decision=false` in the response.
///
/// This is the canonical PDP denial path: the PDP evaluates the request and
/// explicitly sets `decision=false`. The enforcer converts this into
/// `EnforcerError::Denied`.
pub struct DenyAllAuthZResolver;

#[async_trait]
impl AuthZResolverClient for DenyAllAuthZResolver {
    async fn evaluate(
        &self,
        _request: EvaluationRequest,
    ) -> Result<EvaluationResponse, AuthZResolverError> {
        Ok(EvaluationResponse {
            decision: false,
            context: EvaluationResponseContext::default(),
        })
    }
}

/// Mock `AuthZ` resolver that always fails with an internal error.
///
/// Returns `AuthZResolverError::Internal` — simulates a PDP that is
/// unreachable or encounters an unexpected failure.
pub struct FailingAuthZResolver;

#[async_trait]
impl AuthZResolverClient for FailingAuthZResolver {
    async fn evaluate(
        &self,
        _request: EvaluationRequest,
    ) -> Result<EvaluationResponse, AuthZResolverError> {
        Err(AuthZResolverError::Internal("PDP unavailable".to_owned()))
    }
}

/// Policy-aware mock `AuthZ` resolver that enforces owner and city constraints.
///
/// Simulates a PDP that:
/// - Always requires tenant isolation (`owner_tenant_id`)
/// - For mutations on `users_info.address`: enforces `owner_id` must equal
///   `subject.id` and echoes back `city_id` from resource properties as an
///   `eq` constraint (so PEP can enforce city restrictions at SQL level).
///
/// Tenant resolution: `context.tenant_context.root_id` if present, otherwise
/// `subject.properties.tenant_id` (like a real PDP).
///
/// This demonstrates the full PDP → PEP → SQL constraint flow.
pub struct OwnerCityAuthZResolver;

#[async_trait]
impl AuthZResolverClient for OwnerCityAuthZResolver {
    async fn evaluate(
        &self,
        request: EvaluationRequest,
    ) -> Result<EvaluationResponse, AuthZResolverError> {
        if !request.context.require_constraints {
            return Ok(EvaluationResponse {
                decision: true,
                context: EvaluationResponseContext::default(),
            });
        }

        // Resolve tenant: explicit context > subject property (like a real PDP)
        // Filter out nil UUIDs — they represent anonymous/unset context.
        let tenant_root = request
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
            })
            .filter(|id| !id.is_nil());

        let Some(root_id) = tenant_root else {
            // No tenant info at all → allow without constraints (anonymous/root)
            return Ok(EvaluationResponse {
                decision: true,
                context: EvaluationResponseContext::default(),
            });
        };

        let mut predicates = vec![Predicate::In(InPredicate::new(
            pep_properties::OWNER_TENANT_ID,
            [root_id],
        ))];

        let is_address = request.resource.resource_type == "users_info.address";
        let is_mutation = matches!(request.action.name.as_str(), "create" | "update" | "delete");

        if is_address && is_mutation {
            // Enforce owner_id == subject.id
            predicates.push(Predicate::Eq(EqPredicate::new(
                pep_properties::OWNER_ID,
                request.subject.id,
            )));

            // If city_id is provided in resource properties, echo it back
            // as an eq constraint (simulates "user allowed only in this city")
            if let Some(city_val) = request.resource.properties.get("city_id")
                && let Some(city_str) = city_val.as_str()
                && let Ok(city_uuid) = Uuid::parse_str(city_str)
            {
                predicates.push(Predicate::Eq(EqPredicate::new("city_id", city_uuid)));
            }
        }

        Ok(EvaluationResponse {
            decision: true,
            context: EvaluationResponseContext {
                constraints: vec![Constraint { predicates }],
                ..Default::default()
            },
        })
    }
}
