import * as foo from './foo.js'
input.works =
  foo[Math.random() < 1 && 'default'] === void 0 &&
  foo.bar === 123