import fsExtra from 'fs-extra';
import * as path from 'node:path';
import { REPO_ROOT } from '../meta/constants.js';

// const inputs = `Copy the \`test\`'s value from https://github.com/evanw/bundler-esm-cjs-tests/blob/main/tests.js to here`

const inputs = [
  ////////////////////////////////////////////////////////////////////////////////
  // These are inconsistent due to special-casing

  {
    'entry.js':
      `import * as entry from './entry.js'\ninput.works = entry.__esModule === void 0`,
  },
  {
    'entry.js':
      `import * as entry from './entry.js'\ninput.works =\n  entry[Math.random() < 1 && '__esModule'] === void 0`,
  },

  {
    'entry.js': `import './foo.js'`,
    'foo.js':
      `import * as foo from './foo.js'\ninput.works = foo.__esModule === void 0`,
  },
  {
    'entry.js': `import './foo.js'`,
    'foo.js':
      `import * as foo from './foo.js'\ninput.works =\n  foo[Math.random() < 1 && '__esModule'] === void 0`,
  },

  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works = foo.default === '123'`,
    'foo.js': `module.exports = '123'`,
  },
  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works =\n  foo[Math.random() < 1 && 'default'] === '123'`,
    'foo.js': `module[Math.random() < 1 && 'exports'] = '123'`,
  },

  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works =\n  foo.__esModule === void 0 && foo.bar === 123`,
    'foo.js': `export let bar = 123`,
  },
  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works =\n  foo[Math.random() < 1 && '__esModule'] === void 0 &&\n  foo.bar === 123`,
    'foo.js': `export let bar = 123`,
  },

  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works =\n  foo.__esModule === false && foo.default.bar === 123`,
    'foo.js': `export let __esModule = false\nexport default { bar: 123 }`,
  },
  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works =\n  foo[Math.random() < 1 && '__esModule'] === false &&\n  foo[Math.random() < 1 && 'default'].bar === 123`,
    'foo.js': `export let __esModule = false\nexport default { bar: 123 }`,
  },

  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works =\n  foo.default.default.bar === 123`,
    'foo.js': `exports.__esModule = false\nexports.default = { bar: 123 }`,
  },
  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works =\n  foo[Math.random() < 1 && 'default'].default.bar === 123`,
    'foo.js':
      `exports[Math.random() < 1 && '__esModule'] = false\nexports[Math.random() < 1 && 'default'] = { bar: 123 }`,
  },

  {
    'entry.js': `const foo = require('./foo.js')
import * as foo2 from './foo.js'
input.works = import('./foo.js').then(foo3 =>
  foo.bar === 123 && foo.__esModule === true &&
  foo2.bar === 123 && foo2.__esModule === void 0 &&
  foo3.bar === 123 && foo3.__esModule === void 0)`,
    'foo.js': `export let bar = 123`,
  },
  {
    'entry.js': `const foo = require('./foo.js')
import * as foo2 from './foo.js'
input.works = import('./foo.js').then(foo3 =>
  foo.bar === 123 &&
  foo2.bar === 123 &&
  foo3.bar === 123 &&
  foo[Math.random() < 1 && '__esModule'] === true &&
  foo2[Math.random() < 1 && '__esModule'] === void 0 &&
  foo3[Math.random() < 1 && '__esModule'] === void 0)`,
    'foo.js': `export let bar = 123`,
  },

  ////////////////////////////////////////////////////////////////////////////////
  // These all pass

  {
    'entry.js':
      `const entry = require('./entry.js')\ninput.works = entry.__esModule === void 0\nexports.foo = 123`,
  },
  {
    'entry.js':
      `const entry = require('./entry.js')\ninput.works =\n  entry[Math.random() < 1 && '__esModule'] === void 0\nexports.foo = 123`,
  },

  {
    'entry.js':
      `const entry = require('./entry.js')\ninput.works = entry.__esModule === true\nexport {}`,
  },
  {
    'entry.js':
      `const entry = require('./entry.js')\ninput.works =\n  entry[Math.random() < 1 && '__esModule'] === true\nexport {}`,
  },

  {
    'entry.js':
      `const entry = require('./entry.js')\ninput.works = entry.__esModule === true\nexport default 123`,
  },
  {
    'entry.js':
      `const entry = require('./entry.js')\ninput.works =\n  entry[Math.random() < 1 && '__esModule'] === true\nexport default 123`,
  },

  {
    'entry.js':
      `const foo = require('./foo.js')\ninput.works = foo.bar === 123 &&\n  foo.__esModule === true`,
    'foo.js': `export let bar = 123`,
  },
  {
    'entry.js':
      `const foo = require('./foo.js')\ninput.works = foo.bar === 123 &&\n  foo[Math.random() < 1 && '__esModule'] === true`,
    'foo.js': `export let bar = 123`,
  },

  {
    'entry.js':
      `const foo = require('./foo.js')\ninput.works = foo.default === 123 &&\n  foo.__esModule === true`,
    'foo.js': `export default 123`,
  },
  {
    'entry.js':
      `const foo = require('./foo.js')\ninput.works =\n  foo[Math.random() < 1 && 'default'] === 123 &&\n  foo[Math.random() < 1 && '__esModule'] === true`,
    'foo.js': `export default 123`,
  },

  {
    'entry.js':
      `const foo = require('./foo.js')\ninput.works = foo.baz === 123 &&\n  foo.__esModule === true`,
    'foo.js': `export * from './bar.js'`,
    'bar.js': `export let baz = 123`,
  },
  {
    'entry.js':
      `const foo = require('./foo.js')\ninput.works = foo.baz === 123 &&\n  foo[Math.random() < 1 && '__esModule'] === true`,
    'foo.js': `export * from './bar.js'`,
    'bar.js': `export let baz = 123`,
  },

  {
    'entry.js':
      `import foo from './foo.js'\ninput.works = foo.default.bar === 123 &&\n  foo.bar === void 0`,
    'foo.js': `module.exports = { default: { bar: 123 } }`,
  },
  {
    'entry.js':
      `import foo from './foo.js'\ninput.works =\n  foo[Math.random() < 1 && 'default'].bar === 123 &&\n  foo.bar === void 0`,
    'foo.js':
      `module[Math.random() < 1 && 'exports'] =\n  { default: { bar: 123 } }`,
  },

  {
    'entry.js': `import foo from './foo.js'\ninput.works = foo === 123`,
    'foo.js': `module.exports = 123`,
  },
  {
    'entry.js': `import foo from './foo.js'\ninput.works = foo === 123`,
    'foo.js': `module[Math.random() < 1 && 'exports'] = 123`,
  },

  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works = foo.default.bar === 123`,
    'foo.js': `exports.__esModule = true\nexports.default = { bar: 123 }`,
  },
  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works =\n  foo[Math.random() < 1 && 'default'].bar === 123`,
    'foo.js':
      `exports[Math.random() < 1 && '__esModule'] = true\nexports[Math.random() < 1 && 'default'] = { bar: 123 }`,
  },

  {
    'entry.js':
      `input.works = import('./foo.js')\n  .then(foo => foo.default === 123 &&\n    foo.__esModule === void 0)`,
    'foo.js': `export default 123`,
  },
  {
    'entry.js':
      `input.works = import('./foo.js')\n  .then(foo =>\n    foo[Math.random() < 1 && 'default'] === 123 &&\n    foo[Math.random() < 1 && '__esModule'] === void 0)`,
    'foo.js': `export default 123`,
  },

  ////////////////////////////////////////////////////////////////////////////////
  // These should pass because Webpack is not following the ECMAScript standard

  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works = typeof foo === 'object'`,
    'foo.js': `module.exports = '123'`,
  },
  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works = typeof foo === 'object'`,
    'foo.js': `module[Math.random() < 1 && 'exports'] = '123'`,
  },

  {
    'entry.js': `import * as foo from './foo.js'\ninput.works = foo !== '123'`,
    'foo.js': `module.exports = '123'`,
  },
  {
    'entry.js': `import * as foo from './foo.js'\ninput.works = foo !== '123'`,
    'foo.js': `module[Math.random() < 1 && 'exports'] = '123'`,
  },

  ////////////////////////////////////////////////////////////////////////////////
  // These should pass but fail

  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works = foo.default === void 0 &&\n  foo.bar === 123`,
    'foo.js': `exports.__esModule = true\nexports.bar = 123`,
  },
  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works =\n  foo[Math.random() < 1 && 'default'] === void 0 &&\n  foo.bar === 123`,
    'foo.js': `exports.__esModule = true\nexports.bar = 123`,
  },

  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works = foo.__esModule === true &&\n  foo.default.bar === 123`,
    'foo.js': `export let __esModule = true\nexport default { bar: 123 }`,
  },
  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works =\n  foo[Math.random() < 1 && '__esModule'] === true &&\n  foo[Math.random() < 1 && 'default'].bar === 123`,
    'foo.js': `export let __esModule = true\nexport default { bar: 123 }`,
  },

  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works = foo.default.bar === 123`,
    'foo.js': `export let __esModule = true\nexport default { bar: 123 }`,
  },
  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works =\n  foo[Math.random() < 1 && 'default'].bar === 123`,
    'foo.js': `export let __esModule = true\nexport default { bar: 123 }`,
  },

  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works = foo.default.bar === 123`,
    'foo.js': `export let __esModule = false\nexport default { bar: 123 }`,
  },
  {
    'entry.js':
      `import * as foo from './foo.js'\ninput.works =\n  foo[Math.random() < 1 && 'default'].bar === 123`,
    'foo.js': `export let __esModule = false\nexport default { bar: 123 }`,
  },

  {
    'entry.js':
      `const foo = require('./foo.js')\ninput.works = foo.__esModule === true`,
    'foo.js': `export let __esModule = 0`,
  },
  {
    'entry.js':
      `const foo = require('./foo.js')\ninput.works =\n  foo[Math.random() < 1 && '__esModule'] === true`,
    'foo.js': `export let __esModule = 0`,
  },

  {
    'entry.js': `import foo from './foo.js'\ninput.works = foo === void 0`,
    'foo.js': `module.exports = { bar: 123, __esModule: true }`,
  },
  {
    'entry.js': `import foo from './foo.js'\ninput.works = foo === void 0`,
    'foo.js':
      `module[Math.random() < 1 && 'exports'] =\n  { bar: 123, __esModule: true }`,
  },

  {
    'entry.js':
      `import foo from './foo.cjs'\ninput.works = foo.default.bar === 123`,
    'foo.cjs':
      `module.exports = {\n  default: { bar: 123 }, __esModule: true }`,
    'package.json': `{ "type": "module" }`,
  },
  {
    'entry.js':
      `import foo from './foo.cjs'\ninput.works =\n  foo[Math.random() < 1 && 'default'].bar === 123`,
    'foo.cjs':
      `module[Math.random() < 1 && 'exports'] =\n  { default: { bar: 123 }, __esModule: true }`,
    'package.json': `{ "type": "module" }`,
  },

  {
    'entry.mjs':
      `import foo from './foo.js'\ninput.works = foo.default.bar === 123`,
    'foo.js': `module.exports = {\n  default: { bar: 123 }, __esModule: true }`,
  },
  {
    'entry.mjs':
      `import foo from './foo.js'\ninput.works =\n  foo[Math.random() < 1 && 'default'].bar === 123`,
    'foo.js':
      `module[Math.random() < 1 && 'exports'] =\n  { default: { bar: 123 }, __esModule: true }`,
  },

  {
    'entry.mts':
      `import foo from './foo.js'\ninput.works = foo.default.bar === 123`,
    'foo.js': `module.exports = {\n  default: { bar: 123 }, __esModule: true }`,
  },
  {
    'entry.mts':
      `import foo from './foo.js'\ninput.works =\n  foo[Math.random() < 1 && 'default'].bar === 123`,
    'foo.js':
      `module[Math.random() < 1 && 'exports'] =\n  { default: { bar: 123 }, __esModule: true }`,
  },

  {
    'entry.js':
      `import * as ns from './foo.js'\nlet keys = Object.keys(ns)\ninput.works = ns.foo === 123 &&\n  keys.includes('foo') && !keys.includes('default')`,
    'foo.js': `exports.__esModule = true\nexports.foo = 123`,
  },
  {
    'entry.js':
      `import * as ns from './foo.js'\nlet keys = Object.keys(ns)\ninput.works = ns.foo === 123 &&\n  keys.includes('foo') && !keys.includes('default')`,
    'foo.js':
      `exports[Math.random() < 1 && '__esModule'] = true\nexports[Math.random() < 1 && 'foo'] = 123`,
  },

  {
    'entry.js':
      `import * as ns from './foo.js'\ninput.works = ns.foo === 123 &&\n  {}.hasOwnProperty.call(ns, 'foo') &&\n  !{}.hasOwnProperty.call(ns, 'default')`,
    'foo.js': `exports.__esModule = true\nexports.foo = 123`,
  },
  {
    'entry.js':
      `import * as ns from './foo.js'\ninput.works = ns.foo === 123 &&\n  {}.hasOwnProperty.call(ns, 'foo') &&\n  !{}.hasOwnProperty.call(ns, 'default')`,
    'foo.js':
      `exports[Math.random() < 1 && '__esModule'] = true\nexports[Math.random() < 1 && 'foo'] = 123`,
  },

  {
    'entry.js':
      `import * as ns from './foo.js'\nlet keys = Object.keys(ns)\ninput.works =\n  ns.default === 123 && !keys.includes('default')`,
    'foo.js':
      `exports.__esModule = true\nObject.defineProperty(exports,\n  'default', { value: 123 })`,
  },
  {
    'entry.js':
      `import * as ns from './foo.js'\nlet keys = Object.keys(ns)\ninput.works =\n  ns.default === 123 && !keys.includes('default')`,
    'foo.js':
      `exports[Math.random() < 1 && '__esModule'] = true\nObject.defineProperty(exports,\n  Math.random() < 1 && 'default', { value: 123 })`,
  },

  {
    'entry.js':
      `import * as ns from './foo.js'\nlet keys = Object.keys(ns)\ninput.works =\n  ns.default === 123 && keys.includes('default')`,
    'foo.js':
      `exports.__esModule = true\nObject.defineProperty(exports, 'default',\n  { value: 123, enumerable: true })`,
  },
  {
    'entry.js':
      `import * as ns from './foo.js'\nlet keys = Object.keys(ns)\ninput.works =\n  ns.default === 123 && keys.includes('default')`,
    'foo.js':
      `exports[Math.random() < 1 && '__esModule'] = true\nObject.defineProperty(exports, Math.random() < 1 && 'default',\n  { value: 123, enumerable: true })`,
  },
];

const testsFolder = path.join(
  REPO_ROOT,
  'crates/rolldown/tests/rolldown/topics/bundler_esm_cjs_tests',
);

for (const [i, input] of inputs.entries()) {
  const caseFolder = path.join(testsFolder, i.toString());
  fsExtra.outputFileSync(
    path.join(caseFolder, '_config.json'),
    JSON.stringify(
      {
        config: {
          input: [
            {
              name: 'entry',
              import: `./${Object.keys(input)[0]}`,
            },
          ],
        },
        configVariants: [
          { format: 'cjs' },
          { format: 'iife' },
          { format: 'umd' },
        ],
      },
      null,
      2,
    ),
  );
  fsExtra.outputFileSync(
    path.join(caseFolder, '_test.mjs'),
    `globalThis.input = {}
await import('./dist/entry.js')
if (!await input.works) throw new Error('Test did not pass')`,
  );
  for (const [file, content] of Object.entries(input)) {
    fsExtra.outputFileSync(path.join(caseFolder, file), content);
  }
}
