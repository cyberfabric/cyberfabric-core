// This file is used to ensure that all modules are linked and registered via inventory
// In future we can simply DX via build.rs which will collect all crates in ./modules and generate this file.
// But for now we will manually maintain this file.
#![allow(unused_imports)]

use api_egress as _;
use api_gateway as _;
use authn_resolver as _;
use authz_resolver as _;
#[cfg(not(feature = "oop-example"))]
use file_parser as _;
use grpc_hub as _;
use module_orchestrator as _;
use nodes_registry as _;
#[cfg(not(feature = "oop-example"))]
use simple_user_settings as _;
use tenant_resolver as _;
use types_registry as _;

#[cfg(feature = "single-tenant")]
use single_tenant_tr_plugin as _;

#[cfg(feature = "static-tenants")]
use static_tr_plugin as _;

#[cfg(feature = "static-authn")]
use static_authn_plugin as _;

#[cfg(feature = "static-authz")]
use static_authz_plugin as _;

// === Example Features ===

#[cfg(feature = "users-info-example")]
use users_info as _;

#[cfg(feature = "oop-example")]
use calculator_gateway as _;

#[cfg(feature = "oop-example")]
use calculator as _;
