# flare-plugin-protocol

Business-neutral contracts shared by platform control, Core capability routing and plugin SDKs. This crate has no service, database, broker, deployment or commercial authorization dependency.

`PluginManifest::from_json` bounds input and validates identifiers, semantic/API versions, operation schemas, permission references, data scopes, dependencies, runtime limits and UI declarations. Unknown fields are rejected. The generated JSON Schema checks structure; semantic validation remains mandatory. Server runtimes require an artifact digest/signature reference; fetching and verifying actual artifact bytes is the runtime's responsibility.

`PublisherTrustStore` verifies detached Ed25519 signatures over exact manifest bytes, binding key id to publisher id. `VerifiedManifest` cannot be constructed by clients without verification. `PluginRegistry` accepts verified manifests, preserves immutable versions, rejects namespace takeovers and resolves version-pinned dependency closures with bounded backtracking. This in-memory domain model requires a platform persistence adapter before online publication.

`TenantPluginGrant` separates approval from tenant enable and platform flags. Authorization requires matching tenant, plugin version and manifest digest, approved operation/permission/data scope, actor permission and a valid projection time window. The receiving adapter must verify the control-plane projection signature and derive actor identity from verified transport. This type is not a substitute for authentication.

`PluginLifecycle` models validated start, running, drain, stop and failure with revision fencing. Runtime adapters own effects, health, resource isolation and recovery; lifecycle metadata does not start processes.

## Commands

```sh
cargo test
cargo run --bin plugin-manifest -- path/to/plugin.json
cargo run --bin plugin-schema -- ../../flare-sdk-plugin/schema/platform_plugin_manifest.schema.json
```

Client composition-time SDK package manifests remain a separate generated contract. Platform control persists signed originals and immutable digests; deployed services and tenant grants reference those versions. Marketplace queries do not belong in the runtime call path.

## License

Apache-2.0; see the repository LICENSE.
