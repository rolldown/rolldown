# Reason
1. inject path
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
(function(node_assert) {

"use strict";
const { default: assert } = node_assert;


//#region cjs.js
var require_cjs = __commonJS({ "cjs.js"(exports) {
	exports.foo = process;
} });

//#endregion
//#region entry.js
assert.deepEqual(require_cjs(), { foo: process });

//#endregion
})(node_assert);
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,17 +1,11 @@
-(() => {
-    var import_process;
-    var init_shims = __esm({
-        "shims.js"() {
-            import_process = __toESM(__require("process"));
-        }
-    });
+(function (node_assert) {
+    const {default: assert} = node_assert;
     var require_cjs = __commonJS({
         "cjs.js"(exports) {
-            "use strict";
-            init_shims();
-            exports.foo = import_process.default;
+            exports.foo = process;
         }
     });
-    init_shims();
-    console.log(require_cjs());
-})();
+    assert.deepEqual(require_cjs(), {
+        foo: process
+    });
+})(node_assert);

```