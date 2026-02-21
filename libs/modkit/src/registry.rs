// modkit/src/registry/mod.rs
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use thiserror::Error;

// Re-exported contracts are referenced but not defined here.
use crate::contracts;

/// Type alias for REST host module configuration.
type RestHostEntry = (&'static str, Arc<dyn contracts::ApiGatewayCapability>);

// ============================================================================
// Capability System
// ============================================================================

/// A single capability variant that a module can provide.
#[derive(Clone)]
pub enum Capability {
    #[cfg(feature = "db")]
    Database(Arc<dyn contracts::DatabaseCapability>),
    RestApi(Arc<dyn contracts::RestApiCapability>),
    ApiGateway(Arc<dyn contracts::ApiGatewayCapability>),
    Runnable(Arc<dyn contracts::RunnableCapability>),
    System(Arc<dyn contracts::SystemCapability>),
    GrpcHub(Arc<dyn contracts::GrpcHubCapability>),
    GrpcService(Arc<dyn contracts::GrpcServiceCapability>),
}

impl std::fmt::Debug for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "db")]
            Capability::Database(_) => write!(f, "Database(<impl DatabaseCapability>)"),
            Capability::RestApi(_) => write!(f, "RestApi(<impl RestApiCapability>)"),
            Capability::ApiGateway(_) => write!(f, "ApiGateway(<impl ApiGatewayCapability>)"),
            Capability::Runnable(_) => write!(f, "Runnable(<impl RunnableCapability>)"),
            Capability::System(_) => write!(f, "System(<impl SystemCapability>)"),
            Capability::GrpcHub(_) => write!(f, "GrpcHub(<impl GrpcHubCapability>)"),
            Capability::GrpcService(_) => write!(f, "GrpcService(<impl GrpcServiceCapability>)"),
        }
    }
}

/// Trait for capability tags that allow type-safe querying.
pub trait CapTag {
    type Out: ?Sized + 'static;
    fn try_get(cap: &Capability) -> Option<&Arc<Self::Out>>;
}

/// Tag for querying `DatabaseCapability`.
#[cfg(feature = "db")]
pub struct DatabaseCap;
#[cfg(feature = "db")]
impl CapTag for DatabaseCap {
    type Out = dyn contracts::DatabaseCapability;
    fn try_get(cap: &Capability) -> Option<&Arc<Self::Out>> {
        match cap {
            Capability::Database(v) => Some(v),
            _ => None,
        }
    }
}

/// Tag for querying `RestApiCapability`.
pub struct RestApiCap;
impl CapTag for RestApiCap {
    type Out = dyn contracts::RestApiCapability;
    fn try_get(cap: &Capability) -> Option<&Arc<Self::Out>> {
        match cap {
            Capability::RestApi(v) => Some(v),
            _ => None,
        }
    }
}

/// Tag for querying `ApiGatewayCapability`.
pub struct ApiGatewayCap;
impl CapTag for ApiGatewayCap {
    type Out = dyn contracts::ApiGatewayCapability;
    fn try_get(cap: &Capability) -> Option<&Arc<Self::Out>> {
        match cap {
            Capability::ApiGateway(v) => Some(v),
            _ => None,
        }
    }
}

/// Tag for querying `RunnableCapability`.
pub struct RunnableCap;
impl CapTag for RunnableCap {
    type Out = dyn contracts::RunnableCapability;
    fn try_get(cap: &Capability) -> Option<&Arc<Self::Out>> {
        match cap {
            Capability::Runnable(v) => Some(v),
            _ => None,
        }
    }
}

/// Tag for querying `SystemCapability`.
pub struct SystemCap;
impl CapTag for SystemCap {
    type Out = dyn contracts::SystemCapability;
    fn try_get(cap: &Capability) -> Option<&Arc<Self::Out>> {
        match cap {
            Capability::System(v) => Some(v),
            _ => None,
        }
    }
}

/// Tag for querying `GrpcHubCapability`.
pub struct GrpcHubCap;
impl CapTag for GrpcHubCap {
    type Out = dyn contracts::GrpcHubCapability;
    fn try_get(cap: &Capability) -> Option<&Arc<Self::Out>> {
        match cap {
            Capability::GrpcHub(v) => Some(v),
            _ => None,
        }
    }
}

