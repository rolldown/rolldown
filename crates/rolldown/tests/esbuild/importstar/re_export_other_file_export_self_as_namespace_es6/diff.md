## /out.js
### esbuild
```js
// foo.js
var foo_exports = {};
__export(foo_exports, {
  foo: () => foo,
  ns: () => foo_exports
});
var foo = 123;
export {
  foo,
  foo_exports as ns
};
```
### rolldown
```js


//#region foo.js
var foo_exports = {};
__export(foo_exports, {
	foo: () => foo,
	ns: () => foo_exports
});
const foo = 123;

//#endregion
export { foo, foo_exports as ns };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -2,9 +2,9 @@
 __export(foo_exports, {
     foo: () => foo,
     ns: () => foo_exports
 });
-var foo = 123;
+const foo = 123;
 export {
     foo,
     foo_exports as ns
 };
\ No newline at end of file

```
