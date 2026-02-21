use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use dashmap::DashMap;
use http::Method;
use std::sync::Arc;

use modkit::api::{OperationSpec, Problem};

const BASE_FEATURE: &str = "gts.x.core.lic.feat.v1~x.core.global.base.v1";

type LicenseKey = (Method, String);

#[derive(Clone)]
pub struct LicenseRequirementMap {
    requirements: Arc<DashMap<LicenseKey, Vec<String>>>,
}

impl LicenseRequirementMap {
    #[must_use]
    pub fn from_specs(specs: &[OperationSpec])  -> Self {
        let requirements = DashMap::new();

        for spec in specs {
            if let Some(req) = spec.license_requirement.as_ref() {
                requirements.insert(
                    (spec.method.clone(), spec.path.clone()),
                    req.license_names.clone(),
                );
            }
        }

        Self {
            requirements: Arc::new(requirements),
        }
    }

    fn get(&self, method: &Method, path: &str) -> Option<Vec<String>> {
        self.requirements
            .get(&(method.clone(), path.to_owned()))
            .map(|v| v.value().clone())
    }
}

pub async fn license_validation_middleware(
    map: LicenseRequirementMap,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let path = req
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map_or_else(|| req.uri().path().to_owned(), |p| p.as_str().to_owned());

    let nestedpath = req.extensions().get::<axum::extract::NestedPath>();

    let path = nestedpath
        .and_then(|np| path.strip_prefix(np.as_str()))
        .unwrap_or(&path)
        .to_owned();


    let Some(required) = map.get(&method, &path) else {
        return next.run(req).await;
    };

    // TODO: this is a stub implementation
    // We need first to implement plugin and get its client from client_hub
    // Plugin should provide an interface to get a list of global features (features that are not scoped to particular resource)
    if required.iter().any(|r| r != BASE_FEATURE) {
        return Problem::new(
            StatusCode::FORBIDDEN,
            "Forbidden",
            format!(
                "Endpoint requires unsupported license features '{required:?}'; only '{BASE_FEATURE}' is allowed",
            ),
        )
        .into_response();
    }

    next.run(req).await
}
