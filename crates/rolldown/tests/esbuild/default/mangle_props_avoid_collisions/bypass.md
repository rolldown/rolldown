# Reason
1. could be done in minifier
# Diff
## /out.js
### esbuild
```js
export default {
  c: 0,
  // Must not be named "a"
  d: 1,
  // Must not be named "b"
  a: 2,
  b: 3,
  __proto__: {}
  // Always avoid mangling this
};
```
### rolldown
```js

//#region entry.js
var entry_default = {
	foo_: 0,
	bar_: 1,
	a: 2,
	b: 3,
	__proto__: {}
};
//#endregion

export { entry_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,8 @@
-export default {
-    c: 0,
-    d: 1,
+var entry_default = {
+    foo_: 0,
+    bar_: 1,
     a: 2,
     b: 3,
     __proto__: {}
 };
+export {entry_default as default};

```