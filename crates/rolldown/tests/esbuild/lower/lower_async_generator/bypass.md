# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out/entry.js
### esbuild
```js
function foo() {
  return __asyncGenerator(this, null, function* () {
    var _stack2 = [];
    try {
      yield;
      yield x;
      yield* __yieldStar(x);
      const x = __using(_stack2, yield new __await(y), true);
      try {
        for (var iter = __forAwait(y), more, temp, error; more = !(temp = yield new __await(iter.next())).done; more = false) {
          let x2 = temp.value;
        }
      } catch (temp) {
        error = [temp];
      } finally {
        try {
          more && (temp = iter.return) && (yield new __await(temp.call(iter)));
        } finally {
          if (error)
            throw error[0];
        }
      }
      try {
        for (var iter2 = __forAwait(y), more2, temp2, error2; more2 = !(temp2 = yield new __await(iter2.next())).done; more2 = false) {
          var _x = temp2.value;
          var _stack = [];
          try {
            const x2 = __using(_stack, _x, true);
          } catch (_) {
            var _error = _, _hasError = true;
          } finally {
            var _promise = __callDispose(_stack, _error, _hasError);
            _promise && (yield new __await(_promise));
          }
        }
      } catch (temp2) {
        error2 = [temp2];
      } finally {
        try {
          more2 && (temp2 = iter2.return) && (yield new __await(temp2.call(iter2)));
        } finally {
          if (error2)
            throw error2[0];
        }
      }
    } catch (_2) {
      var _error2 = _2, _hasError2 = true;
    } finally {
      var _promise2 = __callDispose(_stack2, _error2, _hasError2);
      _promise2 && (yield new __await(_promise2));
    }
  });
}
foo = function() {
  return __asyncGenerator(this, null, function* () {
    var _stack2 = [];
    try {
      yield;
      yield x;
      yield* __yieldStar(x);
      const x = __using(_stack2, yield new __await(y), true);
      try {
        for (var iter = __forAwait(y), more, temp, error; more = !(temp = yield new __await(iter.next())).done; more = false) {
          let x2 = temp.value;
        }
      } catch (temp) {
        error = [temp];
      } finally {
        try {
          more && (temp = iter.return) && (yield new __await(temp.call(iter)));
        } finally {
          if (error)
            throw error[0];
        }
      }
      try {
        for (var iter2 = __forAwait(y), more2, temp2, error2; more2 = !(temp2 = yield new __await(iter2.next())).done; more2 = false) {
          var _x = temp2.value;
          var _stack = [];
          try {
            const x2 = __using(_stack, _x, true);
          } catch (_) {
            var _error = _, _hasError = true;
          } finally {
            var _promise = __callDispose(_stack, _error, _hasError);
            _promise && (yield new __await(_promise));
          }
        }
      } catch (temp2) {
        error2 = [temp2];
      } finally {
        try {
          more2 && (temp2 = iter2.return) && (yield new __await(temp2.call(iter2)));
        } finally {
          if (error2)
            throw error2[0];
        }
      }
    } catch (_2) {
      var _error2 = _2, _hasError2 = true;
    } finally {
      var _promise2 = __callDispose(_stack2, _error2, _hasError2);
      _promise2 && (yield new __await(_promise2));
    }
  });
};
foo = { bar() {
  return __asyncGenerator(this, null, function* () {
    var _stack2 = [];
    try {
      yield;
      yield x;
      yield* __yieldStar(x);
      const x = __using(_stack2, yield new __await(y), true);
      try {
        for (var iter = __forAwait(y), more, temp, error; more = !(temp = yield new __await(iter.next())).done; more = false) {
          let x2 = temp.value;
        }
      } catch (temp) {
        error = [temp];
      } finally {
        try {
          more && (temp = iter.return) && (yield new __await(temp.call(iter)));
        } finally {
          if (error)
            throw error[0];
        }
      }
      try {
        for (var iter2 = __forAwait(y), more2, temp2, error2; more2 = !(temp2 = yield new __await(iter2.next())).done; more2 = false) {
          var _x = temp2.value;
          var _stack = [];
          try {
            const x2 = __using(_stack, _x, true);
          } catch (_) {
            var _error = _, _hasError = true;
          } finally {
            var _promise = __callDispose(_stack, _error, _hasError);
            _promise && (yield new __await(_promise));
          }
        }
      } catch (temp2) {
        error2 = [temp2];
      } finally {
        try {
          more2 && (temp2 = iter2.return) && (yield new __await(temp2.call(iter2)));
        } finally {
          if (error2)
            throw error2[0];
        }
      }
    } catch (_2) {
      var _error2 = _2, _hasError2 = true;
    } finally {
      var _promise2 = __callDispose(_stack2, _error2, _hasError2);
      _promise2 && (yield new __await(_promise2));
    }
  });
} };
class Foo {
  bar() {
    return __asyncGenerator(this, null, function* () {
      var _stack2 = [];
      try {
        yield;
        yield x;
        yield* __yieldStar(x);
        const x = __using(_stack2, yield new __await(y), true);
        try {
          for (var iter = __forAwait(y), more, temp, error; more = !(temp = yield new __await(iter.next())).done; more = false) {
            let x2 = temp.value;
          }
        } catch (temp) {
          error = [temp];
        } finally {
          try {
            more && (temp = iter.return) && (yield new __await(temp.call(iter)));
          } finally {
            if (error)
              throw error[0];
          }
        }
        try {
          for (var iter2 = __forAwait(y), more2, temp2, error2; more2 = !(temp2 = yield new __await(iter2.next())).done; more2 = false) {
            var _x = temp2.value;
            var _stack = [];
            try {
              const x2 = __using(_stack, _x, true);
            } catch (_) {
              var _error = _, _hasError = true;
            } finally {
              var _promise = __callDispose(_stack, _error, _hasError);
              _promise && (yield new __await(_promise));
            }
          }
        } catch (temp2) {
          error2 = [temp2];
        } finally {
          try {
            more2 && (temp2 = iter2.return) && (yield new __await(temp2.call(iter2)));
          } finally {
            if (error2)
              throw error2[0];
          }
        }
      } catch (_2) {
        var _error2 = _2, _hasError2 = true;
      } finally {
        var _promise2 = __callDispose(_stack2, _error2, _hasError2);
        _promise2 && (yield new __await(_promise2));
      }
    });
  }
}
Foo = class {
  bar() {
    return __asyncGenerator(this, null, function* () {
      var _stack2 = [];
      try {
        yield;
        yield x;
        yield* __yieldStar(x);
        const x = __using(_stack2, yield new __await(y), true);
        try {
          for (var iter = __forAwait(y), more, temp, error; more = !(temp = yield new __await(iter.next())).done; more = false) {
            let x2 = temp.value;
          }
        } catch (temp) {
          error = [temp];
        } finally {
          try {
            more && (temp = iter.return) && (yield new __await(temp.call(iter)));
          } finally {
            if (error)
              throw error[0];
          }
        }
        try {
          for (var iter2 = __forAwait(y), more2, temp2, error2; more2 = !(temp2 = yield new __await(iter2.next())).done; more2 = false) {
            var _x = temp2.value;
            var _stack = [];
            try {
              const x2 = __using(_stack, _x, true);
            } catch (_) {
              var _error = _, _hasError = true;
            } finally {
              var _promise = __callDispose(_stack, _error, _hasError);
              _promise && (yield new __await(_promise));
            }
          }
        } catch (temp2) {
          error2 = [temp2];
        } finally {
          try {
            more2 && (temp2 = iter2.return) && (yield new __await(temp2.call(iter2)));
          } finally {
            if (error2)
              throw error2[0];
          }
        }
      } catch (_2) {
        var _error2 = _2, _hasError2 = true;
      } finally {
        var _promise2 = __callDispose(_stack2, _error2, _hasError2);
        _promise2 && (yield new __await(_promise2));
      }
    });
  }
};
async function bar() {
  await using x = await y;
  for await (let x2 of y) {
  }
  for await (await using x2 of y) {
  }
}
```
### rolldown
```js

//#region entry.ts
async function* foo() {
	yield;
	yield x;
	yield* x;
	await using x = await y;
	for await (let x$1 of y);
	for await (await using x$1 of y);
}
foo = async function* () {
	yield;
	yield x;
	yield* x;
	await using x = await y;
	for await (let x$1 of y);
	for await (await using x$1 of y);
};
foo = { async *bar() {
	yield;
	yield x;
	yield* x;
	await using x = await y;
	for await (let x$1 of y);
	for await (await using x$1 of y);
} };
var Foo = class {
	async *bar() {
		yield;
		yield x;
		yield* x;
		await using x = await y;
		for await (let x$1 of y);
		for await (await using x$1 of y);
	}
};
Foo = class {
	async *bar() {
		yield;
		yield x;
		yield* x;
		await using x = await y;
		for await (let x$1 of y);
		for await (await using x$1 of y);
	}
};
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,276 +1,47 @@
-function foo() {
-  return __asyncGenerator(this, null, function* () {
-    var _stack2 = [];
-    try {
-      yield;
-      yield x;
-      yield* __yieldStar(x);
-      const x = __using(_stack2, yield new __await(y), true);
-      try {
-        for (var iter = __forAwait(y), more, temp, error; more = !(temp = yield new __await(iter.next())).done; more = false) {
-          let x2 = temp.value;
-        }
-      } catch (temp) {
-        error = [temp];
-      } finally {
-        try {
-          more && (temp = iter.return) && (yield new __await(temp.call(iter)));
-        } finally {
-          if (error)
-            throw error[0];
-        }
-      }
-      try {
-        for (var iter2 = __forAwait(y), more2, temp2, error2; more2 = !(temp2 = yield new __await(iter2.next())).done; more2 = false) {
-          var _x = temp2.value;
-          var _stack = [];
-          try {
-            const x2 = __using(_stack, _x, true);
-          } catch (_) {
-            var _error = _, _hasError = true;
-          } finally {
-            var _promise = __callDispose(_stack, _error, _hasError);
-            _promise && (yield new __await(_promise));
-          }
-        }
-      } catch (temp2) {
-        error2 = [temp2];
-      } finally {
-        try {
-          more2 && (temp2 = iter2.return) && (yield new __await(temp2.call(iter2)));
-        } finally {
-          if (error2)
-            throw error2[0];
-        }
-      }
-    } catch (_2) {
-      var _error2 = _2, _hasError2 = true;
-    } finally {
-      var _promise2 = __callDispose(_stack2, _error2, _hasError2);
-      _promise2 && (yield new __await(_promise2));
-    }
-  });
+
+//#region entry.ts
+async function* foo() {
+	yield;
+	yield x;
+	yield* x;
+	await using x = await y;
+	for await (let x$1 of y);
+	for await (await using x$1 of y);
 }
-foo = function() {
-  return __asyncGenerator(this, null, function* () {
-    var _stack2 = [];
-    try {
-      yield;
-      yield x;
-      yield* __yieldStar(x);
-      const x = __using(_stack2, yield new __await(y), true);
-      try {
-        for (var iter = __forAwait(y), more, temp, error; more = !(temp = yield new __await(iter.next())).done; more = false) {
-          let x2 = temp.value;
-        }
-      } catch (temp) {
-        error = [temp];
-      } finally {
-        try {
-          more && (temp = iter.return) && (yield new __await(temp.call(iter)));
-        } finally {
-          if (error)
-            throw error[0];
-        }
-      }
-      try {
-        for (var iter2 = __forAwait(y), more2, temp2, error2; more2 = !(temp2 = yield new __await(iter2.next())).done; more2 = false) {
-          var _x = temp2.value;
-          var _stack = [];
-          try {
-            const x2 = __using(_stack, _x, true);
-          } catch (_) {
-            var _error = _, _hasError = true;
-          } finally {
-            var _promise = __callDispose(_stack, _error, _hasError);
-            _promise && (yield new __await(_promise));
-          }
-        }
-      } catch (temp2) {
-        error2 = [temp2];
-      } finally {
-        try {
-          more2 && (temp2 = iter2.return) && (yield new __await(temp2.call(iter2)));
-        } finally {
-          if (error2)
-            throw error2[0];
-        }
-      }
-    } catch (_2) {
-      var _error2 = _2, _hasError2 = true;
-    } finally {
-      var _promise2 = __callDispose(_stack2, _error2, _hasError2);
-      _promise2 && (yield new __await(_promise2));
-    }
-  });
+foo = async function* () {
+	yield;
+	yield x;
+	yield* x;
+	await using x = await y;
+	for await (let x$1 of y);
+	for await (await using x$1 of y);
 };
-foo = { bar() {
-  return __asyncGenerator(this, null, function* () {
-    var _stack2 = [];
-    try {
-      yield;
-      yield x;
-      yield* __yieldStar(x);
-      const x = __using(_stack2, yield new __await(y), true);
-      try {
-        for (var iter = __forAwait(y), more, temp, error; more = !(temp = yield new __await(iter.next())).done; more = false) {
-          let x2 = temp.value;
-        }
-      } catch (temp) {
-        error = [temp];
-      } finally {
-        try {
-          more && (temp = iter.return) && (yield new __await(temp.call(iter)));
-        } finally {
-          if (error)
-            throw error[0];
-        }
-      }
-      try {
-        for (var iter2 = __forAwait(y), more2, temp2, error2; more2 = !(temp2 = yield new __await(iter2.next())).done; more2 = false) {
-          var _x = temp2.value;
-          var _stack = [];
-          try {
-            const x2 = __using(_stack, _x, true);
-          } catch (_) {
-            var _error = _, _hasError = true;
-          } finally {
-            var _promise = __callDispose(_stack, _error, _hasError);
-            _promise && (yield new __await(_promise));
-          }
-        }
-      } catch (temp2) {
-        error2 = [temp2];
-      } finally {
-        try {
-          more2 && (temp2 = iter2.return) && (yield new __await(temp2.call(iter2)));
-        } finally {
-          if (error2)
-            throw error2[0];
-        }
-      }
-    } catch (_2) {
-      var _error2 = _2, _hasError2 = true;
-    } finally {
-      var _promise2 = __callDispose(_stack2, _error2, _hasError2);
-      _promise2 && (yield new __await(_promise2));
-    }
-  });
+foo = { async *bar() {
+	yield;
+	yield x;
+	yield* x;
+	await using x = await y;
+	for await (let x$1 of y);
+	for await (await using x$1 of y);
 } };
-class Foo {
-  bar() {
-    return __asyncGenerator(this, null, function* () {
-      var _stack2 = [];
-      try {
-        yield;
-        yield x;
-        yield* __yieldStar(x);
-        const x = __using(_stack2, yield new __await(y), true);
-        try {
-          for (var iter = __forAwait(y), more, temp, error; more = !(temp = yield new __await(iter.next())).done; more = false) {
-            let x2 = temp.value;
-          }
-        } catch (temp) {
-          error = [temp];
-        } finally {
-          try {
-            more && (temp = iter.return) && (yield new __await(temp.call(iter)));
-          } finally {
-            if (error)
-              throw error[0];
-          }
-        }
-        try {
-          for (var iter2 = __forAwait(y), more2, temp2, error2; more2 = !(temp2 = yield new __await(iter2.next())).done; more2 = false) {
-            var _x = temp2.value;
-            var _stack = [];
-            try {
-              const x2 = __using(_stack, _x, true);
-            } catch (_) {
-              var _error = _, _hasError = true;
-            } finally {
-              var _promise = __callDispose(_stack, _error, _hasError);
-              _promise && (yield new __await(_promise));
-            }
-          }
-        } catch (temp2) {
-          error2 = [temp2];
-        } finally {
-          try {
-            more2 && (temp2 = iter2.return) && (yield new __await(temp2.call(iter2)));
-          } finally {
-            if (error2)
-              throw error2[0];
-          }
-        }
-      } catch (_2) {
-        var _error2 = _2, _hasError2 = true;
-      } finally {
-        var _promise2 = __callDispose(_stack2, _error2, _hasError2);
-        _promise2 && (yield new __await(_promise2));
-      }
-    });
-  }
-}
+var Foo = class {
+	async *bar() {
+		yield;
+		yield x;
+		yield* x;
+		await using x = await y;
+		for await (let x$1 of y);
+		for await (await using x$1 of y);
+	}
+};
 Foo = class {
-  bar() {
-    return __asyncGenerator(this, null, function* () {
-      var _stack2 = [];
-      try {
-        yield;
-        yield x;
-        yield* __yieldStar(x);
-        const x = __using(_stack2, yield new __await(y), true);
-        try {
-          for (var iter = __forAwait(y), more, temp, error; more = !(temp = yield new __await(iter.next())).done; more = false) {
-            let x2 = temp.value;
-          }
-        } catch (temp) {
-          error = [temp];
-        } finally {
-          try {
-            more && (temp = iter.return) && (yield new __await(temp.call(iter)));
-          } finally {
-            if (error)
-              throw error[0];
-          }
-        }
-        try {
-          for (var iter2 = __forAwait(y), more2, temp2, error2; more2 = !(temp2 = yield new __await(iter2.next())).done; more2 = false) {
-            var _x = temp2.value;
-            var _stack = [];
-            try {
-              const x2 = __using(_stack, _x, true);
-            } catch (_) {
-              var _error = _, _hasError = true;
-            } finally {
-              var _promise = __callDispose(_stack, _error, _hasError);
-              _promise && (yield new __await(_promise));
-            }
-          }
-        } catch (temp2) {
-          error2 = [temp2];
-        } finally {
-          try {
-            more2 && (temp2 = iter2.return) && (yield new __await(temp2.call(iter2)));
-          } finally {
-            if (error2)
-              throw error2[0];
-          }
-        }
-      } catch (_2) {
-        var _error2 = _2, _hasError2 = true;
-      } finally {
-        var _promise2 = __callDispose(_stack2, _error2, _hasError2);
-        _promise2 && (yield new __await(_promise2));
-      }
-    });
-  }
+	async *bar() {
+		yield;
+		yield x;
+		yield* x;
+		await using x = await y;
+		for await (let x$1 of y);
+		for await (await using x$1 of y);
+	}
 };
-async function bar() {
-  await using x = await y;
-  for await (let x2 of y) {
-  }
-  for await (await using x2 of y) {
-  }
-}
\ No newline at end of file
+//#endregion

```