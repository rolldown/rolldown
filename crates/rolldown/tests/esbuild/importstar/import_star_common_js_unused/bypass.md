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
    exports.foo = 123;
  }
});

// entry.js
var ns = __toESM(require_foo());
var foo = 234;
console.log(foo);
```
### rolldown
```js
import assert from "node:assert";

//#region entry.js
let foo = 234;
assert.equal(foo, 234);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,2 @@
-var require_foo = __commonJS({
-    "foo.js"(exports) {
-        exports.foo = 123;
-    }
-});
-var ns = __toESM(require_foo());
 var foo = 234;
 console.log(foo);

```