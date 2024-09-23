## /out.js
### esbuild
```js
// entry.js
var entry_exports = {};
__export(entry_exports, {
  foo: () => foo,
  ns: () => entry_exports
});
var foo = 123;
export {
  foo,
  entry_exports as ns
};
```
### rolldown
```js


//#region entry.js
var entry_exports = {};
__export(entry_exports, {
	foo: () => foo,
	ns: () => entry_exports
});
const foo = 123;

//#endregion
export { foo, entry_exports as ns };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -2,9 +2,9 @@
 __export(entry_exports, {
     foo: () => foo,
     ns: () => entry_exports
 });
-var foo = 123;
+const foo = 123;
 export {
     foo,
     entry_exports as ns
 };
\ No newline at end of file

```
