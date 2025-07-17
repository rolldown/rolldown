# Diff
## /out.js
### esbuild
```js
// foo.js
var foo_exports = {};
__export(foo_exports, {
  foo: () => foo
});
var foo;
var init_foo = __esm({
  "foo.js"() {
    foo = 123;
  }
});

// entry.js
init_foo();
var ns2 = (init_foo(), __toCommonJS(foo_exports));
console.log(foo, ns2.foo);
```
### rolldown
```js
import assert from "node:assert";

// HIDDEN [rolldown:runtime]
//#region foo.js
var foo_exports = {};
__export(foo_exports, { foo: () => foo });
var foo;
var init_foo = __esm({ "foo.js": (() => {
	foo = 123;
}) });

//#endregion
//#region entry.js
init_foo();
const ns2 = (init_foo(), __toCommonJS(foo_exports));
assert.equal(foo, 123);
assert.equal(ns2.foo, 123);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -3,9 +3,9 @@
     foo: () => foo
 });
 var foo;
 var init_foo = __esm({
-    "foo.js"() {
+    "foo.js": () => {
         foo = 123;
     }
 });
 init_foo();

```