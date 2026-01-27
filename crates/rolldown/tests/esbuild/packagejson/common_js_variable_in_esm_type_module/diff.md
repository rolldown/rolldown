## /out.js
### esbuild
```js
// entry.js
module.exports = null;
```
### rolldown
```js
// HIDDEN [\0rolldown/runtime.js]
//#region entry.js
var require_entry = /* @__PURE__ */ __commonJSMin(((exports, module) => {
	module.exports = null;
}));

//#endregion
export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,1 +1,4 @@
-module.exports = null;
+var require_entry = __commonJSMin((exports, module) => {
+    module.exports = null;
+});
+export default require_entry();

```