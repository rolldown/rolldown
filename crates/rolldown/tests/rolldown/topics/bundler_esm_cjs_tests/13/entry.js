const foo = require('./foo.js')
import * as foo2 from './foo.js'
input.works = import('./foo.js').then(foo3 =>
  foo.bar === 123 &&
  foo2.bar === 123 &&
  foo3.bar === 123 &&
  foo[Math.random() < 1 && '__esModule'] === true &&
  foo2[Math.random() < 1 && '__esModule'] === void 0 &&
  foo3[Math.random() < 1 && '__esModule'] === void 0)