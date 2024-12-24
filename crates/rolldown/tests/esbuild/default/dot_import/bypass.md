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
var import_dot_import;
var require_dot_import = __commonJS({ "index.js"(exports) {
	exports.x = 123;
	import_dot_import = __toESM(require_dot_import());
} });

//#endregion
//#region entry.js
require_dot_import();
assert.equal(import_dot_import.x, 123);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,9 @@
-var require_index = __commonJS({
+var import_dot_import;
+var require_dot_import = __commonJS({
     "index.js"(exports) {
         exports.x = 123;
+        import_dot_import = __toESM(require_dot_import());
     }
 });
-var import__ = __toESM(require_index());
-console.log(import__.x);
+require_dot_import();
+console.log(import_dot_import.x);

```