# Reason
1. should read `tsconfig.json`
# Diff
## /out.js
### esbuild
```js
// entry.ts
function i(o, e) {
  let r = "teun";
  if (o) {
    let u = function(n) {
      return n * 2;
    }, t = function(n) {
      return n / 2;
    };
    var b = u, f = t;
    r = u(e) + t(e);
  }
  return r;
}
export {
  i as aap
};
```
### rolldown
```js

//#region entry.ts
function aap(noot, wim) {
	let mies = "teun";
	if (noot) {
		function vuur(v) {
			return v * 2;
		}
		function schaap(s) {
			return s / 2;
		}
		mies = vuur(wim) + schaap(wim);
	}
	return mies;
}

export { aap };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,14 +1,14 @@
-function i(o, e) {
-    let r = "teun";
-    if (o) {
-        let u = function (n) {
-            return n * 2;
-        }, t = function (n) {
-            return n / 2;
-        };
-        var b = u, f = t;
-        r = u(e) + t(e);
+function aap(noot, wim) {
+    let mies = "teun";
+    if (noot) {
+        function vuur(v) {
+            return v * 2;
+        }
+        function schaap(s) {
+            return s / 2;
+        }
+        mies = vuur(wim) + schaap(wim);
     }
-    return r;
+    return mies;
 }
-export {i as aap};
+export {aap};

```