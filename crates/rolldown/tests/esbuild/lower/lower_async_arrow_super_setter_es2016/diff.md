# Diff
## /out.js
### esbuild
```js
// foo1.js
var foo1_default = class _foo1_default extends x {
  foo1() {
    return () => __async(this, null, function* () {
      return __superSet(_foo1_default.prototype, this, "foo", "foo1");
    });
  }
};

// foo2.js
var foo2_default = class _foo2_default extends x {
  foo2() {
    return () => __async(this, null, function* () {
      return () => __superSet(_foo2_default.prototype, this, "foo", "foo2");
    });
  }
};

// foo3.js
var foo3_default = class _foo3_default extends x {
  foo3() {
    return () => () => __async(this, null, function* () {
      return __superSet(_foo3_default.prototype, this, "foo", "foo3");
    });
  }
};

// foo4.js
var foo4_default = class _foo4_default extends x {
  foo4() {
    return () => __async(this, null, function* () {
      return () => __async(this, null, function* () {
        return __superSet(_foo4_default.prototype, this, "foo", "foo4");
      });
    });
  }
};

// bar1.js
var bar1_default = class _bar1_default extends x {
  constructor() {
    super(...arguments);
    __publicField(this, "bar1", () => __async(this, null, function* () {
      return __superSet(_bar1_default.prototype, this, "foo", "bar1");
    }));
  }
};

// bar2.js
var bar2_default = class _bar2_default extends x {
  constructor() {
    super(...arguments);
    __publicField(this, "bar2", () => __async(this, null, function* () {
      return () => __superSet(_bar2_default.prototype, this, "foo", "bar2");
    }));
  }
};

// bar3.js
var bar3_default = class _bar3_default extends x {
  constructor() {
    super(...arguments);
    __publicField(this, "bar3", () => () => __async(this, null, function* () {
      return __superSet(_bar3_default.prototype, this, "foo", "bar3");
    }));
  }
};

// bar4.js
var bar4_default = class _bar4_default extends x {
  constructor() {
    super(...arguments);
    __publicField(this, "bar4", () => __async(this, null, function* () {
      return () => __async(this, null, function* () {
        return __superSet(_bar4_default.prototype, this, "foo", "bar4");
      });
    }));
  }
};

// baz1.js
var baz1_default = class _baz1_default extends x {
  baz1() {
    return __async(this, null, function* () {
      return () => __superSet(_baz1_default.prototype, this, "foo", "baz1");
    });
  }
};

// baz2.js
var baz2_default = class _baz2_default extends x {
  baz2() {
    return __async(this, null, function* () {
      return () => () => __superSet(_baz2_default.prototype, this, "foo", "baz2");
    });
  }
};

// outer.js
var outer_default = function() {
  return __async(this, null, function* () {
    class y extends z {
      constructor() {
        super(...arguments);
        __publicField(this, "foo", () => __async(this, null, function* () {
          return __superSet(y.prototype, this, "foo", "foo");
        }));
      }
    }
    yield new y().foo()();
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
	foo1() {
		return async () => super.foo = "foo1";
	}
};

//#endregion
//#region foo2.js
var foo2_default = class extends x {
	foo2() {
		return async () => () => super.foo = "foo2";
	}
};

//#endregion
//#region foo3.js
var foo3_default = class extends x {
	foo3() {
		return () => async () => super.foo = "foo3";
	}
};

//#endregion
//#region foo4.js
var foo4_default = class extends x {
	foo4() {
		return async () => async () => super.foo = "foo4";
	}
};

//#endregion
//#region bar1.js
var bar1_default = class extends x {
	bar1 = async () => super.foo = "bar1";
};

//#endregion
//#region bar2.js
var bar2_default = class extends x {
	bar2 = async () => () => super.foo = "bar2";
};

//#endregion
//#region bar3.js
var bar3_default = class extends x {
	bar3 = () => async () => super.foo = "bar3";
};

//#endregion
//#region bar4.js
var bar4_default = class extends x {
	bar4 = async () => async () => super.foo = "bar4";
};

//#endregion
//#region baz1.js
var baz1_default = class extends x {
	async baz1() {
		return () => super.foo = "baz1";
	}
};

//#endregion
//#region baz2.js
var baz2_default = class extends x {
	async baz2() {
		return () => () => super.foo = "baz2";
	}
};

//#endregion
//#region outer.js
var outer_default = async function() {
	class y extends z {
		foo = async () => super.foo = "foo";
	}
	await new y().foo()();
}();

//#endregion
export { bar1_default as bar1, bar2_default as bar2, bar3_default as bar3, bar4_default as bar4, baz1_default as baz1, baz2_default as baz2, foo1_default as foo1, foo2_default as foo2, foo3_default as foo3, foo4_default as foo4 };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,92 +1,49 @@
-var foo1_default = class _foo1_default extends x {
+var foo1_default = class extends x {
     foo1() {
-        return () => __async(this, null, function* () {
-            return __superSet(_foo1_default.prototype, this, "foo", "foo1");
-        });
+        return async () => super.foo = "foo1";
     }
 };
-var foo2_default = class _foo2_default extends x {
+var foo2_default = class extends x {
     foo2() {
-        return () => __async(this, null, function* () {
-            return () => __superSet(_foo2_default.prototype, this, "foo", "foo2");
-        });
+        return async () => () => super.foo = "foo2";
     }
 };
-var foo3_default = class _foo3_default extends x {
+var foo3_default = class extends x {
     foo3() {
-        return () => () => __async(this, null, function* () {
-            return __superSet(_foo3_default.prototype, this, "foo", "foo3");
-        });
+        return () => async () => super.foo = "foo3";
     }
 };
-var foo4_default = class _foo4_default extends x {
+var foo4_default = class extends x {
     foo4() {
-        return () => __async(this, null, function* () {
-            return () => __async(this, null, function* () {
-                return __superSet(_foo4_default.prototype, this, "foo", "foo4");
-            });
-        });
+        return async () => async () => super.foo = "foo4";
     }
 };
-var bar1_default = class _bar1_default extends x {
-    constructor() {
-        super(...arguments);
-        __publicField(this, "bar1", () => __async(this, null, function* () {
-            return __superSet(_bar1_default.prototype, this, "foo", "bar1");
-        }));
-    }
+var bar1_default = class extends x {
+    bar1 = async () => super.foo = "bar1";
 };
-var bar2_default = class _bar2_default extends x {
-    constructor() {
-        super(...arguments);
-        __publicField(this, "bar2", () => __async(this, null, function* () {
-            return () => __superSet(_bar2_default.prototype, this, "foo", "bar2");
-        }));
-    }
+var bar2_default = class extends x {
+    bar2 = async () => () => super.foo = "bar2";
 };
-var bar3_default = class _bar3_default extends x {
-    constructor() {
-        super(...arguments);
-        __publicField(this, "bar3", () => () => __async(this, null, function* () {
-            return __superSet(_bar3_default.prototype, this, "foo", "bar3");
-        }));
-    }
+var bar3_default = class extends x {
+    bar3 = () => async () => super.foo = "bar3";
 };
-var bar4_default = class _bar4_default extends x {
-    constructor() {
-        super(...arguments);
-        __publicField(this, "bar4", () => __async(this, null, function* () {
-            return () => __async(this, null, function* () {
-                return __superSet(_bar4_default.prototype, this, "foo", "bar4");
-            });
-        }));
-    }
+var bar4_default = class extends x {
+    bar4 = async () => async () => super.foo = "bar4";
 };
-var baz1_default = class _baz1_default extends x {
-    baz1() {
-        return __async(this, null, function* () {
-            return () => __superSet(_baz1_default.prototype, this, "foo", "baz1");
-        });
+var baz1_default = class extends x {
+    async baz1() {
+        return () => super.foo = "baz1";
     }
 };
-var baz2_default = class _baz2_default extends x {
-    baz2() {
-        return __async(this, null, function* () {
-            return () => () => __superSet(_baz2_default.prototype, this, "foo", "baz2");
-        });
+var baz2_default = class extends x {
+    async baz2() {
+        return () => () => super.foo = "baz2";
     }
 };
-var outer_default = (function () {
-    return __async(this, null, function* () {
-        class y extends z {
-            constructor() {
-                super(...arguments);
-                __publicField(this, "foo", () => __async(this, null, function* () {
-                    return __superSet(y.prototype, this, "foo", "foo");
-                }));
-            }
-        }
-        yield new y().foo()();
-    });
+var outer_default = (async function () {
+    class y extends z {
+        foo = async () => super.foo = "foo";
+    }
+    await new y().foo()();
 })();
 export {bar1_default as bar1, bar2_default as bar2, bar3_default as bar3, bar4_default as bar4, baz1_default as baz1, baz2_default as baz2, foo1_default as foo1, foo2_default as foo2, foo3_default as foo3, foo4_default as foo4};

```