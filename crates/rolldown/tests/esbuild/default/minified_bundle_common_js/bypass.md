# Reason
1. could be done in minifier
2. without minifier https://hyrious.me/esbuild-repl/?version=0.23.0&b=e%00entry.js%00const+%7Bfoo%7D+%3D+require%28%27.%2Fa%27%29%0A%09%09%09%09console.log%28foo%28%29%2C+require%28%27.%2Fj.json%27%29%29&b=%00a.js%00exports.foo+%3D+function%28%29+%7B%0A%09%09%09%09%09return+123%0A%09%09%09%09%7D&b=%00j.json%00%09%7B%22test%22%3A+true%7D&o=%7B%0A++treeShaking%3A+true%2C%0A++external%3A+%5B%22c%22%2C+%22a%22%2C+%22b%22%5D%2C%0A%22bundle%22%3A+true%2C%0Aformat%3A+%22esm%22%0A%7D
# Diff
## /out.js
### esbuild
```js
var t=e(r=>{r.foo=function(){return 123}});var n=e((l,c)=>{c.exports={test:!0}});var{foo:f}=t();console.log(f(),n());
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region a.js
var require_a = __commonJS({ "a.js"(exports) {
	exports.foo = function() {
		return 123;
	};
} });

//#endregion
//#region j.json
var require_j = __commonJS({ "j.json"(exports, module) {
	module.exports = { "test": true };
} });

//#endregion
//#region entry.js
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
+    "a.js"(exports) {
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