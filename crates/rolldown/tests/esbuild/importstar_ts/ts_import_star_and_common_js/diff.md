# Diff
## /out.js
### esbuild
```js
// foo.ts
var foo_exports = {};
__export(foo_exports, {
  foo: () => foo
});
var foo;
var init_foo = __esm({
  "foo.ts"() {
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

//#region foo.ts
var foo_exports = {};
__export(foo_exports, { foo: () => foo });
const foo = 123;
var init_foo = __esm({ "foo.ts"() {} });

//#endregion
//#region entry.js
init_foo();
const ns2 = (init_foo(), __toCommonJS(foo_exports));
console.log(foo, ns2.foo);

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
-    "foo.ts"() {
-        foo = 123;
-    }
+    "foo.ts"() {}
 });
 init_foo();
 var ns2 = (init_foo(), __toCommonJS(foo_exports));
 console.log(foo, ns2.foo);

```