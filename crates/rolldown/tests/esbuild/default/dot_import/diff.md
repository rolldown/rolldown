# Diff
## /out.js
### esbuild
```js
// index.js
var require_index = __commonJS({
  "index.js"(exports) {
    exports.x = 123;
  }
});

// entry.js
var import__ = __toESM(require_index());
console.log(import__.x);
```
### rolldown
```js
import { default as assert } from "node:assert";


//#region index.js
var require_dot_import_index = __commonJS({ "index.js"(exports) {
	exports.x = 123;
} });

//#endregion
//#region entry.js
var import_dot_import_index = __toESM(require_dot_import_index());
assert(import_dot_import_index.x === 123);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,7 @@
-var require_index = __commonJS({
+var require_dot_import_index = __commonJS({
     "index.js"(exports) {
         exports.x = 123;
     }
 });
-var import__ = __toESM(require_index());
-console.log(import__.x);
+var import_dot_import_index = __toESM(require_dot_import_index());
+assert(import_dot_import_index.x === 123);

```