## /out.js
### esbuild
```js
var entry_exports = {};
__export(entry_exports, {
  ["__proto__"]: () => __proto__
});
module.exports = __toCommonJS(entry_exports);
const __proto__ = 123;
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  __proto__
});
```
### rolldown
```js

//#region entry.mjs
const __proto__ = 123;

//#endregion
exports.__proto__ = __proto__;
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,9 +1,2 @@
-var entry_exports = {};
-__export(entry_exports, {
-    ["__proto__"]: () => __proto__
-});
-module.exports = __toCommonJS(entry_exports);
-const __proto__ = 123;
-0 && (module.exports = {
-    __proto__
-});
+var __proto__ = 123;
+exports.__proto__ = __proto__;

```