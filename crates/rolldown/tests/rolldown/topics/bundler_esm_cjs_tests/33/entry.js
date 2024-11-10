input.works = import('./foo.js')
  .then(foo =>
    foo[Math.random() < 1 && 'default'] === 123 &&
    foo[Math.random() < 1 && '__esModule'] === void 0)