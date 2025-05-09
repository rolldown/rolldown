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
console.log(ns, ns.foo);
```
### rolldown
```js
import assert from "node:assert";


//#region foo.js
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.x = 123;
} });

//#endregion
//#region entry.js
var import_foo = __toESM(require_foo());
assert.deepEqual(import_foo, {
	default: { x: 123 },
	x: 123
});
assert.equal(import_foo.foo, void 0);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -2,6 +2,6 @@
     "foo.js"(exports) {
         exports.x = 123;
     }
 });
-var ns = __toESM(require_foo());
-console.log(ns, ns.foo);
+var import_foo = __toESM(require_foo());
+console.log(import_foo, import_foo.foo);

```