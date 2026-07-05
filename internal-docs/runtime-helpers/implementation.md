# Runtime Helpers

## `__commonJS` and `__commonJSMin`: release `cb` after initialization

After the first call in either helper, `mod` is set and `cb` is never accessed again. Without an explicit `cb = null`, the factory is permanently retained in the closure — a memory leak in long-lived processes (e.g. SSR servers loading bundles via `vm.createContext`).

Reference: https://github.com/rolldown/rolldown/issues/9063