/// Tag for querying `GrpcServiceCapability`.
pub struct GrpcServiceCap;
impl CapTag for GrpcServiceCap {
    type Out = dyn contracts::GrpcServiceCapability;
    fn try_get(cap: &Capability) -> Option<&Arc<Self::Out>> {
        match cap {
            Capability::GrpcService(v) => Some(v),
            _ => None,
        }
    }
}

/// A set of capabilities that a module provides.
#[derive(Clone)]
pub struct CapabilitySet {
    caps: Vec<Capability>,
}

impl std::fmt::Debug for CapabilitySet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapabilitySet")
            .field("caps", &self.caps)
            .finish()
    }
}

impl CapabilitySet {
    /// Create an empty capability set.
    #[must_use]
    pub fn new() -> Self {
        Self { caps: Vec::new() }
    }

    /// Add a capability to the set.
    pub fn push(&mut self, cap: Capability) {
        self.caps.push(cap);
    }

    /// Check if the set contains a specific capability type.
    #[must_use]
    pub fn has<T: CapTag>(&self) -> bool {
        self.caps.iter().any(|cap| T::try_get(cap).is_some())
    }

    /// Query for a specific capability type.
    #[must_use]
    pub fn query<T: CapTag>(&self) -> Option<Arc<T::Out>> {
        self.caps.iter().find_map(|cap| T::try_get(cap).cloned())
    }

    /// Returns human-readable capability labels (e.g. `"rest"`, `"db"`, `"system"`).
    #[must_use]
    pub fn labels(&self) -> Vec<&'static str> {
        self.caps
            .iter()
            .map(|cap| match cap {
                #[cfg(feature = "db")]
                Capability::Database(_) => "db",
                Capability::RestApi(_) => "rest",
                Capability::ApiGateway(_) => "rest_host",
                Capability::Runnable(_) => "stateful",
                Capability::System(_) => "system",
                Capability::GrpcHub(_) => "grpc_hub",
                Capability::GrpcService(_) => "grpc",
            })
            .collect()
    }

    /// Convenience helper for DB presence.
    #[must_use]
    pub fn has_db(&self) -> bool {
        #[cfg(feature = "db")]
        {
            self.has::<DatabaseCap>()
        }
        #[cfg(not(feature = "db"))]
        {
            false
        }
    }
}

impl Default for CapabilitySet {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ModuleEntry {
    pub(crate) name: &'static str,
    pub(crate) deps: &'static [&'static str],
    pub(crate) core: Arc<dyn contracts::Module>,
    pub(crate) caps: CapabilitySet,
}

impl ModuleEntry {
    /// Returns the module name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the module dependency names.
    #[must_use]
    pub fn deps(&self) -> &'static [&'static str] {
        self.deps
    }

    /// Returns the capability set.
    #[must_use]
    pub fn caps(&self) -> &CapabilitySet {
        &self.caps
    }
}

impl std::fmt::Debug for ModuleEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleEntry")
            .field("name", &self.name)
            .field("deps", &self.deps)
            .field("has_rest", &self.caps.has::<RestApiCap>())
            .field("is_rest_host", &self.caps.has::<ApiGatewayCap>())
            .field("has_db", &self.caps.has_db())
            .field("has_stateful", &self.caps.has::<RunnableCap>())
            .field("is_system", &self.caps.has::<SystemCap>())
            .field("is_grpc_hub", &self.caps.has::<GrpcHubCap>())
            .field("has_grpc_service", &self.caps.has::<GrpcServiceCap>())
            .finish_non_exhaustive()
    }
}

/// The function type submitted by the macro via `inventory::submit!`.
/// NOTE: It now takes a *builder*, not the final registry.
pub struct Registrator(pub fn(&mut RegistryBuilder));

inventory::collect!(Registrator);

/// The final, topo-sorted runtime registry.
pub struct ModuleRegistry {
    modules: Vec<ModuleEntry>, // topo-sorted
    pub grpc_hub: Option<String>,
    pub grpc_services: Vec<(String, Arc<dyn contracts::GrpcServiceCapability>)>,
}

