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
var foo_ns = {};
__export(foo_ns, {
	foo: () => foo,
	ns: () => foo_ns
});
const foo = 123;

//#endregion
export { foo, foo_ns as ns };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,10 +1,10 @@
-var foo_exports = {};
-__export(foo_exports, {
+var foo_ns = {};
+__export(foo_ns, {
     foo: () => foo,
-    ns: () => foo_exports
+    ns: () => foo_ns
 });
-var foo = 123;
+const foo = 123;
 export {
     foo,
-    foo_exports as ns
+    foo_ns as ns
 };
\ No newline at end of file

```
