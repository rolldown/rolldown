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


//#region foo.js
var foo_exports = {};
__export(foo_exports, { foo: () => foo });
const foo = 123;
var init_foo = __esm({ "foo.js"() {} });

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
@@ -1,13 +1,11 @@
 var foo_exports = {};
 __export(foo_exports, {
     foo: () => foo
 });
-var foo;
+var foo = 123;
 var init_foo = __esm({
-    "foo.js"() {
-        foo = 123;
-    }
+    "foo.js"() {}
 });
 init_foo();
 var ns2 = (init_foo(), __toCommonJS(foo_exports));
 console.log(foo, ns2.foo);

```