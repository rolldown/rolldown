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
var foo_exports, foo;
var init_foo = __esm({ "foo.ts"() {
	foo_exports = {};
	__export(foo_exports, { foo: () => foo });
	foo = 123;
} });

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
@@ -1,11 +1,11 @@
-var foo_exports = {};
-__export(foo_exports, {
-    foo: () => foo
-});
-var foo;
+var foo_exports, foo;
 var init_foo = __esm({
     "foo.ts"() {
+        foo_exports = {};
+        __export(foo_exports, {
+            foo: () => foo
+        });
         foo = 123;
     }
 });
 init_foo();

```