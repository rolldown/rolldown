import all from './foo.json' assert { type: 'json' }
import { default as def } from './foo.json' assert { type: 'json' }
import { unused } from './foo.json' assert { type: 'json' }
import { used } from './foo.json' assert { type: 'json' }
import * as ns from './foo.json' assert { type: 'json' }
use(used, ns.prop)
export { exported } from './foo.json' assert { type: 'json' }
export { default as def2 } from './foo.json' assert { type: 'json' }
export { def3 as default } from './foo.json' assert { type: 'json' }
import text from './foo.text' assert { type: 'json' }
import file from './foo.file' assert { type: 'json' }
import copy from './foo.copy' assert { type: 'json' }