# Reason
1. we use assert rather than console
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
import assert from "node:assert";


//#region foo.js
var import_foo;
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.fn = function() {
		return 123;
	};
	import_foo = __toESM(require_foo());
} });

//#endregion
//#region entry.js
require_foo();
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
@@ -1,11 +1,13 @@
+var import_foo;
 var require_foo = __commonJS({
     "foo.js"(exports) {
         exports.fn = function () {
             return 123;
         };
+        import_foo = __toESM(require_foo());
     }
 });
-var import_foo = __toESM(require_foo());
+require_foo();
 (() => {
-    console.log((0, import_foo.fn)());
+    assert.equal((0, import_foo.fn)(), 123);
 })();

```