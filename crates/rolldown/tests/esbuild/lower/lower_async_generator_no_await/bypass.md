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
function bar() {
  return __async(this, null, function* () {
    var _stack2 = [];
    try {
      const x = __using(_stack2, yield y, true);
      for await (let x2 of y) {
      }
      for await (var _x of y) {
        var _stack = [];
        try {
          const x2 = __using(_stack, _x, true);
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
//#region ../../../../../../node_modules/.pnpm/@oxc-project+runtime@0.76.0/node_modules/@oxc-project/runtime/src/helpers/esm/usingCtx.js
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
		d: function d() {
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
async function* foo() {
	try {
		var _usingCtx$1 = _usingCtx();
		yield;
		yield x;
		yield* x;
		const x = _usingCtx$1.a(await y);
		for await (let x$1 of y);
		for await (const _x of y) try {
			var _usingCtx3 = _usingCtx();
			const x$1 = _usingCtx3.a(_x);
		} catch (_) {
			_usingCtx3.e = _;
		} finally {
			await _usingCtx3.d();
		}
	} catch (_) {
		_usingCtx$1.e = _;
	} finally {
		await _usingCtx$1.d();
	}
}
foo = async function* () {
	try {
		var _usingCtx4 = _usingCtx();
		yield;
		yield x;
		yield* x;
		const x = _usingCtx4.a(await y);
		for await (let x$1 of y);
		for await (const _x2 of y) try {
			var _usingCtx5 = _usingCtx();
			const x$1 = _usingCtx5.a(_x2);
		} catch (_) {
			_usingCtx5.e = _;
		} finally {
			await _usingCtx5.d();
		}
	} catch (_) {
		_usingCtx4.e = _;
	} finally {
		await _usingCtx4.d();
	}
};
foo = { async *bar() {
	try {
		var _usingCtx6 = _usingCtx();
		yield;
		yield x;
		yield* x;
		const x = _usingCtx6.a(await y);
		for await (let x$1 of y);
		for await (const _x3 of y) try {
			var _usingCtx7 = _usingCtx();
			const x$1 = _usingCtx7.a(_x3);
		} catch (_) {
			_usingCtx7.e = _;
		} finally {
			await _usingCtx7.d();
		}
	} catch (_) {
		_usingCtx6.e = _;
	} finally {
		await _usingCtx6.d();
	}
} };
var Foo = class {
	async *bar() {
		try {
			var _usingCtx8 = _usingCtx();
			yield;
			yield x;
			yield* x;
			const x = _usingCtx8.a(await y);
			for await (let x$1 of y);
			for await (const _x4 of y) try {
				var _usingCtx9 = _usingCtx();
				const x$1 = _usingCtx9.a(_x4);
			} catch (_) {
				_usingCtx9.e = _;
			} finally {
				await _usingCtx9.d();
			}
		} catch (_) {
			_usingCtx8.e = _;
		} finally {
			await _usingCtx8.d();
		}
	}
};
Foo = class {
	async *bar() {
		try {
			var _usingCtx10 = _usingCtx();
			yield;
			yield x;
			yield* x;
			const x = _usingCtx10.a(await y);
			for await (let x$1 of y);
			for await (const _x5 of y) try {
				var _usingCtx11 = _usingCtx();
				const x$1 = _usingCtx11.a(_x5);
			} catch (_) {
				_usingCtx11.e = _;
			} finally {
				await _usingCtx11.d();
			}
		} catch (_) {
			_usingCtx10.e = _;
		} finally {
			await _usingCtx10.d();
		}
	}
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,295 +1,174 @@
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
+//#region ../../../../../../node_modules/.pnpm/@oxc-project+runtime@0.76.0/node_modules/@oxc-project/runtime/src/helpers/esm/usingCtx.js
+function _usingCtx() {
+	var r = "function" == typeof SuppressedError ? SuppressedError : function(r$1, e$1) {
+		var n$1 = Error();
+		return n$1.name = "SuppressedError", n$1.error = r$1, n$1.suppressed = e$1, n$1;
+	}, e = {}, n = [];
+	function using(r$1, e$1) {
+		if (null != e$1) {
+			if (Object(e$1) !== e$1) throw new TypeError("using declarations can only be used with objects, functions, null, or undefined.");
+			if (r$1) var o = e$1[Symbol.asyncDispose || Symbol["for"]("Symbol.asyncDispose")];
+			if (void 0 === o && (o = e$1[Symbol.dispose || Symbol["for"]("Symbol.dispose")], r$1)) var t = o;
+			if ("function" != typeof o) throw new TypeError("Object is not disposable.");
+			t && (o = function o$1() {
+				try {
+					t.call(e$1);
+				} catch (r$2) {
+					return Promise.reject(r$2);
+				}
+			}), n.push({
+				v: e$1,
+				d: o,
+				a: r$1
+			});
+		} else r$1 && n.push({
+			d: e$1,
+			a: r$1
+		});
+		return e$1;
+	}
+	return {
+		e,
+		u: using.bind(null, !1),
+		a: using.bind(null, !0),
+		d: function d() {
+			var o, t = this.e, s = 0;
+			function next() {
+				for (; o = n.pop();) try {
+					if (!o.a && 1 === s) return s = 0, n.push(o), Promise.resolve().then(next);
+					if (o.d) {
+						var r$1 = o.d.call(o.v);
+						if (o.a) return s |= 2, Promise.resolve(r$1).then(next, err);
+					} else s |= 1;
+				} catch (r$2) {
+					return err(r$2);
+				}
+				if (1 === s) return t !== e ? Promise.reject(t) : Promise.resolve();
+				if (t !== e) throw t;
+			}
+			function err(n$1) {
+				return t = t !== e ? new r(n$1, t) : n$1, next();
+			}
+			return next();
+		}
+	};
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
+
+//#endregion
+//#region entry.ts
+async function* foo() {
+	try {
+		var _usingCtx$1 = _usingCtx();
+		yield;
+		yield x;
+		yield* x;
+		const x = _usingCtx$1.a(await y);
+		for await (let x$1 of y);
+		for await (const _x of y) try {
+			var _usingCtx3 = _usingCtx();
+			const x$1 = _usingCtx3.a(_x);
+		} catch (_) {
+			_usingCtx3.e = _;
+		} finally {
+			await _usingCtx3.d();
+		}
+	} catch (_) {
+		_usingCtx$1.e = _;
+	} finally {
+		await _usingCtx$1.d();
+	}
+}
+foo = async function* () {
+	try {
+		var _usingCtx4 = _usingCtx();
+		yield;
+		yield x;
+		yield* x;
+		const x = _usingCtx4.a(await y);
+		for await (let x$1 of y);
+		for await (const _x2 of y) try {
+			var _usingCtx5 = _usingCtx();
+			const x$1 = _usingCtx5.a(_x2);
+		} catch (_) {
+			_usingCtx5.e = _;
+		} finally {
+			await _usingCtx5.d();
+		}
+	} catch (_) {
+		_usingCtx4.e = _;
+	} finally {
+		await _usingCtx4.d();
+	}
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
+	try {
+		var _usingCtx6 = _usingCtx();
+		yield;
+		yield x;
+		yield* x;
+		const x = _usingCtx6.a(await y);
+		for await (let x$1 of y);
+		for await (const _x3 of y) try {
+			var _usingCtx7 = _usingCtx();
+			const x$1 = _usingCtx7.a(_x3);
+		} catch (_) {
+			_usingCtx7.e = _;
+		} finally {
+			await _usingCtx7.d();
+		}
+	} catch (_) {
+		_usingCtx6.e = _;
+	} finally {
+		await _usingCtx6.d();
+	}
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
+		try {
+			var _usingCtx8 = _usingCtx();
+			yield;
+			yield x;
+			yield* x;
+			const x = _usingCtx8.a(await y);
+			for await (let x$1 of y);
+			for await (const _x4 of y) try {
+				var _usingCtx9 = _usingCtx();
+				const x$1 = _usingCtx9.a(_x4);
+			} catch (_) {
+				_usingCtx9.e = _;
+			} finally {
+				await _usingCtx9.d();
+			}
+		} catch (_) {
+			_usingCtx8.e = _;
+		} finally {
+			await _usingCtx8.d();
+		}
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
+		try {
+			var _usingCtx10 = _usingCtx();
+			yield;
+			yield x;
+			yield* x;
+			const x = _usingCtx10.a(await y);
+			for await (let x$1 of y);
+			for await (const _x5 of y) try {
+				var _usingCtx11 = _usingCtx();
+				const x$1 = _usingCtx11.a(_x5);
+			} catch (_) {
+				_usingCtx11.e = _;
+			} finally {
+				await _usingCtx11.d();
+			}
+		} catch (_) {
+			_usingCtx10.e = _;
+		} finally {
+			await _usingCtx10.d();
+		}
+	}
 };
-function bar() {
-  return __async(this, null, function* () {
-    var _stack2 = [];
-    try {
-      const x = __using(_stack2, yield y, true);
-      for await (let x2 of y) {
-      }
-      for await (var _x of y) {
-        var _stack = [];
-        try {
-          const x2 = __using(_stack, _x, true);
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
+
+//#endregion
\ No newline at end of file

```