impl std::fmt::Debug for ModuleRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&'static str> = self.modules.iter().map(|m| m.name).collect();
        f.debug_struct("ModuleRegistry")
            .field("modules", &names)
            .field("has_grpc_hub", &self.grpc_hub.is_some())
            .field("grpc_services_count", &self.grpc_services.len())
            .finish()
    }
}

impl ModuleRegistry {
    #[must_use]
    pub fn modules(&self) -> &[ModuleEntry] {
        &self.modules
    }

    /// Returns modules ordered by system priority.
    /// System modules come first, followed by non-system modules.
    /// Within each group, the original topological order is preserved.
    #[must_use]
    pub fn modules_by_system_priority(&self) -> Vec<&ModuleEntry> {
        let mut system_mods = Vec::new();
        let mut non_system_mods = Vec::new();

        for entry in &self.modules {
            if entry.caps.has::<SystemCap>() {
                system_mods.push(entry);
            } else {
                non_system_mods.push(entry);
            }
        }

        system_mods.extend(non_system_mods);
        system_mods
    }

    /// Discover via inventory, have registrators fill the builder, then build & topo-sort.
    ///
    /// # Errors
    /// Returns `RegistryError` if module discovery or dependency resolution fails.
    pub fn discover_and_build() -> Result<Self, RegistryError> {
        let mut b = RegistryBuilder::default();
        for r in ::inventory::iter::<Registrator> {
            r.0(&mut b);
        }
        b.build_topo_sorted()
    }

    /// (Optional) quick lookup if you need it.
    #[must_use]
    pub fn get_module(&self, name: &str) -> Option<Arc<dyn contracts::Module>> {
        self.modules
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.core.clone())
    }
}

