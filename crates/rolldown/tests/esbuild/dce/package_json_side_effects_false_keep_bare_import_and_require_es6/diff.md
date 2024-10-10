<<<<<<< HEAD
# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/index.js
var demo_pkg_exports = {};
__export(demo_pkg_exports, {
  foo: () => foo
});
var foo;
var init_demo_pkg = __esm({
  "Users/user/project/node_modules/demo-pkg/index.js"() {
    foo = 123;
    console.log("hello");
  }
});

// Users/user/project/src/entry.js
init_demo_pkg();
console.log("unused import");
```
### rolldown
```js


//#region node_modules/demo-pkg/index.js
var demo_pkg_index_exports, foo;
var init_demo_pkg_index = __esm({ "node_modules/demo-pkg/index.js"() {
	demo_pkg_index_exports = {};
	__export(demo_pkg_index_exports, { foo: () => foo });
	foo = 123;
	console.log("hello");
} });

//#endregion
//#region src/entry.js
init_demo_pkg_index();
init_demo_pkg_index(), __toCommonJS(demo_pkg_index_exports);
console.log("unused import");

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry_js.js
@@ -1,13 +1,14 @@
-var demo_pkg_exports = {};
-__export(demo_pkg_exports, {
-    foo: () => foo
-});
-var foo;
-var init_demo_pkg = __esm({
-    "Users/user/project/node_modules/demo-pkg/index.js"() {
+var demo_pkg_index_exports, foo;
+var init_demo_pkg_index = __esm({
+    "node_modules/demo-pkg/index.js"() {
+        demo_pkg_index_exports = {};
+        __export(demo_pkg_index_exports, {
+            foo: () => foo
+        });
         foo = 123;
         console.log("hello");
     }
 });
-init_demo_pkg();
+init_demo_pkg_index();
+(init_demo_pkg_index(), __toCommonJS(demo_pkg_index_exports));
 console.log("unused import");

```