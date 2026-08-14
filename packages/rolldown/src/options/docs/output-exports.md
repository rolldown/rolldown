When `'auto'` is used, Rolldown will automatically determine the export mode based on the exports of the `input` modules. If the `input` modules have a single default export, then `'default'` mode is used. If the `input` modules have named exports, then `'named'` mode is used. If there are no exports, then `'none'` mode is used.

`'default'` can only be used when the `input` modules have a single default export. `'none'` can only be used when the `input` modules have no exports. Otherwise, Rolldown will throw an error.

The difference between `'default'` and `'named'` affects how other people can consume your bundle. If you use `'default'`, a CommonJS user could do this, for example:

```js
// your-lib package entry
export default 'Hello world';

// a CommonJS consumer
/* require( "your-lib" ) returns "Hello world" */
const hello = require('your-lib');
```

With `'named'`, a user would do this instead:

```js
// your-lib package entry
export const hello = 'Hello world';

// a CommonJS consumer
/* require( "your-lib" ) returns {hello: "Hello world"} */
const hello = require('your-lib').hello;
/* or using destructuring */
const { hello } = require('your-lib');
```

The wrinkle is that if you use `'named'` exports but also have a default export, a user would have to do something like this to use the default export:

```js
// your-lib package entry
export default 'foo';
export const bar = 'bar';

// a CommonJS consumer
/* require( "your-lib" ) returns {default: "foo", bar: "bar"} */
const foo = require('your-lib').default;
const bar = require('your-lib').bar;
/* or using destructuring */
const { default: foo, bar } = require('your-lib');
```

::: tip

There are many tools that are capable of resolving a CommonJS `require(...)` call with an ES module. If you are generating CommonJS output that is meant to be interchangeable with ESM output for those tools, you should always use `'named'` export mode. The reason is that most of those tools will by default return the namespace of an ES module on `require` where the default export is the `.default` property.

In other words for those tools, you cannot create a package interface where `const lib = require("your-lib")` yields the same as `import lib from "your-lib"`. With `'named'` export mode however, `const {lib} = require("your-lib")` will be equivalent to `import {lib} from "your-lib"`.

:::
