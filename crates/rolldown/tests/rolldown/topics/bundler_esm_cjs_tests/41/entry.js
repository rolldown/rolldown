import * as foo from './foo.js'
input.works =
  foo[Math.random() < 1 && '__esModule'] === true &&
  foo[Math.random() < 1 && 'default'].bar === 123