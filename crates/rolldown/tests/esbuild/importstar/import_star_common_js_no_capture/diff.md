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
console.log(ns.foo, ns.foo, foo2);
```
### rolldown
```js
import { default as assert } from "node:assert";


//#region foo.js
var require_foo = __commonJSMin((exports, module) => {
	exports.foo = 123;
});

//#endregion
//#region entry.js
var import_foo = __toESM(require_foo());
let foo = 234;
assert.equal(import_foo.foo, 123);
assert.equal(foo, 234);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,8 +1,7 @@
-var require_foo = __commonJS({
-    'foo.js'(exports) {
-        exports.foo = 123;
-    }
+var require_foo = __commonJSMin((exports, module) => {
+    exports.foo = 123;
 });
-var ns = __toESM(require_foo());
-var foo2 = 234;
-console.log(ns.foo, ns.foo, foo2);
\ No newline at end of file
+var import_foo = __toESM(require_foo());
+var foo = 234;
+console.log(import_foo.foo);
+console.log(foo);
\ No newline at end of file

```
