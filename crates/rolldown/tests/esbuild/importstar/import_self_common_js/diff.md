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
import { default as assert } from "node:assert";


//#region entry.js
var require_entry = __commonJSMin((exports) => {
	var import_entry = __toESM(require_entry());
	exports.foo = 123;
	assert.equal(import_entry.foo, undefined);
});

//#endregion
export default require_entry();


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,6 +1,6 @@
 var require_entry = __commonJSMin(exports => {
     var import_entry = __toESM(require_entry());
     exports.foo = 123;
-    console.log(import_entry.foo);
+    assert.equal(import_entry.foo, undefined);
 });
-module.exports = require_entry();
\ No newline at end of file
+export default require_entry();
\ No newline at end of file

```
