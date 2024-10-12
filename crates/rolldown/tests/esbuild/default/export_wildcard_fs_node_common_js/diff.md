# Diff
## /out.js
### esbuild
```js
// entry.js
var entry_exports = {};
__export(entry_exports, {
  foo: () => foo
});
module.exports = __toCommonJS(entry_exports);
__reExport(entry_exports, require("fs"), module.exports);

// internal.js
var foo = 123;

// entry.js
__reExport(entry_exports, require("./external"), module.exports);
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  foo,
  ...require("fs"),
  ...require("./external")
});
```
### rolldown
```js
import "fs";
import "./external";

export * from "fs"

export * from "./external"

//#region internal.js
let foo = 123;

//#endregion
export { foo };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,13 +1,6 @@
-var entry_exports = {};
-__export(entry_exports, {
-    foo: () => foo
-});
-module.exports = __toCommonJS(entry_exports);
-__reExport(entry_exports, require("fs"), module.exports);
+import "fs";
+import "./external";
+export * from "fs";
+export * from "./external";
 var foo = 123;
-__reExport(entry_exports, require("./external"), module.exports);
-0 && (module.exports = {
-    foo,
-    ...require("fs"),
-    ...require("./external")
-});
+export {foo};

```