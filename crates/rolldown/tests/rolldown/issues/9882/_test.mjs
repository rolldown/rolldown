// Regression test for #9882. `export default require_main()` eval's the wrapped entry on import.
// Pre-fix, the entry's `var sharedValue` shadowed the dependency's chunk-root `sharedValue`, so
// `SharedEnum.EventMatch` read `undefined.EventMatch` and threw. Importing without throwing means
// the local was deconflicted (e.g. to `sharedValue$1`) and no longer shadows.
await import('./dist/main.js');
