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
//#region ../../../../../../node_modules/.pnpm/@oxc-project+runtime@0.72.3/node_modules/@oxc-project/runtime/src/helpers/esm/usingCtx.js
function _usingCtx() {
	var r = "function" == typeof SuppressedError ? SuppressedError : function(r$1, e$1) {
		var n$1 = Error();
		return n$1.name = "SuppressedError", n$1.error = r$1, n$1.suppressed = e$1, n$1;
	}, e = {}, n = [];
	function using(r$1, e$1) {
		if (null != e$1) {
			if (Object(e$1) !== e$1) throw new TypeError("using declarations can only be used with objects, functions, null, or undefined.");
			if (r$1) var o = e$1[Symbol.asyncDispose || Symbol["for"]("Symbol.asyncDispose")];
			if (void 0 === o && (o = e$1[Symbol.dispose || Symbol["for"]("Symbol.dispose")], r$1)) var t = o;
			if ("function" != typeof o) throw new TypeError("Object is not disposable.");
			t && (o = function o$1() {
				try {
					t.call(e$1);
				} catch (r$2) {
					return Promise.reject(r$2);
				}
			}), n.push({
				v: e$1,
				d: o,
				a: r$1
			});
		} else r$1 && n.push({
			d: e$1,
			a: r$1
		});
		return e$1;
	}
	return {
		e,
		u: using.bind(null, !1),
		a: using.bind(null, !0),
		d: function d$1() {
			var o, t = this.e, s = 0;
			function next() {
				for (; o = n.pop();) try {
					if (!o.a && 1 === s) return s = 0, n.push(o), Promise.resolve().then(next);
					if (o.d) {
						var r$1 = o.d.call(o.v);
						if (o.a) return s |= 2, Promise.resolve(r$1).then(next, err);
					} else s |= 1;
				} catch (r$2) {
					return err(r$2);
				}
				if (1 === s) return t !== e ? Promise.reject(t) : Promise.resolve();
				if (t !== e) throw t;
			}
			function err(n$1) {
				return t = t !== e ? new r(n$1, t) : n$1, next();
			}
			return next();
		}
	};
}

//#endregion
//#region entry.ts
let ns;
(function(_ns) {
	try {
		var _usingCtx$1 = _usingCtx();
		let a = _ns.a = b;
		const c = _usingCtx$1.u(d);
		let e = _ns.e = f;
	} catch (_) {
		_usingCtx$1.e = _;
	} finally {
		_usingCtx$1.d();
	}
})(ns || (ns = {}));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,13 +1,67 @@
+function _usingCtx() {
+    var r = "function" == typeof SuppressedError ? SuppressedError : function (r$1, e$1) {
+        var n$1 = Error();
+        return (n$1.name = "SuppressedError", n$1.error = r$1, n$1.suppressed = e$1, n$1);
+    }, e = {}, n = [];
+    function using(r$1, e$1) {
+        if (null != e$1) {
+            if (Object(e$1) !== e$1) throw new TypeError("using declarations can only be used with objects, functions, null, or undefined.");
+            if (r$1) var o = e$1[Symbol.asyncDispose || Symbol["for"]("Symbol.asyncDispose")];
+            if (void 0 === o && (o = e$1[Symbol.dispose || Symbol["for"]("Symbol.dispose")], r$1)) var t = o;
+            if ("function" != typeof o) throw new TypeError("Object is not disposable.");
+            (t && (o = function o$1() {
+                try {
+                    t.call(e$1);
+                } catch (r$2) {
+                    return Promise.reject(r$2);
+                }
+            }), n.push({
+                v: e$1,
+                d: o,
+                a: r$1
+            }));
+        } else r$1 && n.push({
+            d: e$1,
+            a: r$1
+        });
+        return e$1;
+    }
+    return {
+        e,
+        u: using.bind(null, !1),
+        a: using.bind(null, !0),
+        d: function d$1() {
+            var o, t = this.e, s = 0;
+            function next() {
+                for (; o = n.pop(); ) try {
+                    if (!o.a && 1 === s) return (s = 0, n.push(o), Promise.resolve().then(next));
+                    if (o.d) {
+                        var r$1 = o.d.call(o.v);
+                        if (o.a) return (s |= 2, Promise.resolve(r$1).then(next, err));
+                    } else s |= 1;
+                } catch (r$2) {
+                    return err(r$2);
+                }
+                if (1 === s) return t !== e ? Promise.reject(t) : Promise.resolve();
+                if (t !== e) throw t;
+            }
+            function err(n$1) {
+                return (t = t !== e ? new r(n$1, t) : n$1, next());
+            }
+            return next();
+        }
+    };
+}
 var ns;
-(ns2 => {
-    var _stack = [];
+(function (_ns) {
     try {
-        ns2.a = b;
-        const c = __using(_stack, d);
-        ns2.e = f;
+        var _usingCtx$1 = _usingCtx();
+        let a = _ns.a = b;
+        const c = _usingCtx$1.u(d);
+        let e = _ns.e = f;
     } catch (_) {
-        var _error = _, _hasError = true;
+        _usingCtx$1.e = _;
     } finally {
-        __callDispose(_stack, _error, _hasError);
+        _usingCtx$1.d();
     }
 })(ns || (ns = {}));

```