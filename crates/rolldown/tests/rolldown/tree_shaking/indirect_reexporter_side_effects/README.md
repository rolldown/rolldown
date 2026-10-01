# Indirect re-exporter side effects

When a used export forwards a locally imported binding, evaluating the indirect re-exporter must retain its own side effects even when `moduleSideEffects` is disabled. A direct `export { value } from './source.js'` does not demand the re-exporter's body, and an unused indirect re-exporter remains removable.

The fixture covers used and unused indirect re-exporters, a direct re-exporter, and a mixed direct/indirect chain. It mirrors Rollup's `respect-reexporter-side-effects` tree-shaking test.
