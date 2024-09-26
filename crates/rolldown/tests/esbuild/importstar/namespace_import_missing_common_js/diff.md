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
import { default as assert } from "node:assert";


//#region foo.js
var require_foo = __commonJSMin((exports) => {
	exports.x = 123;
});

//#endregion
//#region entry.js
var import_foo = __toESM(require_foo());
assert.deepEqual(import_foo, {
	default: { x: 123 },
	x: 123
});
assert.equal(import_foo.foo, undefined);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,5 +1,5 @@
 var require_foo = __commonJSMin(exports => {
     exports.x = 123;
 });
-var ns = __toESM(require_foo());
-console.log(ns, ns.foo);
\ No newline at end of file
+var import_foo = __toESM(require_foo());
+console.log(import_foo, import_foo.foo);
\ No newline at end of file

```
