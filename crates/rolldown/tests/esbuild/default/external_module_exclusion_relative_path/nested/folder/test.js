import foo from './foo.js'
import out from '../../../out/in-out-dir.js'
import sha256 from '../../sha256.min.js'
import config from '/api/config?a=1&b=2'
console.log(foo, out, sha256, config)