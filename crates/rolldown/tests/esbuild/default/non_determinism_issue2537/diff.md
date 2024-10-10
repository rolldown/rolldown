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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,14 +0,0 @@
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
-    }
-    return r;
-}
-export {i as aap};

```