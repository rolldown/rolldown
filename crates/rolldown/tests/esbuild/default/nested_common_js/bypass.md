# Reason
1. we use assert rather than console
# Diff
## /out.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports, module) {
    module.exports = function() {
      return 123;
    };
  }
});

// entry.js
function nestedScope() {
  const fn = require_foo();
  console.log(fn());
}
nestedScope();
```
### rolldown
```js
import assert from "node:assert";

// HIDDEN [rolldown:runtime]
//#region foo.js
var require_foo = /* @__PURE__ */ __commonJS({ "foo.js"(exports, module) {
	module.exports = function() {
		return 123;
	};
} });

//#endregion
//#region entry.js
function nestedScope() {
	const fn = require_foo();
	assert.equal(fn(), 123);
}
nestedScope();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -6,7 +6,7 @@
     }
 });
 function nestedScope() {
     const fn = require_foo();
-    console.log(fn());
+    assert.equal(fn(), 123);
 }
 nestedScope();

```