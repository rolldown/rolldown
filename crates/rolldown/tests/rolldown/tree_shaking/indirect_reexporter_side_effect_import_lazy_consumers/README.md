# Indirect re-exporter side-effect import with lazy consumers

A package barrel that is not listed in `sideEffects` bare-imports a module that
is (`**/configure.{js,mjs}`), then forwards locally imported bindings
(`import { x } from './x.js'; export { x }`). Consumers that only reach the
barrel through dynamic imports must still execute `configure.js` exactly once,
including when small common chunks are inlined.
