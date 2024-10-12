# Diff
## /out.js
### esbuild
```js
(() => {
  // shims.js
  var import_process;
  var init_shims = __esm({
    "shims.js"() {
      import_process = __toESM(__require("process"));
    }
  });

  // cjs.js
  var require_cjs = __commonJS({
    "cjs.js"(exports) {
      "use strict";
      init_shims();
      exports.foo = import_process.default;
    }
  });

  // entry.js
  init_shims();
  console.log(require_cjs());
})();
```
### rolldown
```js
import { default as assert } from "node:assert";


//#region cjs.js
var require_cjs = __commonJS({ "cjs.js"(exports) {
	exports.foo = process;
} });

//#endregion
//#region entry.js
assert.deepEqual(require_cjs(), { foo: process });

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,17 +1,6 @@
-(() => {
-    var import_process;
-    var init_shims = __esm({
-        "shims.js"() {
-            import_process = __toESM(__require("process"));
-        }
-    });
-    var require_cjs = __commonJS({
-        "cjs.js"(exports) {
-            "use strict";
-            init_shims();
-            exports.foo = import_process.default;
-        }
-    });
-    init_shims();
-    console.log(require_cjs());
-})();
+var require_cjs = __commonJS({
+    "cjs.js"(exports) {
+        exports.foo = process;
+    }
+});
+console.log(require_cjs());

```