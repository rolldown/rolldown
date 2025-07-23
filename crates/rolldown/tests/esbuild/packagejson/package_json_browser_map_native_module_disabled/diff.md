# Diff
## /Users/user/project/out.js
### esbuild
```js
// (disabled):fs
var require_fs = __commonJS({
  "(disabled):fs"() {
  }
});

// Users/user/project/node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({
  "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
    var fs = require_fs();
    module.exports = function() {
      return fs.readFile();
    };
  }
});

// Users/user/project/src/entry.js
var import_demo_pkg = __toESM(require_demo_pkg());
console.log((0, import_demo_pkg.default)());
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region (ignored) node_modules/demo-pkg
var require_demo_pkg$1 = /* @__PURE__ */ __commonJS({ "node_modules/demo-pkg"() {} });

//#endregion
//#region node_modules/demo-pkg/index.js
var require_demo_pkg = /* @__PURE__ */ __commonJS({ "node_modules/demo-pkg/index.js"(exports, module) {
	const fs = require_demo_pkg$1();
	module.exports = function() {
		return fs.readFile();
	};
} });

//#endregion
//#region src/entry.js
var import_demo_pkg = __toESM(require_demo_pkg());
console.log((0, import_demo_pkg.default)());

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,10 +1,10 @@
-var require_fs = __commonJS({
-    "(disabled):fs"() {}
+var require_demo_pkg$1 = __commonJS({
+    "node_modules/demo-pkg"() {}
 });
 var require_demo_pkg = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
-        var fs = require_fs();
+    "node_modules/demo-pkg/index.js"(exports, module) {
+        const fs = require_demo_pkg$1();
         module.exports = function () {
             return fs.readFile();
         };
     }

```