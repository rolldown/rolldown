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

// HIDDEN [rolldown:runtime]
//#region foo.js
var require_foo = /* @__PURE__ */ __commonJS({ "foo.js"(exports) {
	exports.fn = function() {
		return 123;
	};
} });

//#endregion
//#region entry.js
var import_foo = __toESM(require_foo());
assert.equal((0, import_foo.fn)(), 123);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -5,7 +5,5 @@
         };
     }
 });
 var import_foo = __toESM(require_foo());
-(() => {
-    console.log((0, import_foo.fn)());
-})();
+console.log((0, import_foo.fn)());

```