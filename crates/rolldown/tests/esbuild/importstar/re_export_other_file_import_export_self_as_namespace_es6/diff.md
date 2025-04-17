# Diff
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

//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
};

//#region foo.js
var foo_exports = {};
__export(foo_exports, {
	foo: () => foo,
	ns: () => foo_exports
});
const foo = 123;

export { foo, foo_exports as ns };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,11 @@
+var __defProp = Object.defineProperty;
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
 var foo_exports = {};
 __export(foo_exports, {
     foo: () => foo,
     ns: () => foo_exports

```