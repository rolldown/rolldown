# Reason
1. sub optimal
2. esbuild will reuse `ns` variable
# Diff
## /out.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports) {
    exports.x = 123;
  }
});

// entry.js
var ns = __toESM(require_foo());
console.log(ns.foo);
```
### rolldown
```js
import assert from "node:assert";


//#region foo.js
var import_foo;
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.x = 123;
	import_foo = __toESM(require_foo());
} });

//#endregion
//#region entry.js
require_foo();
assert.equal(import_foo.foo, undefined);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,9 @@
+var import_foo;
 var require_foo = __commonJS({
     "foo.js"(exports) {
         exports.x = 123;
+        import_foo = __toESM(require_foo());
     }
 });
-var ns = __toESM(require_foo());
-console.log(ns.foo);
+require_foo();
+console.log(import_foo.foo);

```