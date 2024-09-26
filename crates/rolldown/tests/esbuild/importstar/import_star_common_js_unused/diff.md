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
import { default as assert } from "node:assert";


//#region foo.js
var require_foo = __commonJSMin((exports) => {
	exports.foo = 123;
});

//#endregion
//#region entry.js
var import_foo = __toESM(require_foo());
let foo = 234;
assert.equal(foo, 234);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,6 +1,6 @@
 var require_foo = __commonJSMin(exports => {
     exports.foo = 123;
 });
-var ns = __toESM(require_foo());
+var import_foo = __toESM(require_foo());
 var foo = 234;
 console.log(foo);
\ No newline at end of file

```
