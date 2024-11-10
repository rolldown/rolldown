const foo = require('./foo.js')
input.works =
  foo[Math.random() < 1 && 'default'] === 123 &&
  foo[Math.random() < 1 && '__esModule'] === true