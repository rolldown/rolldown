# Diff
## /out/entry.js
### esbuild
```js
var ns;
((ns2) => {
  var _stack = [];
  try {
    ns2.a = b;
    const c = __using(_stack, d);
    ns2.e = f;
  } catch (_) {
    var _error = _, _hasError = true;
  } finally {
    __callDispose(_stack, _error, _hasError);
  }
})(ns || (ns = {}));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,13 +0,0 @@
-var ns;
-(ns2 => {
-    var _stack = [];
-    try {
-        ns2.a = b;
-        const c = __using(_stack, d);
-        ns2.e = f;
-    } catch (_) {
-        var _error = _, _hasError = true;
-    } finally {
-        __callDispose(_stack, _error, _hasError);
-    }
-})(ns || (ns = {}));

```