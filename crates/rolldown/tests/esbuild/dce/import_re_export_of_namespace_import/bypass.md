# Reason
1. different naming style
# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/pkg/foo.js
var require_foo = __commonJS({
  "Users/user/project/node_modules/pkg/foo.js"(exports, module) {
    module.exports = 123;
  }
});

// Users/user/project/node_modules/pkg/index.js
var import_foo = __toESM(require_foo());

// Users/user/project/entry.js
console.log(import_foo.default);
```
### rolldown
```js
import assert from "node:assert";


//#region node_modules/pkg/foo.js
var import_foo;
var require_foo = __commonJS({ "node_modules/pkg/foo.js"(exports, module) {
	module.exports = 123;
	import_foo = __toESM(require_foo());
} });

//#endregion
//#region node_modules/pkg/bar.js
var require_bar = __commonJS({ "node_modules/pkg/bar.js"(exports, module) {
	module.exports = "abc";
} });

//#endregion
//#region node_modules/pkg/index.js
require_foo();
require_bar();

//#endregion
//#region entry.js
assert.equal(
	// => const import_xxx = require_xxx
	import_foo.default,
	123
);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,15 @@
+var import_foo;
 var require_foo = __commonJS({
-    "Users/user/project/node_modules/pkg/foo.js"(exports, module) {
+    "node_modules/pkg/foo.js"(exports, module) {
         module.exports = 123;
+        import_foo = __toESM(require_foo());
     }
 });
-var import_foo = __toESM(require_foo());
+var require_bar = __commonJS({
+    "node_modules/pkg/bar.js"(exports, module) {
+        module.exports = "abc";
+    }
+});
+require_foo();
+require_bar();
 console.log(import_foo.default);

```