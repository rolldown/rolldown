const foo = require('./foo.js')
input.works = foo.baz === 123 &&
  foo[Math.random() < 1 && '__esModule'] === true