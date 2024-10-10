<<<<<<< HEAD
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
import { default as assert } from "node:assert";


//#region node_modules/pkg/foo.js
var require_foo = __commonJS({ "node_modules/pkg/foo.js"(exports, module) {
	module.exports = 123;
} });

//#endregion
//#region node_modules/pkg/index.js
var import_foo = __toESM(require_foo());

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
+++ rolldown	entry_js.js
@@ -1,6 +1,6 @@
 var require_foo = __commonJS({
-    "Users/user/project/node_modules/pkg/foo.js"(exports, module) {
+    "node_modules/pkg/foo.js"(exports, module) {
         module.exports = 123;
     }
 });
 var import_foo = __toESM(require_foo());

```