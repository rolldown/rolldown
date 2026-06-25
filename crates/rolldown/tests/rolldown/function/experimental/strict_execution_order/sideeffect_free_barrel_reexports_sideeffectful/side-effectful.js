// Root package (sideEffects NOT waived). A top-level *call* is an unambiguous side effect
// rolldown keeps once this module is included (unlike a bare property write, which its analysis
// can drop). This is the side effect that must run in execution order whenever included.
console.log('[side-effectful] ran');
