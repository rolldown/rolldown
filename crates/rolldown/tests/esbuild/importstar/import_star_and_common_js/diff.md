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
import { default as assert } from "node:assert";


//#region foo.js
var foo_exports, foo;
var init_foo = __esmMin(() => {
	foo_exports = {};
	__export(foo_exports, { foo: () => foo });
	foo = 123;
});

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
+++ rolldown	entry_js.mjs
@@ -1,11 +1,9 @@
-var foo_exports = {};
-__export(foo_exports, { foo: () => foo });
-var foo;
-var init_foo = __esm({
-    'foo.js'() {
-        foo = 123;
-    }
+var foo_exports, foo;
+var init_foo = __esmMin(() => {
+    foo_exports = {};
+    __export(foo_exports, { foo: () => foo });
+    foo = 123;
 });
 init_foo();
 var ns2 = (init_foo(), __toCommonJS(foo_exports));
 console.log(foo, ns2.foo);
\ No newline at end of file

```
