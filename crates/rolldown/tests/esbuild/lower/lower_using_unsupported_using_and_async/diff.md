# Diff
## /out/entry.js
### esbuild
```js
function foo() {
  var _stack2 = [];
  try {
    const a = __using(_stack2, b);
    if (nested) {
      var _stack = [];
      try {
        const x = __using(_stack, 1);
      } catch (_) {
        var _error = _, _hasError = true;
      } finally {
        __callDispose(_stack, _error, _hasError);
      }
    }
  } catch (_2) {
    var _error2 = _2, _hasError2 = true;
  } finally {
    __callDispose(_stack2, _error2, _hasError2);
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
+++ rolldown	
@@ -1,46 +0,0 @@
-function foo() {
-    var _stack2 = [];
-    try {
-        const a = __using(_stack2, b);
-        if (nested) {
-            var _stack = [];
-            try {
-                const x = __using(_stack, 1);
-            } catch (_) {
-                var _error = _, _hasError = true;
-            } finally {
-                __callDispose(_stack, _error, _hasError);
-            }
-        }
-    } catch (_2) {
-        var _error2 = _2, _hasError2 = true;
-    } finally {
-        __callDispose(_stack2, _error2, _hasError2);
-    }
-}
-function bar() {
-    return __async(this, null, function* () {
-        var _stack2 = [];
-        try {
-            const a = __using(_stack2, b);
-            const c = __using(_stack2, d, true);
-            if (nested) {
-                var _stack = [];
-                try {
-                    const x = __using(_stack, 1);
-                    const y = __using(_stack, 2, true);
-                } catch (_) {
-                    var _error = _, _hasError = true;
-                } finally {
-                    var _promise = __callDispose(_stack, _error, _hasError);
-                    _promise && (yield _promise);
-                }
-            }
-        } catch (_2) {
-            var _error2 = _2, _hasError2 = true;
-        } finally {
-            var _promise2 = __callDispose(_stack2, _error2, _hasError2);
-            _promise2 && (yield _promise2);
-        }
-    });
-}

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
if (nested) {
  for (var _a of b) {
    var _stack2 = [];
    try {
      const a = __using(_stack2, _a);
      c(() => a);
    } catch (_2) {
      var _error2 = _2, _hasError2 = true;
    } finally {
      __callDispose(_stack2, _error2, _hasError2);
    }
  }
}
function foo() {
  for (var _a of b) {
    var _stack3 = [];
    try {
      const a = __using(_stack3, _a);
      c(() => a);
    } catch (_3) {
      var _error3 = _3, _hasError3 = true;
    } finally {
      __callDispose(_stack3, _error3, _hasError3);
    }
  }
}
function bar() {
  return __async(this, null, function* () {
    for (var _a of b) {
      var _stack3 = [];
      try {
        const a = __using(_stack3, _a);
        c(() => a);
      } catch (_3) {
        var _error3 = _3, _hasError3 = true;
      } finally {
        __callDispose(_stack3, _error3, _hasError3);
      }
    }
    for (var _d of e) {
      var _stack4 = [];
      try {
        const d = __using(_stack4, _d, true);
        f(() => d);
      } catch (_4) {
        var _error4 = _4, _hasError4 = true;
      } finally {
        var _promise = __callDispose(_stack4, _error4, _hasError4);
        _promise && (yield _promise);
      }
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
--- esbuild	/out/loops.js
+++ rolldown	
@@ -1,64 +0,0 @@
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
-if (nested) {
-    for (var _a of b) {
-        var _stack2 = [];
-        try {
-            const a = __using(_stack2, _a);
-            c(() => a);
-        } catch (_2) {
-            var _error2 = _2, _hasError2 = true;
-        } finally {
-            __callDispose(_stack2, _error2, _hasError2);
-        }
-    }
-}
-function foo() {
-    for (var _a of b) {
-        var _stack3 = [];
-        try {
-            const a = __using(_stack3, _a);
-            c(() => a);
-        } catch (_3) {
-            var _error3 = _3, _hasError3 = true;
-        } finally {
-            __callDispose(_stack3, _error3, _hasError3);
-        }
-    }
-}
-function bar() {
-    return __async(this, null, function* () {
-        for (var _a of b) {
-            var _stack3 = [];
-            try {
-                const a = __using(_stack3, _a);
-                c(() => a);
-            } catch (_3) {
-                var _error3 = _3, _hasError3 = true;
-            } finally {
-                __callDispose(_stack3, _error3, _hasError3);
-            }
-        }
-        for (var _d of e) {
-            var _stack4 = [];
-            try {
-                const d = __using(_stack4, _d, true);
-                f(() => d);
-            } catch (_4) {
-                var _error4 = _4, _hasError4 = true;
-            } finally {
-                var _promise = __callDispose(_stack4, _error4, _hasError4);
-                _promise && (yield _promise);
-            }
-        }
-    });
-}

```
## /out/switch.js
### esbuild
```js
function foo() {
  return __async(this, null, function* () {
    var _stack5 = [];
    try {
      const x2 = __using(_stack5, y);
      var _stack3 = [];
      try {
        switch (foo) {
          case 0:
            const c = __using(_stack3, d);
          default:
            const e = __using(_stack3, f);
        }
      } catch (_3) {
        var _error3 = _3, _hasError3 = true;
      } finally {
        __callDispose(_stack3, _error3, _hasError3);
      }
      var _stack4 = [];
      try {
        switch (foo) {
          case 0:
            const c = __using(_stack4, d, true);
          default:
            const e = __using(_stack4, f);
        }
      } catch (_4) {
        var _error4 = _4, _hasError4 = true;
      } finally {
        var _promise = __callDispose(_stack4, _error4, _hasError4);
        _promise && (yield _promise);
      }
    } catch (_5) {
      var _error5 = _5, _hasError5 = true;
    } finally {
      __callDispose(_stack5, _error5, _hasError5);
    }
  });
}
var _stack2 = [];
try {
  var x = __using(_stack2, y);
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
} catch (_2) {
  var _error2 = _2, _hasError2 = true;
} finally {
  __callDispose(_stack2, _error2, _hasError2);
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/switch.js
+++ rolldown	
@@ -1,60 +0,0 @@
-function foo() {
-    return __async(this, null, function* () {
-        var _stack5 = [];
-        try {
-            const x2 = __using(_stack5, y);
-            var _stack3 = [];
-            try {
-                switch (foo) {
-                    case 0:
-                        const c = __using(_stack3, d);
-                    default:
-                        const e = __using(_stack3, f);
-                }
-            } catch (_3) {
-                var _error3 = _3, _hasError3 = true;
-            } finally {
-                __callDispose(_stack3, _error3, _hasError3);
-            }
-            var _stack4 = [];
-            try {
-                switch (foo) {
-                    case 0:
-                        const c = __using(_stack4, d, true);
-                    default:
-                        const e = __using(_stack4, f);
-                }
-            } catch (_4) {
-                var _error4 = _4, _hasError4 = true;
-            } finally {
-                var _promise = __callDispose(_stack4, _error4, _hasError4);
-                _promise && (yield _promise);
-            }
-        } catch (_5) {
-            var _error5 = _5, _hasError5 = true;
-        } finally {
-            __callDispose(_stack5, _error5, _hasError5);
-        }
-    });
-}
-var _stack2 = [];
-try {
-    var x = __using(_stack2, y);
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
-} catch (_2) {
-    var _error2 = _2, _hasError2 = true;
-} finally {
-    __callDispose(_stack2, _error2, _hasError2);
-}

```