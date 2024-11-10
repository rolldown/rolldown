const foo = require('./foo.js')
input.works = foo.bar === 123 &&
  foo[Math.random() < 1 && '__esModule'] === true