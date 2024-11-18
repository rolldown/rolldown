import foo from './foo.cjs'
input.works =
  foo[Math.random() < 1 && 'default'].bar === 123