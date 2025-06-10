import * as file from './foo.mjs'
import * as dir from './foo/index.mjs'

console.log(file.foo, dir.foo)

import.meta.hot.accept()
