# Reason
1. different fs
2. different naming style
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/lib/util-browser.js
var require_util_browser = __commonJS({
  "Users/user/project/node_modules/demo-pkg/lib/util-browser.js"(exports, module) {
    module.exports = "util-browser";
  }
});

// Users/user/project/node_modules/demo-pkg/main-browser.js
var require_main_browser = __commonJS({
  "Users/user/project/node_modules/demo-pkg/main-browser.js"(exports, module) {
    var util = require_util_browser();
    module.exports = function() {
      return ["main-browser", util];
    };
  }
});

// Users/user/project/src/entry.js
var import_demo_pkg = __toESM(require_main_browser());
console.log((0, import_demo_pkg.default)());
```
### rolldown
```js
import assert from "node:assert";



//#region node_modules/demo-pkg/lib/util-browser.js
var require_util_browser = __commonJS({ "node_modules/demo-pkg/lib/util-browser.js"(exports, module) {
	module.exports = "util-browser";
} });
//#endregion

//#region node_modules/demo-pkg/main-browser.js
var require_main_browser = __commonJS({ "node_modules/demo-pkg/main-browser.js"(exports, module) {
	const util = require_util_browser();
	module.exports = function() {
		return ["main-browser", util];
	};
} });
var import_main_browser = __toESM(require_main_browser());
//#endregion

//#region src/entry.js
assert.deepEqual((0, import_main_browser.default)(), ["main-browser", "util-browser"]);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,15 +1,15 @@
 var require_util_browser = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/lib/util-browser.js"(exports, module) {
+    "node_modules/demo-pkg/lib/util-browser.js"(exports, module) {
         module.exports = "util-browser";
     }
 });
 var require_main_browser = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main-browser.js"(exports, module) {
-        var util = require_util_browser();
+    "node_modules/demo-pkg/main-browser.js"(exports, module) {
+        const util = require_util_browser();
         module.exports = function () {
             return ["main-browser", util];
         };
     }
 });
-var import_demo_pkg = __toESM(require_main_browser());
-console.log((0, import_demo_pkg.default)());
+var import_main_browser = __toESM(require_main_browser());
+console.log((0, import_main_browser.default)());

```