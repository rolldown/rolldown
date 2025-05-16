# Diff
## /out.js
### esbuild
```js
// foo1.js
var foo1_default = class _foo1_default extends x {
  static foo1() {
    return () => __async(this, null, function* () {
      return __superGet(_foo1_default, this, "foo").call(this, "foo1");
    });
  }
};

// foo2.js
var foo2_default = class _foo2_default extends x {
  static foo2() {
    return () => __async(this, null, function* () {
      return () => __superGet(_foo2_default, this, "foo").call(this, "foo2");
    });
  }
};

// foo3.js
var foo3_default = class _foo3_default extends x {
  static foo3() {
    return () => () => __async(this, null, function* () {
      return __superGet(_foo3_default, this, "foo").call(this, "foo3");
    });
  }
};

// foo4.js
var foo4_default = class _foo4_default extends x {
  static foo4() {
    return () => __async(this, null, function* () {
      return () => __async(this, null, function* () {
        return __superGet(_foo4_default, this, "foo").call(this, "foo4");
      });
    });
  }
};

// bar1.js
var _bar1_default = class _bar1_default extends x {
};
__publicField(_bar1_default, "bar1", () => __async(_bar1_default, null, function* () {
  return __superGet(_bar1_default, _bar1_default, "foo").call(this, "bar1");
}));
var bar1_default = _bar1_default;

// bar2.js
var _bar2_default = class _bar2_default extends x {
};
__publicField(_bar2_default, "bar2", () => __async(_bar2_default, null, function* () {
  return () => __superGet(_bar2_default, _bar2_default, "foo").call(this, "bar2");
}));
var bar2_default = _bar2_default;

// bar3.js
var _bar3_default = class _bar3_default extends x {
};
__publicField(_bar3_default, "bar3", () => () => __async(_bar3_default, null, function* () {
  return __superGet(_bar3_default, _bar3_default, "foo").call(this, "bar3");
}));
var bar3_default = _bar3_default;

// bar4.js
var _bar4_default = class _bar4_default extends x {
};
__publicField(_bar4_default, "bar4", () => __async(_bar4_default, null, function* () {
  return () => __async(_bar4_default, null, function* () {
    return __superGet(_bar4_default, _bar4_default, "foo").call(this, "bar4");
  });
}));
var bar4_default = _bar4_default;

// baz1.js
var baz1_default = class _baz1_default extends x {
  static baz1() {
    return __async(this, null, function* () {
      return () => __superGet(_baz1_default, this, "foo").call(this, "baz1");
    });
  }
};

// baz2.js
var baz2_default = class _baz2_default extends x {
  static baz2() {
    return __async(this, null, function* () {
      return () => () => __superGet(_baz2_default, this, "foo").call(this, "baz2");
    });
  }
};

// outer.js
var outer_default = function() {
  return __async(this, null, function* () {
    const _y = class _y extends z {
    };
    __publicField(_y, "foo", () => __async(_y, null, function* () {
      return __superGet(_y, _y, "foo").call(this);
    }));
    let y = _y;
    yield y.foo()();
  });
}();
export {
  bar1_default as bar1,
  bar2_default as bar2,
  bar3_default as bar3,
  bar4_default as bar4,
  baz1_default as baz1,
  baz2_default as baz2,
  foo1_default as foo1,
  foo2_default as foo2,
  foo3_default as foo3,
  foo4_default as foo4
};
```
### rolldown
```js
//#region foo1.js
var foo1_default = class extends x {
	static foo1() {
		return async () => super.foo("foo1");
	}
};

//#endregion
//#region foo2.js
var foo2_default = class extends x {
	static foo2() {
		return async () => () => super.foo("foo2");
	}
};

//#endregion
//#region foo3.js
var foo3_default = class extends x {
	static foo3() {
		return () => async () => super.foo("foo3");
	}
};

//#endregion
//#region foo4.js
var foo4_default = class extends x {
	static foo4() {
		return async () => async () => super.foo("foo4");
	}
};

//#endregion
//#region bar1.js
var bar1_default = class extends x {
	static bar1 = async () => super.foo("bar1");
};

//#endregion
//#region bar2.js
var bar2_default = class extends x {
	static bar2 = async () => () => super.foo("bar2");
};

//#endregion
//#region bar3.js
var bar3_default = class extends x {
	static bar3 = () => async () => super.foo("bar3");
};

//#endregion
//#region bar4.js
var bar4_default = class extends x {
	static bar4 = async () => async () => super.foo("bar4");
};

//#endregion
//#region baz1.js
var baz1_default = class extends x {
	static async baz1() {
		return () => super.foo("baz1");
	}
};

//#endregion
//#region baz2.js
var baz2_default = class extends x {
	static async baz2() {
		return () => () => super.foo("baz2");
	}
};

//#endregion
//#region outer.js
var outer_default = async function() {
	class y extends z {
		static foo = async () => super.foo();
	}
	await y.foo()();
}();

//#endregion
export { bar1_default as bar1, bar2_default as bar2, bar3_default as bar3, bar4_default as bar4, baz1_default as baz1, baz2_default as baz2, foo1_default as foo1, foo2_default as foo2, foo3_default as foo3, foo4_default as foo4 };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,77 +1,49 @@
-var foo1_default = class _foo1_default extends x {
+var foo1_default = class extends x {
     static foo1() {
-        return () => __async(this, null, function* () {
-            return __superGet(_foo1_default, this, "foo").call(this, "foo1");
-        });
+        return async () => super.foo("foo1");
     }
 };
-var foo2_default = class _foo2_default extends x {
+var foo2_default = class extends x {
     static foo2() {
-        return () => __async(this, null, function* () {
-            return () => __superGet(_foo2_default, this, "foo").call(this, "foo2");
-        });
+        return async () => () => super.foo("foo2");
     }
 };
-var foo3_default = class _foo3_default extends x {
+var foo3_default = class extends x {
     static foo3() {
-        return () => () => __async(this, null, function* () {
-            return __superGet(_foo3_default, this, "foo").call(this, "foo3");
-        });
+        return () => async () => super.foo("foo3");
     }
 };
-var foo4_default = class _foo4_default extends x {
+var foo4_default = class extends x {
     static foo4() {
-        return () => __async(this, null, function* () {
-            return () => __async(this, null, function* () {
-                return __superGet(_foo4_default, this, "foo").call(this, "foo4");
-            });
-        });
+        return async () => async () => super.foo("foo4");
     }
 };
-var _bar1_default = class _bar1_default extends x {};
-__publicField(_bar1_default, "bar1", () => __async(_bar1_default, null, function* () {
-    return __superGet(_bar1_default, _bar1_default, "foo").call(this, "bar1");
-}));
-var bar1_default = _bar1_default;
-var _bar2_default = class _bar2_default extends x {};
-__publicField(_bar2_default, "bar2", () => __async(_bar2_default, null, function* () {
-    return () => __superGet(_bar2_default, _bar2_default, "foo").call(this, "bar2");
-}));
-var bar2_default = _bar2_default;
-var _bar3_default = class _bar3_default extends x {};
-__publicField(_bar3_default, "bar3", () => () => __async(_bar3_default, null, function* () {
-    return __superGet(_bar3_default, _bar3_default, "foo").call(this, "bar3");
-}));
-var bar3_default = _bar3_default;
-var _bar4_default = class _bar4_default extends x {};
-__publicField(_bar4_default, "bar4", () => __async(_bar4_default, null, function* () {
-    return () => __async(_bar4_default, null, function* () {
-        return __superGet(_bar4_default, _bar4_default, "foo").call(this, "bar4");
-    });
-}));
-var bar4_default = _bar4_default;
-var baz1_default = class _baz1_default extends x {
-    static baz1() {
-        return __async(this, null, function* () {
-            return () => __superGet(_baz1_default, this, "foo").call(this, "baz1");
-        });
+var bar1_default = class extends x {
+    static bar1 = async () => super.foo("bar1");
+};
+var bar2_default = class extends x {
+    static bar2 = async () => () => super.foo("bar2");
+};
+var bar3_default = class extends x {
+    static bar3 = () => async () => super.foo("bar3");
+};
+var bar4_default = class extends x {
+    static bar4 = async () => async () => super.foo("bar4");
+};
+var baz1_default = class extends x {
+    static async baz1() {
+        return () => super.foo("baz1");
     }
 };
-var baz2_default = class _baz2_default extends x {
-    static baz2() {
-        return __async(this, null, function* () {
-            return () => () => __superGet(_baz2_default, this, "foo").call(this, "baz2");
-        });
+var baz2_default = class extends x {
+    static async baz2() {
+        return () => () => super.foo("baz2");
     }
 };
-var outer_default = (function () {
-    return __async(this, null, function* () {
-        const _y = class _y extends z {};
-        __publicField(_y, "foo", () => __async(_y, null, function* () {
-            return __superGet(_y, _y, "foo").call(this);
-        }));
-        let y = _y;
-        yield y.foo()();
-    });
+var outer_default = (async function () {
+    class y extends z {
+        static foo = async () => super.foo();
+    }
+    await y.foo()();
 })();
 export {bar1_default as bar1, bar2_default as bar2, bar3_default as bar3, bar4_default as bar4, baz1_default as baz1, baz2_default as baz2, foo1_default as foo1, foo2_default as foo2, foo3_default as foo3, foo4_default as foo4};

```