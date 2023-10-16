import foo from './commonjs.js'
import esm, { esm_named_var, esm_named_fn, esm_named_class  } from './esm.js'
console.log(foo, esm, esm_named_var, esm_named_fn, esm_named_class)
const require_commonjs = () => {}