# Changelog

All notable changes to this repository are documented in this file.

This file follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

release-plz updates this file in the Release PR.

## [Unreleased]

## [0.2.10](https://github.com/cyberfabric/cyberfabric-core/compare/types-sdk-v0.2.9...types-sdk-v0.2.10) - 2026-02-18

### Other

- update Cargo.toml dependencies

## [0.1.2](https://github.com/cyberfabric/cyberfabric-core/compare/cf-static-tr-plugin-v0.1.1...cf-static-tr-plugin-v0.1.2) - 2026-02-18

### Added

- implement authentication and authorization resolvers (by @aviator5) - #612

### Fixed

- validate GTS registration results instead of silently discarding them (by @aviator5) - #612
- update module names to use kebab-case convention (by @yoskini) - #459

### Other

- release (by @github-actions[bot]) - #595

### Security

- *(auth)* require subject_id and subject_tenant_id in SecurityContext builder (by @aviator5)

### Contributors

* @github-actions[bot]
* @aviator5
* @yoskini

## [0.1.0](https://github.com/cyberfabric/cyberfabric-core/releases/tag/cf-static-authz-plugin-v0.1.0) - 2026-02-18

### Added

- implement authentication and authorization resolvers (by @aviator5) - #612

### Fixed

- validate GTS registration results instead of silently discarding them (by @aviator5) - #612

### Other

- release (by @github-actions[bot]) - #595
- *(plugins)* extract shared choose_plugin_instance into modkit::plugins (by @aviator5)

### Security

- *(authz)* deny access on nil/missing tenant in static-authz-plugin (by @aviator5)

### Contributors

* @github-actions[bot]
* @aviator5

## [0.1.0](https://github.com/cyberfabric/cyberfabric-core/releases/tag/cf-static-authn-plugin-v0.1.0) - 2026-02-18

### Added

- implement authentication and authorization resolvers (by @aviator5) - #612

### Fixed

- validate GTS registration results instead of silently discarding them (by @aviator5) - #612

### Other

- release (by @github-actions[bot]) - #595
- *(config)* streamline quickstart.yaml auth configuration (by @aviator5)
- *(plugins)* extract shared choose_plugin_instance into modkit::plugins (by @aviator5)

### Security

- *(auth)* require subject_id and subject_tenant_id in SecurityContext builder (by @aviator5)

### Contributors

* @github-actions[bot]
* @aviator5

## [0.1.2](https://github.com/cyberfabric/cyberfabric-core/compare/cf-single-tenant-tr-plugin-v0.1.1...cf-single-tenant-tr-plugin-v0.1.2) - 2026-02-18

### Added

- implement authentication and authorization resolvers (by @aviator5) - #612

### Fixed

- validate GTS registration results instead of silently discarding them (by @aviator5) - #612
- update module names to use kebab-case convention (by @yoskini) - #459

### Other

- release (by @github-actions[bot]) - #595

### Security

- *(auth)* require subject_id and subject_tenant_id in SecurityContext builder (by @aviator5)

### Contributors

* @github-actions[bot]
* @aviator5
* @yoskini

## [0.2.10](https://github.com/cyberfabric/cyberfabric-core/compare/cf-modkit-http-v0.2.8...cf-modkit-http-v0.2.10) - 2026-02-18

### Other

- enable let_underscore_must_use because coderabbitai check it (by @lansfy) - #616

### Contributors

* @lansfy

## [0.2.9](https://github.com/cyberfabric/cyberfabric-core/compare/types-sdk-v0.2.8...types-sdk-v0.2.9) - 2026-02-17

### Other

- update Cargo.toml dependencies

## [0.1.0](https://github.com/cyberfabric/cyberfabric-core/releases/tag/cf-static-authz-plugin-v0.1.0) - 2026-02-17

### Added

- implement authentication and authorization resolvers (by @aviator5) - #612

### Fixed

- validate GTS registration results instead of silently discarding them (by @aviator5) - #612

### Other

- *(plugins)* extract shared choose_plugin_instance into modkit::plugins (by @aviator5)

### Security

- *(authz)* deny access on nil/missing tenant in static-authz-plugin (by @aviator5)

### Contributors

* @aviator5

## [0.1.0](https://github.com/cyberfabric/cyberfabric-core/releases/tag/cf-static-authn-plugin-v0.1.0) - 2026-02-17

