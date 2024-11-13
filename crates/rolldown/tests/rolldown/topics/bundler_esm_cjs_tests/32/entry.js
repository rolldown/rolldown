input.works = import('./foo.js')
  .then(foo => foo.default === 123 &&
    foo.__esModule === void 0)