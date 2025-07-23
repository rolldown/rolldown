# Reason
1. oxc codegen remove the block
# Diff
## /out.js
### esbuild
```js
// entry.js
var require_entry = __commonJS({
  "entry.js"(exports) {
    try {
      const supportsColor = __require("supports-color");
      if (supportsColor && (supportsColor.stderr || supportsColor).level >= 2) {
        exports.colors = [];
      }
    } catch (error) {
    }
  }
});
export default require_entry();
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region entry.js
var require_entry = /* @__PURE__ */ __commonJS({ "entry.js"(exports) {
	try {
		const supportsColor = __require("supports-color");
		if (supportsColor && (supportsColor.stderr || supportsColor).level >= 2) exports.colors = [];
	} catch (error) {}
} });

//#endregion
export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,9 @@
 var require_entry = __commonJS({
     "entry.js"(exports) {
         try {
             const supportsColor = __require("supports-color");
-            if (supportsColor && (supportsColor.stderr || supportsColor).level >= 2) {
-                exports.colors = [];
-            }
+            if (supportsColor && (supportsColor.stderr || supportsColor).level >= 2) exports.colors = [];
         } catch (error) {}
     }
 });
 export default require_entry();

```