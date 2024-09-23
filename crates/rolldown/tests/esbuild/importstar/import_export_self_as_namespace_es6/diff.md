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
var entry_ns = {};
__export(entry_ns, {
	foo: () => foo,
	ns: () => entry_ns
});
const foo = 123;

//#endregion
export { foo, entry_ns as ns };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,10 +1,10 @@
-var entry_exports = {};
-__export(entry_exports, {
+var entry_ns = {};
+__export(entry_ns, {
     foo: () => foo,
-    ns: () => entry_exports
+    ns: () => entry_ns
 });
-var foo = 123;
+const foo = 123;
 export {
     foo,
-    entry_exports as ns
+    entry_ns as ns
 };
\ No newline at end of file

```
