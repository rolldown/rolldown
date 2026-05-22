# @rolldown/debug

Utilities and generated TypeScript types for reading Rolldown devtools output.

When `devtools` is enabled, Rolldown writes JSON-lines files to:

```text
node_modules/.rolldown/<session_id>/
  meta.json
  logs.json
```

`logs.json` is complete after `await bundle.close()` resolves.

## Parse Events

```ts
import fs from 'node:fs';
import { parseToEvents, type Event, type StringRef } from '@rolldown/debug';

const data = fs.readFileSync('node_modules/.rolldown/<session_id>/logs.json', 'utf8');
const events = parseToEvents(data.trim());

type ActionEvent = Exclude<Event, StringRef> & { build_id: string };
const actionEvents = events.filter((event): event is ActionEvent => 'build_id' in event);
```

Action events include `session_id`, `build_id`, `timestamp`, and an `action` discriminator. `StringRef` events contain deduplicated large string content and do not belong to a specific build.

## Consume Package Data

Use `PackageGraphReady` to build a package table. In watch/rebuild sessions, `logs.json` is append-only, so select the events for the `build_id` being displayed.

```ts
import fs from 'node:fs';
import { parseToEvents, type Event, type StringRef } from '@rolldown/debug';

type ActionEvent = Exclude<Event, StringRef> & { build_id: string };

function isActionEvent(event: Event): event is ActionEvent {
  return 'build_id' in event;
}

const data = fs.readFileSync('node_modules/.rolldown/<session_id>/logs.json', 'utf8');
const actionEvents = parseToEvents(data.trim()).filter(isActionEvent);
const buildId = actionEvents.at(-1)?.build_id;

function latest<T extends ActionEvent['action']>(
  action: T,
): Extract<ActionEvent, { action: T }> | undefined {
  for (let i = actionEvents.length - 1; i >= 0; i--) {
    const event = actionEvents[i];
    if (event.build_id === buildId && event.action === action) {
      return event as Extract<ActionEvent, { action: T }>;
    }
  }
}

const packageGraph = latest('PackageGraphReady');
const chunkGraph = latest('ChunkGraphReady');
const moduleGraph = latest('ModuleGraphReady');

const chunksById = new Map(chunkGraph?.chunks.map((chunk) => [chunk.chunk_id, chunk]) ?? []);
const modulesById = new Map(moduleGraph?.modules.map((module) => [module.id, module]) ?? []);

const packageRows = packageGraph?.packages.map((pkg) => ({
  id: pkg.package_id,
  label: pkg.name ?? pkg.package_root,
  version: pkg.version ?? 'unknown',
  dependencyType: pkg.dependency_type,
  renderedSize: pkg.size,
  isUsed: pkg.is_used,
  chunks: pkg.chunk_ids.flatMap((chunkId) => {
    const chunk = chunksById.get(chunkId);
    return chunk ? [chunk] : [];
  }),
  modules: pkg.modules.map(
    (moduleId) =>
      modulesById.get(moduleId) ?? {
        id: moduleId,
      },
  ),
}));
```

## Package Fields

| Field              | Consumer meaning                                                                                                                                                                                                     |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `package_id`       | Stable row key for one emitted package record. Prefer this, or `package_root`, over `name@version`.                                                                                                                  |
| `name` / `version` | Metadata from the resolved package manifest. Either can be `null`; fall back to `package_root` or an unknown version label in UI.                                                                                    |
| `package_root`     | Package directory. Useful for display, duplicate detection, and row identity.                                                                                                                                        |
| `is_used`          | `true` when at least one module from the package appears in a generated chunk. `false` means the package was resolved but tree-shaken away.                                                                          |
| `dependency_type`  | `direct` when any package module is imported by a source module under the build `cwd` and outside `node_modules`; otherwise `transitive`. Rolldown does not inspect `package.json` dependency fields for this value. |
| `size`             | Sum of rendered package module bytes after tree-shaking/codegen and before `renderChunk`, minification, banners, and final asset emission. This is package attribution data, not final asset size.                   |
| `modules`          | Generated chunk module IDs for the package. Join with `ModuleGraphReady.modules[].id` when the module graph is needed. Empty for unused packages.                                                                    |
| `chunk_ids`        | IDs of chunks containing modules from the package. Join with `ChunkGraphReady.chunks[].chunk_id`. Empty for unused packages.                                                                                         |

To detect duplicate packages, group records by non-null `name`, then mark groups that contain more than one version or package root.

See `meta/design/devtools.md` in the Rolldown repository for implementation details and event lifecycle notes.
