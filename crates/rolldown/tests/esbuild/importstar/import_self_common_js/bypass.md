# Reason 
1. We rewrite `console.log` to assertions.
# Diff
## /out.js
### esbuild
```js
// entry.js
var require_entry = __commonJS({
  "entry.js"(exports) {
    var import_entry = __toESM(require_entry());
    exports.foo = 123;
    console.log(import_entry.foo);
  }
});
module.exports = require_entry();
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
const node_assert = __toESM(require("node:assert"));

//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports) {
	var import_entry = __toESM(require_entry());
	exports.foo = 123;
	node_assert.default.equal(import_entry.foo, void 0);
} });

//#endregion
module.exports = require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,9 @@
+var node_assert = __toESM(require("node:assert"));
 var require_entry = __commonJS({
     "entry.js"(exports) {
         var import_entry = __toESM(require_entry());
         exports.foo = 123;
-        console.log(import_entry.foo);
+        node_assert.default.equal(import_entry.foo, void 0);
     }
 });
 module.exports = require_entry();

```