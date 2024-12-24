# Reason
1. sub optimal
2. esbuild try to reuse `ns` variable, we always create a new one
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
var foo2 = 234;
console.log(ns, ns.foo, foo2);
```
### rolldown
```js
import assert from "node:assert";


//#region foo.js
var import_foo;
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.foo = 123;
	import_foo = __toESM(require_foo());
} });

//#endregion
//#region entry.js
require_foo();
let foo = 234;
assert.deepEqual(import_foo, {
	default: { foo: 123 },
	foo: 123
});
assert.equal(import_foo.foo, 123);
assert.equal(foo, 234);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,10 @@
+var import_foo;
 var require_foo = __commonJS({
     "foo.js"(exports) {
         exports.foo = 123;
+        import_foo = __toESM(require_foo());
     }
 });
-var ns = __toESM(require_foo());
-var foo2 = 234;
-console.log(ns, ns.foo, foo2);
+require_foo();
+var foo = 234;
+console.log(import_foo, import_foo.foo, foo);

```