/// Type alias for gRPC hub module configuration.
type GrpcHubEntry = (&'static str, Arc<dyn contracts::GrpcHubCapability>);

/// Internal builder that macro registrators will feed.
/// Keys are module **names**; uniqueness enforced at build time.
#[derive(Default)]
pub struct RegistryBuilder {
    core: HashMap<&'static str, Arc<dyn contracts::Module>>,
    deps: HashMap<&'static str, &'static [&'static str]>,
    capabilities: HashMap<&'static str, Vec<Capability>>,
    rest_host: Option<RestHostEntry>,
    grpc_hub: Option<GrpcHubEntry>,
    errors: Vec<String>,
}

/// Type alias for dependency graph: (names, adjacency list, index map)
type DependencyGraph = (
    Vec<&'static str>,
    Vec<Vec<usize>>,
    HashMap<&'static str, usize>,
);

impl RegistryBuilder {
    pub fn register_core_with_meta(
        &mut self,
        name: &'static str,
        deps: &'static [&'static str],
        m: Arc<dyn contracts::Module>,
    ) {
        if self.core.contains_key(name) {
            self.errors
                .push(format!("Module '{name}' is already registered"));
            return;
        }
        self.core.insert(name, m);
        self.deps.insert(name, deps);
    }

    pub fn register_rest_with_meta(
        &mut self,
        name: &'static str,
        m: Arc<dyn contracts::RestApiCapability>,
    ) {
        self.capabilities
            .entry(name)
            .or_default()
            .push(Capability::RestApi(m));
    }

    pub fn register_rest_host_with_meta(
        &mut self,
        name: &'static str,
        m: Arc<dyn contracts::ApiGatewayCapability>,
    ) {
        if let Some((existing, _)) = &self.rest_host {
            self.errors.push(format!(
                "Multiple REST host modules detected: '{existing}' and '{name}'. Only one REST host is allowed."
            ));
            return;
        }
        self.rest_host = Some((name, m));
    }

    #[cfg(feature = "db")]
    pub fn register_db_with_meta(
        &mut self,
        name: &'static str,
        m: Arc<dyn contracts::DatabaseCapability>,
    ) {
        self.capabilities
            .entry(name)
            .or_default()
            .push(Capability::Database(m));
    }

    pub fn register_stateful_with_meta(
        &mut self,
        name: &'static str,
        m: Arc<dyn contracts::RunnableCapability>,
    ) {
        self.capabilities
            .entry(name)
            .or_default()
            .push(Capability::Runnable(m));
    }

    pub fn register_system_with_meta(
        &mut self,
        name: &'static str,
        m: Arc<dyn contracts::SystemCapability>,
    ) {
        self.capabilities
            .entry(name)
            .or_default()
            .push(Capability::System(m));
    }

    pub fn register_grpc_hub_with_meta(
        &mut self,
        name: &'static str,
        m: Arc<dyn contracts::GrpcHubCapability>,
    ) {
        if let Some((existing, _)) = &self.grpc_hub {
            self.errors.push(format!(
                "Multiple gRPC hub modules detected: '{existing}' and '{name}'. Only one gRPC hub is allowed."
            ));
            return;
        }
        self.grpc_hub = Some((name, m));
    }

    pub fn register_grpc_service_with_meta(
        &mut self,
        name: &'static str,
        m: Arc<dyn contracts::GrpcServiceCapability>,
    ) {
        self.capabilities
            .entry(name)
            .or_default()
            .push(Capability::GrpcService(m));
    }

    /// Detect cycles in the dependency graph using DFS with path tracking.
    /// Returns the cycle path if found, None otherwise.
    fn detect_cycle_with_path(
        names: &[&'static str],
        adj: &[Vec<usize>],
    ) -> Option<Vec<&'static str>> {
        #[derive(Clone, Copy, PartialEq)]
        enum Color {
            White, // unvisited
            Gray,  // visiting (on current path)
            Black, // visited (finished)
        }

        fn dfs(
            node: usize,
            names: &[&'static str],
            adj: &[Vec<usize>],
            colors: &mut [Color],
            path: &mut Vec<usize>,
        ) -> Option<Vec<&'static str>> {
            colors[node] = Color::Gray;
            path.push(node);

            for &neighbor in &adj[node] {
                match colors[neighbor] {
                    Color::Gray => {
                        // Found a back edge - cycle detected
                        // Find the cycle start in the current path
                        if let Some(cycle_start) = path.iter().position(|&n| n == neighbor) {
                            let cycle_indices = &path[cycle_start..];
                            let mut cycle_path: Vec<&'static str> =
                                cycle_indices.iter().map(|&i| names[i]).collect();
                            // Close the cycle by adding the first node again
                            cycle_path.push(names[neighbor]);
                            return Some(cycle_path);
                        }
                    }
                    Color::White => {
                        if let Some(cycle) = dfs(neighbor, names, adj, colors, path) {
                            return Some(cycle);
                        }
                    }
                    Color::Black => {
                        // Already processed, no cycle through this path
                    }
                }
            }

            path.pop();
            colors[node] = Color::Black;
            None
        }

        let mut colors = vec![Color::White; names.len()];
        let mut path = Vec::new();

        for i in 0..names.len() {
            if colors[i] == Color::White
                && let Some(cycle) = dfs(i, names, adj, &mut colors, &mut path)
            {
                return Some(cycle);
            }
        }

        None
    }

    /// Validate that all capabilities reference known core modules.
    fn validate_capabilities(&self) -> Result<(), RegistryError> {
        // Check rest_host early
        if let Some((host_name, _)) = &self.rest_host
            && !self.core.contains_key(host_name)
        {
            return Err(RegistryError::UnknownModule((*host_name).to_owned()));
        }

        // Check for configuration errors
        if !self.errors.is_empty() {
            return Err(RegistryError::InvalidRegistryConfiguration {
                errors: self.errors.clone(),
            });
        }

        // Validate all capability module names reference known core modules
        for name in self.capabilities.keys() {
            if !self.core.contains_key(name) {
                return Err(RegistryError::UnknownModule((*name).to_owned()));
            }
        }

        // Validate grpc_hub
        if let Some((name, _)) = &self.grpc_hub
            && !self.core.contains_key(name)
        {
            return Err(RegistryError::UnknownModule((*name).to_owned()));
        }

        Ok(())
    }

    /// Build dependency graph and return module names, adjacency list, and index mapping.
    fn build_dependency_graph(&self) -> Result<DependencyGraph, RegistryError> {
        let names: Vec<&'static str> = self.core.keys().copied().collect();
        let mut idx: HashMap<&'static str, usize> = HashMap::new();
        for (i, &n) in names.iter().enumerate() {
            idx.insert(n, i);
        }

        let mut adj = vec![Vec::<usize>::new(); names.len()];

        for (&n, &deps) in &self.deps {
            let u = *idx
                .get(n)
                .ok_or_else(|| RegistryError::UnknownModule(n.to_owned()))?;
            for &d in deps {
                let v = *idx.get(d).ok_or_else(|| RegistryError::UnknownDependency {
                    module: n.to_owned(),
                    depends_on: d.to_owned(),
                })?;
                // edge d -> n (dep before module)
                adj[v].push(u);
            }
        }

        Ok((names, adj, idx))
    }

    /// Assemble final module entries in topological order.
    fn assemble_entries(
        &self,
        order: &[usize],
        names: &[&'static str],
    ) -> Result<Vec<ModuleEntry>, RegistryError> {
        let mut entries = Vec::with_capacity(order.len());
        for &i in order {
            let name = names[i];
            let deps = *self
                .deps
                .get(name)
                .ok_or_else(|| RegistryError::MissingDeps(name.to_owned()))?;

            let core = self
                .core
                .get(name)
                .cloned()
                .ok_or_else(|| RegistryError::CoreNotFound(name.to_owned()))?;

            // Build the capability set for this module
            let mut caps = CapabilitySet::new();

            // Add capabilities from the main capabilities map
            if let Some(module_caps) = self.capabilities.get(name) {
                for cap in module_caps {
                    caps.push(cap.clone());
                }
            }

            // Add rest_host if this module is the host
            if let Some((host_name, module)) = &self.rest_host
                && *host_name == name
            {
                caps.push(Capability::ApiGateway(module.clone()));
            }

            // Add grpc_hub if this module is the hub
            if let Some((hub_name, module)) = &self.grpc_hub
                && *hub_name == name
            {
                caps.push(Capability::GrpcHub(module.clone()));
            }

            let entry = ModuleEntry {
                name,
                deps,
                core,
                caps,
            };
            entries.push(entry);
        }
        Ok(entries)
    }

    /// Finalize & topo-sort; verify deps & capability binding to known cores.
    ///
    /// # Errors
    /// Returns `RegistryError` if validation fails or a dependency cycle is detected.
    pub fn build_topo_sorted(self) -> Result<ModuleRegistry, RegistryError> {
        // 1) Validate all capabilities
        self.validate_capabilities()?;

        // 2) Build dependency graph
        let (names, adj, _idx) = self.build_dependency_graph()?;

        // 3) Cycle detection using DFS with path tracking
        if let Some(cycle_path) = Self::detect_cycle_with_path(&names, &adj) {
            return Err(RegistryError::CycleDetected { path: cycle_path });
        }

        // 4) Kahn's algorithm for topological sorting
        let mut indeg = vec![0usize; names.len()];
        for adj_list in &adj {
            for &target in adj_list {
                indeg[target] += 1;
            }
        }

        let mut q = VecDeque::new();
        for (i, &degree) in indeg.iter().enumerate() {
            if degree == 0 {
                q.push_back(i);
            }
        }

        let mut order = Vec::with_capacity(names.len());
        while let Some(u) = q.pop_front() {
            order.push(u);
            for &w in &adj[u] {
                indeg[w] -= 1;
                if indeg[w] == 0 {
                    q.push_back(w);
                }
            }
        }

        // 5) Assemble final entries
        let entries = self.assemble_entries(&order, &names)?;

        // Collect grpc_hub and grpc_services for the final registry
        let grpc_hub = self.grpc_hub.as_ref().map(|(name, _)| (*name).to_owned());

        // Collect grpc_services from capabilities
        let mut grpc_services: Vec<(String, Arc<dyn contracts::GrpcServiceCapability>)> =
            Vec::new();
        for (name, caps) in &self.capabilities {
            for cap in caps {
                if let Capability::GrpcService(service) = cap {
                    grpc_services.push(((*name).to_owned(), service.clone()));
                }
            }
        }

        tracing::info!(
            modules = ?entries.iter().map(|e| e.name).collect::<Vec<_>>(),
            "Module dependency order resolved (topo)"
        );

        Ok(ModuleRegistry {
            modules: entries,
            grpc_hub,
            grpc_services,
        })
    }
}

/// Structured errors for the module registry.
#[derive(Debug, Error)]
pub enum RegistryError {
    // Phase errors with module context
    #[error("pre-init failed for module '{module}'")]
    PreInit {
        module: &'static str,
        #[source]
        source: anyhow::Error,
    },
    #[error("initialization failed for module '{module}'")]
    Init {
        module: &'static str,
        #[source]
        source: anyhow::Error,
    },
    #[error("post-init failed for module '{module}'")]
    PostInit {
        module: &'static str,
        #[source]
        source: anyhow::Error,
    },
    #[error("start failed for '{module}'")]
    Start {
        module: &'static str,
        #[source]
        source: anyhow::Error,
    },

    #[error("DB migration failed for module '{module}'")]
    DbMigrate {
        module: &'static str,
        #[source]
        source: anyhow::Error,
    },
    #[error("REST prepare failed for host module '{module}'")]
    RestPrepare {
        module: &'static str,
        #[source]
        source: anyhow::Error,
    },
    #[error("REST registration failed for module '{module}'")]
    RestRegister {
        module: &'static str,
        #[source]
        source: anyhow::Error,
    },
    #[error("REST finalize failed for host module '{module}'")]
    RestFinalize {
        module: &'static str,
        #[source]
        source: anyhow::Error,
    },
    #[error(
        "REST phase requires an gateway host: modules with capability 'rest' found, but no module with capability 'rest_host'"
    )]
    RestRequiresHost,
    #[error("multiple 'rest_host' modules detected; exactly one is allowed")]
    MultipleRestHosts,
    #[error("REST host module not found after validation")]
    RestHostNotFoundAfterValidation,
    #[error("REST host missing from entry")]
    RestHostMissingFromEntry,

    // gRPC-related errors
    #[error("gRPC registration failed for module '{module}'")]
    GrpcRegister {
        module: String,
        #[source]
        source: anyhow::Error,
    },
    #[error(
        "gRPC phase requires a hub: modules with capability 'grpc' found, but no module with capability 'grpc_hub'"
    )]
    GrpcRequiresHub,
    #[error("multiple 'grpc_hub' modules detected; exactly one is allowed")]
    MultipleGrpcHubs,

    // OoP spawn errors
    #[error("OoP spawn failed for module '{module}'")]
    OopSpawn {
        module: String,
        #[source]
        source: anyhow::Error,
    },

    // Cancellation error
    #[error("operation cancelled by termination signal")]
    Cancelled,

    // Build/topo-sort errors
    #[error("unknown module '{0}'")]
    UnknownModule(String),
    #[error("module '{module}' depends on unknown '{depends_on}'")]
    UnknownDependency { module: String, depends_on: String },
    #[error("cyclic dependency detected: {}", path.join(" -> "))]
    CycleDetected { path: Vec<&'static str> },
    #[error("missing deps for '{0}'")]
    MissingDeps(String),
    #[error("core not found for '{0}'")]
    CoreNotFound(String),
    #[error("invalid registry configuration:\n{errors:#?}")]
    InvalidRegistryConfiguration { errors: Vec<String> },
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use std::sync::Arc;

    // Use the real contracts/context APIs from the crate to avoid type mismatches.
    use crate::context::ModuleCtx;
    use crate::contracts;

    /* --------------------------- Test helpers ------------------------- */
    #[derive(Default)]
    struct DummyCore;
    #[async_trait::async_trait]
    impl contracts::Module for DummyCore {
        async fn init(&self, _ctx: &ModuleCtx) -> anyhow::Result<()> {
            Ok(())
        }
    }

    /* ------------------------------- Tests ---------------------------- */

    #[test]
    fn topo_sort_happy_path() {
        let mut b = RegistryBuilder::default();
        // cores
        b.register_core_with_meta("core_a", &[], Arc::new(DummyCore));
        b.register_core_with_meta("core_b", &["core_a"], Arc::new(DummyCore));

        let reg = b.build_topo_sorted().unwrap();
        let order: Vec<_> = reg.modules().iter().map(|m| m.name).collect();
        assert_eq!(order, vec!["core_a", "core_b"]);
    }

    #[test]
    fn unknown_dependency_error() {
        let mut b = RegistryBuilder::default();
        b.register_core_with_meta("core_a", &["missing_dep"], Arc::new(DummyCore));

        let err = b.build_topo_sorted().unwrap_err();
        match err {
            RegistryError::UnknownDependency { module, depends_on } => {
                assert_eq!(module, "core_a");
                assert_eq!(depends_on, "missing_dep");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn cyclic_dependency_detected() {
        let mut b = RegistryBuilder::default();
        b.register_core_with_meta("a", &["b"], Arc::new(DummyCore));
        b.register_core_with_meta("b", &["a"], Arc::new(DummyCore));

        let err = b.build_topo_sorted().unwrap_err();
        match err {
            RegistryError::CycleDetected { path } => {
                // Should contain both modules in the cycle
                assert!(path.contains(&"a"));
                assert!(path.contains(&"b"));
                assert!(path.len() >= 3); // At least a -> b -> a
            }
            other => panic!("expected CycleDetected, got: {other:?}"),
        }
    }

    #[test]
    fn complex_cycle_detection_with_path() {
        let mut b = RegistryBuilder::default();
        // Create a more complex cycle: a -> b -> c -> a
        b.register_core_with_meta("a", &["b"], Arc::new(DummyCore));
        b.register_core_with_meta("b", &["c"], Arc::new(DummyCore));
        b.register_core_with_meta("c", &["a"], Arc::new(DummyCore));
        // Add an unrelated module to ensure we only detect the actual cycle
        b.register_core_with_meta("d", &[], Arc::new(DummyCore));

        let err = b.build_topo_sorted().unwrap_err();
        match err {
            RegistryError::CycleDetected { path } => {
                // Should contain all modules in the cycle
                assert!(path.contains(&"a"));
                assert!(path.contains(&"b"));
                assert!(path.contains(&"c"));
                assert!(!path.contains(&"d")); // Should not include unrelated module
                assert!(path.len() >= 4); // At least a -> b -> c -> a

                // Verify the error message is helpful
                let error_msg = format!("{}", RegistryError::CycleDetected { path: path.clone() });
                assert!(error_msg.contains("cyclic dependency detected"));
                assert!(error_msg.contains("->"));
            }
            other => panic!("expected CycleDetected, got: {other:?}"),
        }
    }

    #[test]
    fn duplicate_core_reported_in_configuration_errors() {
        let mut b = RegistryBuilder::default();
        b.register_core_with_meta("a", &[], Arc::new(DummyCore));
        // duplicate
        b.register_core_with_meta("a", &[], Arc::new(DummyCore));

        let err = b.build_topo_sorted().unwrap_err();
        match err {
            RegistryError::InvalidRegistryConfiguration { errors } => {
                assert!(
                    errors.iter().any(|e| e.contains("already registered")),
                    "expected duplicate registration error, got {errors:?}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn rest_capability_without_core_fails() {
        let mut b = RegistryBuilder::default();
        b.register_core_with_meta("core_a", &[], Arc::new(DummyCore));
        // Register a rest capability for a module that doesn't exist
        b.register_rest_with_meta("unknown_module", Arc::new(DummyRest));

        let err = b.build_topo_sorted().unwrap_err();
        match err {
            RegistryError::UnknownModule(name) => {
                assert_eq!(name, "unknown_module");
            }
            other => panic!("expected UnknownModule, got: {other:?}"),
        }
    }

    #[test]
    fn db_capability_without_core_fails() {
        let mut b = RegistryBuilder::default();
        b.register_core_with_meta("core_a", &[], Arc::new(DummyCore));
        // Register a db capability for a module that doesn't exist
        b.register_db_with_meta("unknown_module", Arc::new(DummyDb));

        let err = b.build_topo_sorted().unwrap_err();
        match err {
            RegistryError::UnknownModule(name) => {
                assert_eq!(name, "unknown_module");
            }
            other => panic!("expected UnknownModule, got: {other:?}"),
        }
    }

    #[test]
    fn stateful_capability_without_core_fails() {
        let mut b = RegistryBuilder::default();
        b.register_core_with_meta("core_a", &[], Arc::new(DummyCore));
        // Register a stateful capability for a module that doesn't exist
        b.register_stateful_with_meta("unknown_module", Arc::new(DummyStateful));

        let err = b.build_topo_sorted().unwrap_err();
        match err {
            RegistryError::UnknownModule(name) => {
                assert_eq!(name, "unknown_module");
            }
            other => panic!("expected UnknownModule, got: {other:?}"),
        }
    }

    #[test]
    fn capability_query_works() {
        let mut b = RegistryBuilder::default();
        let module = Arc::new(DummyCore);
        b.register_core_with_meta("test", &[], module);
        b.register_db_with_meta("test", Arc::new(DummyDb));
        b.register_rest_with_meta("test", Arc::new(DummyRest));

        let reg = b.build_topo_sorted().unwrap();
        let entry = &reg.modules()[0];

        assert!(entry.caps.has::<DatabaseCap>());
        assert!(entry.caps.has::<RestApiCap>());
        assert!(!entry.caps.has::<SystemCap>());

        assert!(entry.caps.query::<DatabaseCap>().is_some());
        assert!(entry.caps.query::<RestApiCap>().is_some());
        assert!(entry.caps.query::<SystemCap>().is_none());
    }

    #[test]
    fn rest_host_capability_without_core_fails() {
        let mut b = RegistryBuilder::default();
        b.register_core_with_meta("core_a", &[], Arc::new(DummyCore));
        // Set rest_host to a module that doesn't exist
        b.register_rest_host_with_meta("unknown_host", Arc::new(DummyRestHost));

        let err = b.build_topo_sorted().unwrap_err();
        match err {
            RegistryError::UnknownModule(name) => {
                assert_eq!(name, "unknown_host");
            }
            other => panic!("expected UnknownModule, got: {other:?}"),
        }
    }

    #[test]
    fn module_entry_getters_work() {
        let mut b = RegistryBuilder::default();
        b.register_core_with_meta("alpha", &[], Arc::new(DummyCore));
        b.register_core_with_meta("beta", &["alpha"], Arc::new(DummyCore));
        b.register_rest_with_meta("beta", Arc::new(DummyRest));

        let reg = b.build_topo_sorted().unwrap();
        let beta = reg.modules().iter().find(|e| e.name() == "beta").unwrap();

        assert_eq!(beta.name(), "beta");
        assert_eq!(beta.deps(), &["alpha"]);
        assert!(beta.caps().has::<RestApiCap>());
    }

    #[test]
    fn test_module_registry_builds() {
        let registry = ModuleRegistry::discover_and_build();
        assert!(registry.is_ok(), "Registry should build successfully");
    }

    /* Test helper implementations */
    #[derive(Default, Clone)]
    struct DummyRest;
    impl contracts::RestApiCapability for DummyRest {
        fn register_rest(
            &self,
            _ctx: &crate::context::ModuleCtx,
            _router: axum::Router,
            _openapi: &dyn crate::api::OpenApiRegistry,
        ) -> anyhow::Result<axum::Router> {
            Ok(axum::Router::new())
        }
    }

    #[derive(Default)]
    struct DummyDb;
    impl contracts::DatabaseCapability for DummyDb {
        fn migrations(&self) -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
            vec![]
        }
    }

    #[derive(Default)]
    struct DummyStateful;
    #[async_trait::async_trait]
    impl contracts::RunnableCapability for DummyStateful {
        async fn start(&self, _cancel: tokio_util::sync::CancellationToken) -> anyhow::Result<()> {
            Ok(())
        }
        async fn stop(&self, _cancel: tokio_util::sync::CancellationToken) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct DummyRestHost;
    impl contracts::ApiGatewayCapability for DummyRestHost {
        fn rest_prepare(
            &self,
            _ctx: &crate::context::ModuleCtx,
            router: axum::Router,
        ) -> anyhow::Result<axum::Router> {
            Ok(router)
        }
        fn rest_finalize(
            &self,
            _ctx: &crate::context::ModuleCtx,
            router: axum::Router,
            _module_router: axum::Router,
        ) -> anyhow::Result<axum::Router> {
            Ok(router)
        }
        fn as_registry(&self) -> &dyn crate::contracts::OpenApiRegistry {
            panic!("DummyRestHost::as_registry should not be called in tests")
        }
    }
}
