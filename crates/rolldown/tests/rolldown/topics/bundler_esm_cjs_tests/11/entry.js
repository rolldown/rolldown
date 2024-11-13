import * as foo from './foo.js'
input.works =
  foo[Math.random() < 1 && 'default'].default.bar === 123