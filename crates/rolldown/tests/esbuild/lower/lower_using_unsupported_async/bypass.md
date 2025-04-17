# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out/entry.js
### esbuild
```js
function foo() {
  using a = b;
  if (nested) {
    using x = 1;
  }
}
function bar() {
  return __async(this, null, function* () {
    var _stack2 = [];
    try {
      const a = __using(_stack2, b);
      const c = __using(_stack2, d, true);
      if (nested) {
        var _stack = [];
        try {
          const x = __using(_stack, 1);
          const y = __using(_stack, 2, true);
        } catch (_) {
          var _error = _, _hasError = true;
        } finally {
          var _promise = __callDispose(_stack, _error, _hasError);
          _promise && (yield _promise);
        }
      }
    } catch (_2) {
      var _error2 = _2, _hasError2 = true;
    } finally {
      var _promise2 = __callDispose(_stack2, _error2, _hasError2);
      _promise2 && (yield _promise2);
    }
  });
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,32 +0,0 @@
-function foo() {
-  using a = b;
-  if (nested) {
-    using x = 1;
-  }
-}
-function bar() {
-  return __async(this, null, function* () {
-    var _stack2 = [];
-    try {
-      const a = __using(_stack2, b);
-      const c = __using(_stack2, d, true);
-      if (nested) {
-        var _stack = [];
-        try {
-          const x = __using(_stack, 1);
-          const y = __using(_stack, 2, true);
-        } catch (_) {
-          var _error = _, _hasError = true;
-        } finally {
-          var _promise = __callDispose(_stack, _error, _hasError);
-          _promise && (yield _promise);
-        }
-      }
-    } catch (_2) {
-      var _error2 = _2, _hasError2 = true;
-    } finally {
-      var _promise2 = __callDispose(_stack2, _error2, _hasError2);
-      _promise2 && (yield _promise2);
-    }
-  });
-}
\ No newline at end of file

```
## /out/loops.js
### esbuild
```js
for (using a of b) c(() => a);
if (nested) {
  for (using a of b) c(() => a);
}
function foo() {
  for (using a of b) c(() => a);
}
function bar() {
  return __async(this, null, function* () {
    for (using a of b) c(() => a);
    for (var _d of e) {
      var _stack = [];
      try {
        const d = __using(_stack, _d, true);
        f(() => d);
      } catch (_) {
        var _error = _, _hasError = true;
      } finally {
        var _promise = __callDispose(_stack, _error, _hasError);
        _promise && (yield _promise);
      }
    }
  });
}
```
### rolldown
```js

//#region loops.js
for (using a of b) c(() => a);
if (nested) for (using a of b) c(() => a);

```
### diff
```diff
===================================================================
--- esbuild	/out/loops.js
+++ rolldown	loops.js
@@ -1,24 +1,4 @@
+
+//#region loops.js
 for (using a of b) c(() => a);
-if (nested) {
-  for (using a of b) c(() => a);
-}
-function foo() {
-  for (using a of b) c(() => a);
-}
-function bar() {
-  return __async(this, null, function* () {
-    for (using a of b) c(() => a);
-    for (var _d of e) {
-      var _stack = [];
-      try {
-        const d = __using(_stack, _d, true);
-        f(() => d);
-      } catch (_) {
-        var _error = _, _hasError = true;
-      } finally {
-        var _promise = __callDispose(_stack, _error, _hasError);
-        _promise && (yield _promise);
-      }
-    }
-  });
-}
\ No newline at end of file
+if (nested) for (using a of b) c(() => a);

```
## /out/switch.js
### esbuild
```js
using x = y;
switch (foo) {
  case 0:
    using c = d;
  default:
    using e = f;
}
function foo() {
  return __async(this, null, function* () {
    using x2 = y;
    switch (foo) {
      case 0:
        using c = d;
      default:
        using e = f;
    }
    var _stack = [];
    try {
      switch (foo) {
        case 0:
          const c = __using(_stack, d, true);
        default:
          const e = __using(_stack, f);
      }
    } catch (_) {
      var _error = _, _hasError = true;
    } finally {
      var _promise = __callDispose(_stack, _error, _hasError);
      _promise && (yield _promise);
    }
  });
}
```
### rolldown
```js

//#region switch.js
using x = y;
switch (foo) {
	case 0: using c = d;
	default: using e = f;
}
async function foo() {
	using x$1 = y;
	switch (foo) {
		case 0: using c = d;
		default: using e = f;
	}
	switch (foo) {
		case 0: await using c = d;
		default: using e = f;
	}
}

```
### diff
```diff
===================================================================
--- esbuild	/out/switch.js
+++ rolldown	switch.js
@@ -1,32 +1,18 @@
+
+//#region switch.js
 using x = y;
 switch (foo) {
-  case 0:
-    using c = d;
-  default:
-    using e = f;
+	case 0: using c = d;
+	default: using e = f;
 }
-function foo() {
-  return __async(this, null, function* () {
-    using x2 = y;
-    switch (foo) {
-      case 0:
-        using c = d;
-      default:
-        using e = f;
-    }
-    var _stack = [];
-    try {
-      switch (foo) {
-        case 0:
-          const c = __using(_stack, d, true);
-        default:
-          const e = __using(_stack, f);
-      }
-    } catch (_) {
-      var _error = _, _hasError = true;
-    } finally {
-      var _promise = __callDispose(_stack, _error, _hasError);
-      _promise && (yield _promise);
-    }
-  });
-}
\ No newline at end of file
+async function foo() {
+	using x$1 = y;
+	switch (foo) {
+		case 0: using c = d;
+		default: using e = f;
+	}
+	switch (foo) {
+		case 0: await using c = d;
+		default: using e = f;
+	}
+}

```