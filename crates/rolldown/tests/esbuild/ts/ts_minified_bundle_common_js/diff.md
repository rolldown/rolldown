# Diff
## /out.js
### esbuild
```js
var t=e(r=>{r.foo=function(){return 123}});var n=e((l,c)=>{c.exports={test:!0}});var{foo:f}=t();console.log(f(),n());
```
### rolldown
```js


//#region a.ts
var require_a = __commonJS({ "a.ts"(exports) {
	exports.foo = function() {
		return 123;
	};
} });

//#endregion
//#region j.json
var j_exports, test, j_default;
var init_j = __esm({ "j.json"() {
	j_exports = {};
	__export(j_exports, {
		default: () => j_default,
		test: () => test
	});
	test = true;
	j_default = { test };
} });

//#endregion
//#region entry.ts
const { foo } = require_a();
console.log(foo(), (init_j(), __toCommonJS(j_exports).default));

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,12 +1,23 @@
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
+var j_exports, test, j_default;
+var init_j = __esm({
+    "j.json"() {
+        j_exports = {};
+        __export(j_exports, {
+            default: () => j_default,
+            test: () => test
+        });
+        test = true;
+        j_default = {
+            test
+        };
+    }
 });
-var {foo: f} = t();
-console.log(f(), n());
+var {foo} = require_a();
+console.log(foo(), (init_j(), __toCommonJS(j_exports).default));

```