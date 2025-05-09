# Reason
1. different naming style
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
import assert from "node:assert";


//#region index.js
var require_dot_import = __commonJS({ "index.js"(exports) {
	exports.x = 123;
} });

//#endregion
//#region entry.js
var import_dot_import = __toESM(require_dot_import());
assert.equal(import_dot_import.x, 123);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,7 @@
-var require_index = __commonJS({
+var require_dot_import = __commonJS({
     "index.js"(exports) {
         exports.x = 123;
     }
 });
-var import__ = __toESM(require_index());
-console.log(import__.x);
+var import_dot_import = __toESM(require_dot_import());
+console.log(import_dot_import.x);

```