import foo from './commonjs.js'
import esm, { esm_named_var, esm_named_fn, esm_named_class } from './esm.js'
console.log(foo, esm, esm_named_var, esm_named_fn, esm_named_class)
// test commonjs warp symbol deconflict
const require_commonjs = () => { }
// test esm export function symbol deconflict
function esm_default_fn() { }
console.log(require_commonjs, esm_default_fn)
