# Diff
## /out.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports) {
    exports.fn = function() {
      return 123;
    };
  }
});

// entry.js
var import_foo = __toESM(require_foo());
(() => {
  console.log((0, import_foo.fn)());
})();
```
### rolldown
```js
import { default as assert } from "node:assert";


//#region foo.js
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.fn = function() {
		return 123;
	};
} });

//#endregion
//#region entry.js
var import_foo = __toESM(require_foo());
(() => {
	assert.equal((0, import_foo.fn)(), 123);
})();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -6,6 +6,6 @@
     }
 });
 var import_foo = __toESM(require_foo());
 (() => {
-    console.log((0, import_foo.fn)());
+    assert.equal((0, import_foo.fn)(), 123);
 })();

```