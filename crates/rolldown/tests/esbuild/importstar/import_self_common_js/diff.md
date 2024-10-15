# Reason 
1. Format cjs should not appear `export`
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

const { default: assert } = __toESM(require("node:assert"));

//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports) {
	var import_entry = __toESM(require_entry());
	exports.foo = 123;
	assert.equal(import_entry.foo, undefined);
} });

//#endregion
export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,9 @@
+var {default: assert} = __toESM(require("node:assert"));
 var require_entry = __commonJS({
     "entry.js"(exports) {
         var import_entry = __toESM(require_entry());
         exports.foo = 123;
-        console.log(import_entry.foo);
+        assert.equal(import_entry.foo, undefined);
     }
 });
-module.exports = require_entry();
+export default require_entry();

```