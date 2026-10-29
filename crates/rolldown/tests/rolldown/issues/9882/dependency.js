// Re-exported under a different name (`SharedEnum`) so the entry can declare a colliding local
// named `sharedValue` without clashing with the import binding. `sharedValue` is the chunk-root
// name the entry's local shadows. Imported by the CJS-wrapped entry, so this module is wrapped
// with `__esmMin` and `sharedValue` is hoisted to chunk-root scope.
let sharedValue = { EventMatch: 'event_match' };
export { sharedValue as SharedEnum };
