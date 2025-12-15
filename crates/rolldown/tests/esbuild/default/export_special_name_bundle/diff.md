## /out.js
### esbuild
```js
// lib.mjs
var lib_exports = {};
__export(lib_exports, {
  ["__proto__"]: () => __proto__
});
var __proto__;
var init_lib = __esm({
  "lib.mjs"() {
    __proto__ = 123;
  }
});

// entry.js
var lib = (init_lib(), __toCommonJS(lib_exports));
console.log(lib.__proto__);
```
### rolldown
```js
// HIDDEN [rolldown:runtime]

//#region lib.mjs
var lib_exports = /* @__PURE__ */ __export({ __proto__: () => __proto__ });
var __proto__;
var init_lib = __esmMin((() => {
	__proto__ = 123;
}));

//#endregion
//#region entry.js
const lib = (init_lib(), __toCommonJS(lib_exports));
console.log(lib.__proto__);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,12 +1,9 @@
-var lib_exports = {};
-__export(lib_exports, {
-    ["__proto__"]: () => __proto__
+var lib_exports = __export({
+    __proto__: () => __proto__
 });
 var __proto__;
-var init_lib = __esm({
-    "lib.mjs"() {
-        __proto__ = 123;
-    }
+var init_lib = __esmMin(() => {
+    __proto__ = 123;
 });
 var lib = (init_lib(), __toCommonJS(lib_exports));
 console.log(lib.__proto__);

```