### Added

- implement authentication and authorization resolvers (by @aviator5) - #612

### Fixed

- validate GTS registration results instead of silently discarding them (by @aviator5) - #612

### Other

- *(config)* streamline quickstart.yaml auth configuration (by @aviator5)
- *(plugins)* extract shared choose_plugin_instance into modkit::plugins (by @aviator5)

### Security

- *(auth)* require subject_id and subject_tenant_id in SecurityContext builder (by @aviator5)

### Contributors

* @aviator5

## [0.1.0](https://github.com/cyberfabric/cyberfabric-core/releases/tag/cf-authz-resolver-v0.1.0) - 2026-02-17

### Added

- implement authentication and authorization resolvers (by @aviator5) - #612

### Fixed

- validate GTS registration results instead of silently discarding them (by @aviator5) - #612

### Other

- *(plugins)* extract shared choose_plugin_instance into modkit::plugins (by @aviator5)

### Contributors

* @aviator5

## [0.1.0](https://github.com/cyberfabric/cyberfabric-core/releases/tag/cf-authz-resolver-sdk-v0.1.0) - 2026-02-17

### Added

- implement authentication and authorization resolvers (by @aviator5) - #612

### Other

- *(plugins)* extract shared choose_plugin_instance into modkit::plugins (by @aviator5)

### Security

- *(auth)* require subject_id and subject_tenant_id in SecurityContext builder (by @aviator5)

### Contributors

* @aviator5

## [0.1.0](https://github.com/cyberfabric/cyberfabric-core/releases/tag/cf-authn-resolver-v0.1.0) - 2026-02-17

### Added

- implement authentication and authorization resolvers (by @aviator5) - #612

### Fixed

- validate GTS registration results instead of silently discarding them (by @aviator5) - #612

### Other

- *(plugins)* extract shared choose_plugin_instance into modkit::plugins (by @aviator5)

### Contributors

* @aviator5

## [0.2.9](https://github.com/cyberfabric/cyberfabric-core/compare/cf-modkit-http-v0.2.8...cf-modkit-http-v0.2.9) - 2026-02-17

### Other

- enable let_underscore_must_use because coderabbitai check it (by @lansfy) - #616

### Contributors

* @lansfy

## [0.1.0](https://github.com/cyberfabric/cyberfabric-core/releases/tag/cf-authn-resolver-sdk-v0.1.0) - 2026-02-17

### Added

- implement authentication and authorization resolvers (by @aviator5) - #612

### Contributors

* @aviator5

## [0.2.9](https://github.com/cyberfabric/cyberfabric-core/compare/cf-modkit-v0.2.8...cf-modkit-v0.2.9) - 2026-02-17

### Added

- implement authentication and authorization resolvers (by @aviator5) - #612

### Fixed

- show colors on supported windows terminal (by @lansfy) - #590

### Other

- Merge pull request #612 from aviator5/authn-authz-impl-p1 (by @MikeFalcon77) - #612
- *(plugins)* extract shared choose_plugin_instance into modkit::plugins (by @aviator5)
- enable let_underscore_must_use because coderabbitai check it (by @lansfy) - #616
- Merge pull request #594 from lansfy/fix_random_fail_in_test (by @MikeFalcon77) - #594

### Contributors

* @MikeFalcon77
* @aviator5
* @lansfy

## [0.2.8](https://github.com/cyberfabric/cyberfabric-core/compare/types-sdk-v0.2.7...types-sdk-v0.2.8) - 2026-02-12

### Other

- update Cargo.toml dependencies

## [0.2.7](https://github.com/cyberfabric/cyberfabric-core/compare/cf-modkit-v0.2.1...cf-modkit-v0.2.7) - 2026-02-12

### Added

- *(modkit-http)* introduce HttpClient (hyper+tower), replace reqwest and harden JWKS handling (by @MikeFalcon77)

### Fixed

- update module names to use kebab-case convention (by @yoskini) - #459
- *(dylint/de1301)* allow proc-macro impl methods via visit_assoc_item (by @Artifizer)

### Other

- Merge branch 'main' into shutdown-nl (by @Artifizer) - #475
- Merge branch 'main' into rescue-docs-standards (by @Artifizer) - #478

### Contributors

* @yoskini
* @MikeFalcon77
* @Artifizer

## [0.1.0] - 2026-01-23

### Added

- Initial release.
