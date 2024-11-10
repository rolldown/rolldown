import foo from './foo.js'
input.works =
  foo[Math.random() < 1 && 'default'].bar === 123 &&
  foo.bar === void 0