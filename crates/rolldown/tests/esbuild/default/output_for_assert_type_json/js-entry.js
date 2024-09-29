import all from './foo.json' assert { type: 'json' }
import copy from './foo.copy' assert { type: 'json' }
import { default as def } from './foo.json' assert { type: 'json' }
import * as ns from './foo.json' assert { type: 'json' }
use(all, copy, def, ns.prop)
export { default } from './foo.json' assert { type: 'json' }