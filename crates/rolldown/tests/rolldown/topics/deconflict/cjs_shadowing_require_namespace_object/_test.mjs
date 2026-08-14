import { strict as assert } from 'node:assert';

// Regression test for the #9882 require()/namespace-object shadowing variant. The CJS entry's
// local `var lib_exports` collided with the dependency's chunk-root namespace object
// `lib_exports`, turning the require initializer into `__toCommonJS(undefined)` and throwing at
// module-eval time. After the fix the local is deconflicted and the bundle evaluates cleanly.
const mod = await import('./dist/main.js');

assert.equal(mod.default, 'lib-default:lib-named');
