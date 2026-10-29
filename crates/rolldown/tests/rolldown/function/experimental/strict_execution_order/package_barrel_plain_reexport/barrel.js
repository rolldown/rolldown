// A package barrel in the `@carbon/react` shape: it does NOT use `export { X } from './x'`
// (a re-export record). Instead it *plain-imports* each component and re-exports the local binding
// through a source-less `export { ... }` clause. The plain-import records are not marked
// `IsReExportOnly`, and the side-effect-free components are not top-level execution dependencies, so
// under strict order the barrel's `init_*` dropped its forward to each component's `init_*` — the
// component's `init_*` ended up with zero call sites and its `forwardRef`/definition stayed
// `undefined` until a render read it. The barrel carries a top-level side effect (like the package's
// `feature-flags` import) so it stays a real wrapped module in its own chunk rather than being
// inlined, which is what makes the barrel's own cross-chunk `init_*` forward the load-bearing path.
import Checkbox from './checkbox.js';
import Radio from './radio.js';

(globalThis.__events ??= []).push('barrel');

export { Checkbox, Radio };
