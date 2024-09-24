## /out.js
### esbuild
```js
// entry.js
var foo = 123;
export {
  foo
};
```
### rolldown
```js

//#region entry.js
const foo = 123;

//#endregion
export { foo };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs

```
