const foo = require('./foo.js')
import * as foo2 from './foo.js'
input.works = import('./foo.js').then(foo3 =>
  foo.bar === 123 && foo.__esModule === true &&
  foo2.bar === 123 && foo2.__esModule === void 0 &&
  foo3.bar === 123 && foo3.__esModule === void 0)