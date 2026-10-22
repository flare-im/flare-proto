# flare-plugin-protocol

Business-neutral contracts shared by platform control, Core capability routing and plugin SDKs. This crate has no service, database, broker, deployment or commercial authorization dependency.

`PluginManifest::from_json` bounds input and validates identifiers, semantic/API versions, operation schemas, permission references, data scopes, dependencies, runtime limits and UI declarations. Unknown fields are rejected. The generated JSON Schema checks structure; semantic validation remains mandatory. Server runtimes require an artifact digest/signature reference; fetching and verifying actual artifact bytes is the runtime's responsibility.

`PublisherTrustStore` verifies detached Ed25519 signatures over exact manifest bytes, binding key id to publisher id. `VerifiedManifest` cannot be constructed by clients without verification. `PluginRegistry` accepts verified manifests, preserves immutable versions, rejects namespace takeovers and resolves version-pinned dependency closures with bounded backtracking. This in-memory domain model requires a platform persistence adapter before online publication.

`TenantPluginGrant` separates approval from tenant enable and platform flags. Authorization requires matching tenant, plugin version and manifest digest, approved operation/permission/data scope, actor permission and a valid projection time window. The receiving adapter must verify the control-plane projection signature and derive actor identity from verified transport. This type is not a substitute for authentication.

`PluginLifecycle` models validated start, running, drain, stop and failure with revision fencing. Runtime adapters own effects, health, resource isolation and recovery; lifecycle metadata does not start processes.

`UiDocument` is data only. Besides labelled fields and actions it carries display blocks (text, fields, choices, entries, people, status, progress, time, divider, note, and rows on admin pages) and form inputs (text, textarea, number, toggle, select, datetime, options, members); actions may collect a form, ask for confirmation and carry a style. `UiCatalog::resolve_with_state` resolves a card over its payload, the entity's shared state and the viewer's local state, in that order, into a flat `ResolvedPluginView`; `validate_form_values` checks submitted forms with the same rules on clients and plugin services. Contributions may bind an entity (`entityPath`), allow forwarding, restrict a composer entry to an audience, declare a detail view, or be an `admin-page` for the console. Every block and the detail take `hiddenWhen` conditions like actions (at most four, read on the merged card data or, on an admin page, the page): a matching block is left out, and a matching detail resolves to `detail: false`. An entries block reports `more` only while the contribution offers its detail, and a string in its array is the entry text when no `textPath` is given, so a string list kept in shared state renders as is.

Further optional document fields, all absent from older manifests:

- Conditions take `allOf: [condition]` (up to four, one level, no `path` of its own) besides `anyOf`, `viewer` and `exists`, in every `hiddenWhen` and `closedWhen`.
- A choices block declares static `options: [{label}]` instead of `optionsPath`; labels go through `texts`, counts and the selection are still read from their paths.
- An entries block's `userIdsPath` names an array of user ids aligned with the entries by position, for string entries such as solitaire lines.
- An admin-page rows block lists its filter inputs in `filters`; `validate_inputs` checks only those, and the other inputs belong to row action forms. Without `filters` every input is a filter.
- An options or members input reads its largest item count from `maxPath`, falling back to the declared `max`.

Composer data: a composer-action document is resolved, its form validated (`validate_form_over`) and its action input bound against `{"config": <client-visible configuration>}` (`composer_data`), so `defaultPath` and `maxPath` read `/config/<key>`. No message payload or state layer takes part.

Detail paging: a detail with `cursorPath` names the next-page cursor in the detail result. The resolved detail view carries it as `nextCursor` (a non-blank scalar of at most 256 bytes; anything else means the last page). The first page is requested with the detail input alone. The next page is requested with the same input plus `cursor` (`UiDetail::page_input`, also for an admin row's `detailInput`), resolved against the same document, and appended with `ResolvedPluginView::append_page`: entries blocks pair up in order and number on, everything else stays as on the first page. A plugin returns at most an entries block's `limit` per page.

The host bridge reads are `im.conversation.read`, `im.conversation.participants.read` (pages of up to 500), `im.conversation.members.check` (`{"userIds": [...]}`, up to 50 unique ids, answered with `{"members": [...], "nonMembers": [...]}` in request order) and `social.group.member.read`, each scoped to the conversation the ticket binds.

`SharedState` is the entity snapshot Core turns into `EVENT_MESSAGE_PLUGIN_STATE`. Plugins return it as `data.sharedState`, or publish it through the one host write capability `im.plugin.entity_state.publish` (`ResourceWriteRequest`). User tickets (`PluginCallClaims`), staff tickets for admin-page operations (`PluginStaffCallClaims`, permission `plugin:<pluginId>:admin`) and the plugin runtime credential (`PluginRuntimeClaims`, bound to one deployment and one exact request body) use separate signing domains, so none verifies as another. Configuration properties marked `x-flare-client-visible: true` form the subset clients receive with the catalog.

## Commands

```sh
cargo test
cargo run --bin plugin-manifest -- path/to/plugin.json
cargo run --bin plugin-schema -- ../../flare-sdk-plugin/schema/platform_plugin_manifest.schema.json
```

Client composition-time SDK package manifests remain a separate generated contract. Platform control persists signed originals and immutable digests; deployed services and tenant grants reference those versions. Marketplace queries do not belong in the runtime call path.

## License

Apache-2.0; see the repository LICENSE.
