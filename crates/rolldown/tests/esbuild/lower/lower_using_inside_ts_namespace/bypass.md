# Reason
1. pure transformation is handled by `oxc-transform`
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

//#region entry.ts
let ns;
(function(_ns) {
	let a = _ns.a = b;
	using c = d;
	let e = _ns.e = f;
})(ns || (ns = {}));
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,13 +1,9 @@
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
+
+//#region entry.ts
+let ns;
+(function(_ns) {
+	let a = _ns.a = b;
+	using c = d;
+	let e = _ns.e = f;
 })(ns || (ns = {}));
+//#endregion

```