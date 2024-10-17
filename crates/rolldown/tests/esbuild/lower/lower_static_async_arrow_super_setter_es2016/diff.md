# Diff
## /out.js
### esbuild
```js
// foo1.js
var foo1_default = class _foo1_default extends x {
  static foo1() {
    return () => __async(this, null, function* () {
      return __superSet(_foo1_default, this, "foo", "foo1");
    });
  }
};

// foo2.js
var foo2_default = class _foo2_default extends x {
  static foo2() {
    return () => __async(this, null, function* () {
      return () => __superSet(_foo2_default, this, "foo", "foo2");
    });
  }
};

// foo3.js
var foo3_default = class _foo3_default extends x {
  static foo3() {
    return () => () => __async(this, null, function* () {
      return __superSet(_foo3_default, this, "foo", "foo3");
    });
  }
};

// foo4.js
var foo4_default = class _foo4_default extends x {
  static foo4() {
    return () => __async(this, null, function* () {
      return () => __async(this, null, function* () {
        return __superSet(_foo4_default, this, "foo", "foo4");
      });
    });
  }
};

// bar1.js
var _bar1_default = class _bar1_default extends x {
};
__publicField(_bar1_default, "bar1", () => __async(_bar1_default, null, function* () {
  return __superSet(_bar1_default, _bar1_default, "foo", "bar1");
}));
var bar1_default = _bar1_default;

// bar2.js
var _bar2_default = class _bar2_default extends x {
};
__publicField(_bar2_default, "bar2", () => __async(_bar2_default, null, function* () {
  return () => __superSet(_bar2_default, _bar2_default, "foo", "bar2");
}));
var bar2_default = _bar2_default;

// bar3.js
var _bar3_default = class _bar3_default extends x {
};
__publicField(_bar3_default, "bar3", () => () => __async(_bar3_default, null, function* () {
  return __superSet(_bar3_default, _bar3_default, "foo", "bar3");
}));
var bar3_default = _bar3_default;

// bar4.js
var _bar4_default = class _bar4_default extends x {
};
__publicField(_bar4_default, "bar4", () => __async(_bar4_default, null, function* () {
  return () => __async(_bar4_default, null, function* () {
    return __superSet(_bar4_default, _bar4_default, "foo", "bar4");
  });
}));
var bar4_default = _bar4_default;

// baz1.js
var baz1_default = class _baz1_default extends x {
  static baz1() {
    return __async(this, null, function* () {
      return () => __superSet(_baz1_default, this, "foo", "baz1");
    });
  }
};

// baz2.js
var baz2_default = class _baz2_default extends x {
  static baz2() {
    return __async(this, null, function* () {
      return () => () => __superSet(_baz2_default, this, "foo", "baz2");
    });
  }
};

// outer.js
var outer_default = function() {
  return __async(this, null, function* () {
    const _y = class _y extends z {
    };
    __publicField(_y, "foo", () => __async(_y, null, function* () {
      return __superSet(_y, _y, "foo", "foo");
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
class foo1_default extends x {
	static foo1() {
		return async () => super.foo = "foo1";
	}
}

//#endregion
//#region foo2.js
class foo2_default extends x {
	static foo2() {
		return async () => () => super.foo = "foo2";
	}
}

//#endregion
//#region foo3.js
class foo3_default extends x {
	static foo3() {
		return () => async () => super.foo = "foo3";
	}
}

//#endregion
//#region foo4.js
class foo4_default extends x {
	static foo4() {
		return async () => async () => super.foo = "foo4";
	}
}

//#endregion
//#region bar1.js
class bar1_default extends x {
	static bar1 = async () => super.foo = "bar1";
}

//#endregion
//#region bar2.js
class bar2_default extends x {
	static bar2 = async () => () => super.foo = "bar2";
}

//#endregion
//#region bar3.js
class bar3_default extends x {
	static bar3 = () => async () => super.foo = "bar3";
}

//#endregion
//#region bar4.js
class bar4_default extends x {
	static bar4 = async () => async () => super.foo = "bar4";
}

//#endregion
//#region baz1.js
class baz1_default extends x {
	static async baz1() {
		return () => super.foo = "baz1";
	}
}

//#endregion
//#region baz2.js
class baz2_default extends x {
	static async baz2() {
		return () => () => super.foo = "baz2";
	}
}

//#endregion
//#region outer.js
var outer_default = async function() {
	class y extends z {
		static foo = async () => super.foo = "foo";
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
+class foo1_default extends x {
     static foo1() {
-        return () => __async(this, null, function* () {
-            return __superSet(_foo1_default, this, "foo", "foo1");
-        });
+        return async () => super.foo = "foo1";
     }
-};
-var foo2_default = class _foo2_default extends x {
+}
+class foo2_default extends x {
     static foo2() {
-        return () => __async(this, null, function* () {
-            return () => __superSet(_foo2_default, this, "foo", "foo2");
-        });
+        return async () => () => super.foo = "foo2";
     }
-};
-var foo3_default = class _foo3_default extends x {
+}
+class foo3_default extends x {
     static foo3() {
-        return () => () => __async(this, null, function* () {
-            return __superSet(_foo3_default, this, "foo", "foo3");
-        });
+        return () => async () => super.foo = "foo3";
     }
-};
-var foo4_default = class _foo4_default extends x {
+}
+class foo4_default extends x {
     static foo4() {
-        return () => __async(this, null, function* () {
-            return () => __async(this, null, function* () {
-                return __superSet(_foo4_default, this, "foo", "foo4");
-            });
-        });
+        return async () => async () => super.foo = "foo4";
     }
-};
-var _bar1_default = class _bar1_default extends x {};
-__publicField(_bar1_default, "bar1", () => __async(_bar1_default, null, function* () {
-    return __superSet(_bar1_default, _bar1_default, "foo", "bar1");
-}));
-var bar1_default = _bar1_default;
-var _bar2_default = class _bar2_default extends x {};
-__publicField(_bar2_default, "bar2", () => __async(_bar2_default, null, function* () {
-    return () => __superSet(_bar2_default, _bar2_default, "foo", "bar2");
-}));
-var bar2_default = _bar2_default;
-var _bar3_default = class _bar3_default extends x {};
-__publicField(_bar3_default, "bar3", () => () => __async(_bar3_default, null, function* () {
-    return __superSet(_bar3_default, _bar3_default, "foo", "bar3");
-}));
-var bar3_default = _bar3_default;
-var _bar4_default = class _bar4_default extends x {};
-__publicField(_bar4_default, "bar4", () => __async(_bar4_default, null, function* () {
-    return () => __async(_bar4_default, null, function* () {
-        return __superSet(_bar4_default, _bar4_default, "foo", "bar4");
-    });
-}));
-var bar4_default = _bar4_default;
-var baz1_default = class _baz1_default extends x {
-    static baz1() {
-        return __async(this, null, function* () {
-            return () => __superSet(_baz1_default, this, "foo", "baz1");
-        });
+}
+class bar1_default extends x {
+    static bar1 = async () => super.foo = "bar1";
+}
+class bar2_default extends x {
+    static bar2 = async () => () => super.foo = "bar2";
+}
+class bar3_default extends x {
+    static bar3 = () => async () => super.foo = "bar3";
+}
+class bar4_default extends x {
+    static bar4 = async () => async () => super.foo = "bar4";
+}
+class baz1_default extends x {
+    static async baz1() {
+        return () => super.foo = "baz1";
     }
-};
-var baz2_default = class _baz2_default extends x {
-    static baz2() {
-        return __async(this, null, function* () {
-            return () => () => __superSet(_baz2_default, this, "foo", "baz2");
-        });
+}
+class baz2_default extends x {
+    static async baz2() {
+        return () => () => super.foo = "baz2";
     }
-};
-var outer_default = (function () {
-    return __async(this, null, function* () {
-        const _y = class _y extends z {};
-        __publicField(_y, "foo", () => __async(_y, null, function* () {
-            return __superSet(_y, _y, "foo", "foo");
-        }));
-        let y = _y;
-        yield y.foo()();
-    });
+}
+var outer_default = (async function () {
+    class y extends z {
+        static foo = async () => super.foo = "foo";
+    }
+    await y.foo()();
 })();
 export {bar1_default as bar1, bar2_default as bar2, bar3_default as bar3, bar4_default as bar4, baz1_default as baz1, baz2_default as baz2, foo1_default as foo1, foo2_default as foo2, foo3_default as foo3, foo4_default as foo4};

```