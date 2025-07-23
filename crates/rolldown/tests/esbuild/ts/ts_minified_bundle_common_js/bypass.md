# Reason
1. could be done in minifier
# Diff
## /out.js
### esbuild
```js
var t=e(r=>{r.foo=function(){return 123}});var n=e((l,c)=>{c.exports={test:!0}});var{foo:f}=t();console.log(f(),n());
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region a.ts
var require_a = /* @__PURE__ */ __commonJS({ "a.ts"(exports) {
	exports.foo = function() {
		return 123;
	};
} });

//#endregion
//#region j.json
var require_j = /* @__PURE__ */ __commonJS({ "j.json"(exports, module) {
	module.exports = { "test": true };
} });

//#endregion
//#region entry.ts
const { foo } = require_a();
console.log(foo(), require_j());

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,12 +1,16 @@
-var t = e(r => {
-    r.foo = function () {
-        return 123;
-    };
+var require_a = __commonJS({
+    "a.ts"(exports) {
+        exports.foo = function () {
+            return 123;
+        };
+    }
 });
-var n = e((l, c) => {
-    c.exports = {
-        test: !0
-    };
+var require_j = __commonJS({
+    "j.json"(exports, module) {
+        module.exports = {
+            "test": true
+        };
+    }
 });
-var {foo: f} = t();
-console.log(f(), n());
+var {foo} = require_a();
+console.log(foo(), require_j());

```