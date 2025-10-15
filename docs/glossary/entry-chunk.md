# Entry Chunk

An entry chunk is created, because we need to output a JavaScript file for:

- Exporting the exports of the entry module
- Representing the executing point of the corresponding [entry](./entry.md).
- Storing the code of the entry module and its dependencies (if not code-split into separate chunks)

Let's say you have an app that could run separately but also could be used as a library by other apps.

File structure:

```js
// component.js
export function component() {
  return 'Hello World';
}

// render.js
export function render(component) {
  console.log(component());
}

// app.js
import { component } from './component.js';
import { render } from './render.js';

render(component);

// lib.js
export { component } from './component.js';
```

Config:

```js
export default defineConfig({
  input: {
    app: './app.js',
    lib: './lib.js',
  },
});
```

Rolldown will create outputs like:

::: code-group

```js [app.js]
import { component } from './common.js';

function render(component) {
  console.log(component());
}

render(component);
```

```js [lib.js]
export { component } from './common.js';
```

```js [common.js]
export function component() {
  return 'Hello World';
}
```

:::

- `lib.js` is created because we need to create the export signature `export { component }` and export it in `lib.js`.
- For `app.js`, though it doesn't export anything, we still need to create `app.js` as the executing point of the app.
- You'll also notice, from the executing point `app.js`, only modules, like `render.js`, imported are executed. This is another reason and promise made by being the executing point.
