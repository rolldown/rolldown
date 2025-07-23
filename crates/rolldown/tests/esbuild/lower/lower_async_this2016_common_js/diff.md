# Diff
## /out.js
### esbuild
```js
// entry.js
var require_entry = __commonJS({
  "entry.js"(exports) {
    exports.foo = () => __async(exports, null, function* () {
      return exports;
    });
  }
});
export default require_entry();
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region entry.js
var require_entry = /* @__PURE__ */ __commonJS({ "entry.js"(exports) {
	exports.foo = async () => exports;
} });

//#endregion
export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,6 @@
 var require_entry = __commonJS({
     "entry.js"(exports) {
-        exports.foo = () => __async(exports, null, function* () {
-            return exports;
-        });
+        exports.foo = async () => exports;
     }
 });
 export default require_entry();

```