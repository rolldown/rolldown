# Reason
1. could be done in minifier
# Diff
## /out.js
### esbuild
```js
export default {
  a: 0,
  _bar_: 1
};
```
### rolldown
```js
//#region entry.js
var entry_default = {
	foo_: 0,
	_bar_: 1
};

//#endregion
export { entry_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,5 @@
-export default {
-    a: 0,
+var entry_default = {
+    foo_: 0,
     _bar_: 1
 };
+export {entry_default as default};

```