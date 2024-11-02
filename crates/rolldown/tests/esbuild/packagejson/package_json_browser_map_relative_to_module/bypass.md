# Reason
1. different fs
2. trivial naming difference
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/util-browser/index.js
var require_util_browser = __commonJS({
  "Users/user/project/node_modules/util-browser/index.js"(exports, module) {
    module.exports = "util-browser";
  }
});

// Users/user/project/node_modules/demo-pkg/main.js
var require_main = __commonJS({
  "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
    var util = require_util_browser();
    module.exports = function() {
      return ["main", util];
    };
  }
});

// Users/user/project/src/entry.js
var import_demo_pkg = __toESM(require_main());
console.log((0, import_demo_pkg.default)());
```
### rolldown
```js
import assert from "node:assert";


//#region node_modules/util-browser/index.js
var require_util_browser = __commonJS({ "node_modules/util-browser/index.js"(exports, module) {
	module.exports = "util-browser";
} });

//#endregion
//#region node_modules/demo-pkg/main.js
var require_main = __commonJS({ "node_modules/demo-pkg/main.js"(exports, module) {
	const util = require_util_browser();
	module.exports = function() {
		return ["main", util];
	};
} });

//#endregion
//#region src/entry.js
var import_main = __toESM(require_main());
assert.deepEqual((0, import_main.default)(), ["main", "util-browser"]);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,15 +1,15 @@
 var require_util_browser = __commonJS({
-    "Users/user/project/node_modules/util-browser/index.js"(exports, module) {
+    "node_modules/util-browser/index.js"(exports, module) {
         module.exports = "util-browser";
     }
 });
 var require_main = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
-        var util = require_util_browser();
+    "node_modules/demo-pkg/main.js"(exports, module) {
+        const util = require_util_browser();
         module.exports = function () {
             return ["main", util];
         };
     }
 });
-var import_demo_pkg = __toESM(require_main());
-console.log((0, import_demo_pkg.default)());
+var import_main = __toESM(require_main());
+console.log((0, import_main.default)());

```