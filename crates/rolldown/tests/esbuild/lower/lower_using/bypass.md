# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out/entry.js
### esbuild
```js
function foo() {
  var _stack4 = [];
  try {
    const a2 = __using(_stack4, b);
    if (nested) {
      var _stack3 = [];
      try {
        const x = __using(_stack3, 1);
      } catch (_3) {
        var _error3 = _3, _hasError3 = true;
      } finally {
        __callDispose(_stack3, _error3, _hasError3);
      }
    }
  } catch (_4) {
    var _error4 = _4, _hasError4 = true;
  } finally {
    __callDispose(_stack4, _error4, _hasError4);
  }
}
async function bar() {
  var _stack4 = [];
  try {
    const a2 = __using(_stack4, b);
    const c2 = __using(_stack4, d, true);
    if (nested) {
      var _stack3 = [];
      try {
        const x = __using(_stack3, 1);
        const y = __using(_stack3, 2, true);
      } catch (_3) {
        var _error3 = _3, _hasError3 = true;
      } finally {
        var _promise3 = __callDispose(_stack3, _error3, _hasError3);
        _promise3 && await _promise3;
      }
    }
  } catch (_4) {
    var _error4 = _4, _hasError4 = true;
  } finally {
    var _promise4 = __callDispose(_stack4, _error4, _hasError4);
    _promise4 && await _promise4;
  }
}
var _stack2 = [];
try {
  var a = __using(_stack2, b);
  var c = __using(_stack2, d, true);
  if (nested) {
    var _stack = [];
    try {
      const x = __using(_stack, 1);
      const y = __using(_stack, 2, true);
    } catch (_) {
      var _error = _, _hasError = true;
    } finally {
      var _promise = __callDispose(_stack, _error, _hasError);
      _promise && await _promise;
    }
  }
} catch (_2) {
  var _error2 = _2, _hasError2 = true;
} finally {
  var _promise2 = __callDispose(_stack2, _error2, _hasError2);
  _promise2 && await _promise2;
}
```
### rolldown
```js
//#region entry.js
using a = b;
await using c = d;
if (nested) {
	using x = 1;
	await using y = 2;
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,66 +1,9 @@
-function foo() {
-    var _stack4 = [];
-    try {
-        const a2 = __using(_stack4, b);
-        if (nested) {
-            var _stack3 = [];
-            try {
-                const x = __using(_stack3, 1);
-            } catch (_3) {
-                var _error3 = _3, _hasError3 = true;
-            } finally {
-                __callDispose(_stack3, _error3, _hasError3);
-            }
-        }
-    } catch (_4) {
-        var _error4 = _4, _hasError4 = true;
-    } finally {
-        __callDispose(_stack4, _error4, _hasError4);
-    }
+//#region entry.js
+using a = b;
+await using c = d;
+if (nested) {
+	using x = 1;
+	await using y = 2;
 }
-async function bar() {
-    var _stack4 = [];
-    try {
-        const a2 = __using(_stack4, b);
-        const c2 = __using(_stack4, d, true);
-        if (nested) {
-            var _stack3 = [];
-            try {
-                const x = __using(_stack3, 1);
-                const y = __using(_stack3, 2, true);
-            } catch (_3) {
-                var _error3 = _3, _hasError3 = true;
-            } finally {
-                var _promise3 = __callDispose(_stack3, _error3, _hasError3);
-                _promise3 && await _promise3;
-            }
-        }
-    } catch (_4) {
-        var _error4 = _4, _hasError4 = true;
-    } finally {
-        var _promise4 = __callDispose(_stack4, _error4, _hasError4);
-        _promise4 && await _promise4;
-    }
-}
-var _stack2 = [];
-try {
-    var a = __using(_stack2, b);
-    var c = __using(_stack2, d, true);
-    if (nested) {
-        var _stack = [];
-        try {
-            const x = __using(_stack, 1);
-            const y = __using(_stack, 2, true);
-        } catch (_) {
-            var _error = _, _hasError = true;
-        } finally {
-            var _promise = __callDispose(_stack, _error, _hasError);
-            _promise && await _promise;
-        }
-    }
-} catch (_2) {
-    var _error2 = _2, _hasError2 = true;
-} finally {
-    var _promise2 = __callDispose(_stack2, _error2, _hasError2);
-    _promise2 && await _promise2;
-}
+
+//#endregion
\ No newline at end of file

```
## /out/loops.js
### esbuild
```js
for (var _a of b) {
  var _stack = [];
  try {
    const a = __using(_stack, _a);
    c(() => a);
  } catch (_) {
    var _error = _, _hasError = true;
  } finally {
    __callDispose(_stack, _error, _hasError);
  }
}
for (var _d of e) {
  var _stack2 = [];
  try {
    const d = __using(_stack2, _d, true);
    f(() => d);
  } catch (_2) {
    var _error2 = _2, _hasError2 = true;
  } finally {
    var _promise = __callDispose(_stack2, _error2, _hasError2);
    _promise && await _promise;
  }
}
for await (var _g of h) {
  var _stack3 = [];
  try {
    const g = __using(_stack3, _g);
    i(() => g);
  } catch (_3) {
    var _error3 = _3, _hasError3 = true;
  } finally {
    __callDispose(_stack3, _error3, _hasError3);
  }
}
for await (var _j of k) {
  var _stack4 = [];
  try {
    const j = __using(_stack4, _j, true);
    l(() => j);
  } catch (_4) {
    var _error4 = _4, _hasError4 = true;
  } finally {
    var _promise2 = __callDispose(_stack4, _error4, _hasError4);
    _promise2 && await _promise2;
  }
}
if (nested) {
  for (var _a of b) {
    var _stack5 = [];
    try {
      const a = __using(_stack5, _a);
      c(() => a);
    } catch (_5) {
      var _error5 = _5, _hasError5 = true;
    } finally {
      __callDispose(_stack5, _error5, _hasError5);
    }
  }
  for (var _d of e) {
    var _stack6 = [];
    try {
      const d = __using(_stack6, _d, true);
      f(() => d);
    } catch (_6) {
      var _error6 = _6, _hasError6 = true;
    } finally {
      var _promise3 = __callDispose(_stack6, _error6, _hasError6);
      _promise3 && await _promise3;
    }
  }
  for await (var _g of h) {
    var _stack7 = [];
    try {
      const g = __using(_stack7, _g);
      i(() => g);
    } catch (_7) {
      var _error7 = _7, _hasError7 = true;
    } finally {
      __callDispose(_stack7, _error7, _hasError7);
    }
  }
  for await (var _j of k) {
    var _stack8 = [];
    try {
      const j = __using(_stack8, _j, true);
      l(() => j);
    } catch (_8) {
      var _error8 = _8, _hasError8 = true;
    } finally {
      var _promise4 = __callDispose(_stack8, _error8, _hasError8);
      _promise4 && await _promise4;
    }
  }
}
function foo() {
  for (var _a of b) {
    var _stack9 = [];
    try {
      const a = __using(_stack9, _a);
      c(() => a);
    } catch (_9) {
      var _error9 = _9, _hasError9 = true;
    } finally {
      __callDispose(_stack9, _error9, _hasError9);
    }
  }
}
async function bar() {
  for (var _a of b) {
    var _stack9 = [];
    try {
      const a = __using(_stack9, _a);
      c(() => a);
    } catch (_9) {
      var _error9 = _9, _hasError9 = true;
    } finally {
      __callDispose(_stack9, _error9, _hasError9);
    }
  }
  for (var _d of e) {
    var _stack10 = [];
    try {
      const d = __using(_stack10, _d, true);
      f(() => d);
    } catch (_10) {
      var _error10 = _10, _hasError10 = true;
    } finally {
      var _promise5 = __callDispose(_stack10, _error10, _hasError10);
      _promise5 && await _promise5;
    }
  }
  for await (var _g of h) {
    var _stack11 = [];
    try {
      const g = __using(_stack11, _g);
      i(() => g);
    } catch (_11) {
      var _error11 = _11, _hasError11 = true;
    } finally {
      __callDispose(_stack11, _error11, _hasError11);
    }
  }
  for await (var _j of k) {
    var _stack12 = [];
    try {
      const j = __using(_stack12, _j, true);
      l(() => j);
    } catch (_12) {
      var _error12 = _12, _hasError12 = true;
    } finally {
      var _promise6 = __callDispose(_stack12, _error12, _hasError12);
      _promise6 && await _promise6;
    }
  }
}
```
### rolldown
```js
//#region loops.js
for (using a of b) c(() => a);
for (await using d of e) f(() => d);
for await (using g of h) i(() => g);
for await (await using j of k) l(() => j);
if (nested) {
	for (using a of b) c(() => a);
	for (await using d of e) f(() => d);
	for await (using g of h) i(() => g);
	for await (await using j of k) l(() => j);
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/loops.js
+++ rolldown	loops.js
@@ -1,155 +1,13 @@
-for (var _a of b) {
-    var _stack = [];
-    try {
-        const a = __using(_stack, _a);
-        c(() => a);
-    } catch (_) {
-        var _error = _, _hasError = true;
-    } finally {
-        __callDispose(_stack, _error, _hasError);
-    }
-}
-for (var _d of e) {
-    var _stack2 = [];
-    try {
-        const d = __using(_stack2, _d, true);
-        f(() => d);
-    } catch (_2) {
-        var _error2 = _2, _hasError2 = true;
-    } finally {
-        var _promise = __callDispose(_stack2, _error2, _hasError2);
-        _promise && await _promise;
-    }
-}
-for await (var _g of h) {
-    var _stack3 = [];
-    try {
-        const g = __using(_stack3, _g);
-        i(() => g);
-    } catch (_3) {
-        var _error3 = _3, _hasError3 = true;
-    } finally {
-        __callDispose(_stack3, _error3, _hasError3);
-    }
-}
-for await (var _j of k) {
-    var _stack4 = [];
-    try {
-        const j = __using(_stack4, _j, true);
-        l(() => j);
-    } catch (_4) {
-        var _error4 = _4, _hasError4 = true;
-    } finally {
-        var _promise2 = __callDispose(_stack4, _error4, _hasError4);
-        _promise2 && await _promise2;
-    }
-}
+//#region loops.js
+for (using a of b) c(() => a);
+for (await using d of e) f(() => d);
+for await (using g of h) i(() => g);
+for await (await using j of k) l(() => j);
 if (nested) {
-    for (var _a of b) {
-        var _stack5 = [];
-        try {
-            const a = __using(_stack5, _a);
-            c(() => a);
-        } catch (_5) {
-            var _error5 = _5, _hasError5 = true;
-        } finally {
-            __callDispose(_stack5, _error5, _hasError5);
-        }
-    }
-    for (var _d of e) {
-        var _stack6 = [];
-        try {
-            const d = __using(_stack6, _d, true);
-            f(() => d);
-        } catch (_6) {
-            var _error6 = _6, _hasError6 = true;
-        } finally {
-            var _promise3 = __callDispose(_stack6, _error6, _hasError6);
-            _promise3 && await _promise3;
-        }
-    }
-    for await (var _g of h) {
-        var _stack7 = [];
-        try {
-            const g = __using(_stack7, _g);
-            i(() => g);
-        } catch (_7) {
-            var _error7 = _7, _hasError7 = true;
-        } finally {
-            __callDispose(_stack7, _error7, _hasError7);
-        }
-    }
-    for await (var _j of k) {
-        var _stack8 = [];
-        try {
-            const j = __using(_stack8, _j, true);
-            l(() => j);
-        } catch (_8) {
-            var _error8 = _8, _hasError8 = true;
-        } finally {
-            var _promise4 = __callDispose(_stack8, _error8, _hasError8);
-            _promise4 && await _promise4;
-        }
-    }
+	for (using a of b) c(() => a);
+	for (await using d of e) f(() => d);
+	for await (using g of h) i(() => g);
+	for await (await using j of k) l(() => j);
 }
-function foo() {
-    for (var _a of b) {
-        var _stack9 = [];
-        try {
-            const a = __using(_stack9, _a);
-            c(() => a);
-        } catch (_9) {
-            var _error9 = _9, _hasError9 = true;
-        } finally {
-            __callDispose(_stack9, _error9, _hasError9);
-        }
-    }
-}
-async function bar() {
-    for (var _a of b) {
-        var _stack9 = [];
-        try {
-            const a = __using(_stack9, _a);
-            c(() => a);
-        } catch (_9) {
-            var _error9 = _9, _hasError9 = true;
-        } finally {
-            __callDispose(_stack9, _error9, _hasError9);
-        }
-    }
-    for (var _d of e) {
-        var _stack10 = [];
-        try {
-            const d = __using(_stack10, _d, true);
-            f(() => d);
-        } catch (_10) {
-            var _error10 = _10, _hasError10 = true;
-        } finally {
-            var _promise5 = __callDispose(_stack10, _error10, _hasError10);
-            _promise5 && await _promise5;
-        }
-    }
-    for await (var _g of h) {
-        var _stack11 = [];
-        try {
-            const g = __using(_stack11, _g);
-            i(() => g);
-        } catch (_11) {
-            var _error11 = _11, _hasError11 = true;
-        } finally {
-            __callDispose(_stack11, _error11, _hasError11);
-        }
-    }
-    for await (var _j of k) {
-        var _stack12 = [];
-        try {
-            const j = __using(_stack12, _j, true);
-            l(() => j);
-        } catch (_12) {
-            var _error12 = _12, _hasError12 = true;
-        } finally {
-            var _promise6 = __callDispose(_stack12, _error12, _hasError12);
-            _promise6 && await _promise6;
-        }
-    }
-}
+
+//#endregion
\ No newline at end of file

```
## /out/switch.js
### esbuild
```js
async function foo() {
  var _stack6 = [];
  try {
    const x2 = __using(_stack6, y);
    var _stack4 = [];
    try {
      switch (foo) {
        case 0:
          const c = __using(_stack4, d);
        default:
          const e = __using(_stack4, f);
      }
    } catch (_4) {
      var _error4 = _4, _hasError4 = true;
    } finally {
      __callDispose(_stack4, _error4, _hasError4);
    }
    var _stack5 = [];
    try {
      switch (foo) {
        case 0:
          const c = __using(_stack5, d, true);
        default:
          const e = __using(_stack5, f);
      }
    } catch (_5) {
      var _error5 = _5, _hasError5 = true;
    } finally {
      var _promise2 = __callDispose(_stack5, _error5, _hasError5);
      _promise2 && await _promise2;
    }
  } catch (_6) {
    var _error6 = _6, _hasError6 = true;
  } finally {
    __callDispose(_stack6, _error6, _hasError6);
  }
}
var _stack3 = [];
try {
  var x = __using(_stack3, y);
  var _stack = [];
  try {
    switch (foo) {
      case 0:
        var c = __using(_stack, d);
      default:
        var e = __using(_stack, f);
    }
  } catch (_) {
    var _error = _, _hasError = true;
  } finally {
    __callDispose(_stack, _error, _hasError);
  }
  var _stack2 = [];
  try {
    switch (foo) {
      case 0:
        var c = __using(_stack2, d, true);
      default:
        var e = __using(_stack2, f);
    }
  } catch (_2) {
    var _error2 = _2, _hasError2 = true;
  } finally {
    var _promise = __callDispose(_stack2, _error2, _hasError2);
    _promise && await _promise;
  }
} catch (_3) {
  var _error3 = _3, _hasError3 = true;
} finally {
  __callDispose(_stack3, _error3, _hasError3);
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
switch (foo) {
	case 0: await using c = d;
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

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/switch.js
+++ rolldown	switch.js
@@ -1,72 +1,23 @@
-async function foo() {
-    var _stack6 = [];
-    try {
-        const x2 = __using(_stack6, y);
-        var _stack4 = [];
-        try {
-            switch (foo) {
-                case 0:
-                    const c = __using(_stack4, d);
-                default:
-                    const e = __using(_stack4, f);
-            }
-        } catch (_4) {
-            var _error4 = _4, _hasError4 = true;
-        } finally {
-            __callDispose(_stack4, _error4, _hasError4);
-        }
-        var _stack5 = [];
-        try {
-            switch (foo) {
-                case 0:
-                    const c = __using(_stack5, d, true);
-                default:
-                    const e = __using(_stack5, f);
-            }
-        } catch (_5) {
-            var _error5 = _5, _hasError5 = true;
-        } finally {
-            var _promise2 = __callDispose(_stack5, _error5, _hasError5);
-            _promise2 && await _promise2;
-        }
-    } catch (_6) {
-        var _error6 = _6, _hasError6 = true;
-    } finally {
-        __callDispose(_stack6, _error6, _hasError6);
-    }
+//#region switch.js
+using x = y;
+switch (foo) {
+	case 0: using c = d;
+	default: using e = f;
 }
-var _stack3 = [];
-try {
-    var x = __using(_stack3, y);
-    var _stack = [];
-    try {
-        switch (foo) {
-            case 0:
-                var c = __using(_stack, d);
-            default:
-                var e = __using(_stack, f);
-        }
-    } catch (_) {
-        var _error = _, _hasError = true;
-    } finally {
-        __callDispose(_stack, _error, _hasError);
-    }
-    var _stack2 = [];
-    try {
-        switch (foo) {
-            case 0:
-                var c = __using(_stack2, d, true);
-            default:
-                var e = __using(_stack2, f);
-        }
-    } catch (_2) {
-        var _error2 = _2, _hasError2 = true;
-    } finally {
-        var _promise = __callDispose(_stack2, _error2, _hasError2);
-        _promise && await _promise;
-    }
-} catch (_3) {
-    var _error3 = _3, _hasError3 = true;
-} finally {
-    __callDispose(_stack3, _error3, _hasError3);
+switch (foo) {
+	case 0: await using c = d;
+	default: using e = f;
 }
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
+
+//#endregion
\ No newline at end of file

```