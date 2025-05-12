# Diff
## /out.js
### esbuild
```js
// entry.js
var require_entry = __commonJS({
  "entry.js"(exports) {
    if (shouldBeExportsNotThis) {
      console.log(exports);
      console.log((x = exports) => exports);
      console.log({ x: exports });
      console.log(class extends exports.foo {
      });
      console.log(class {
        [exports.foo];
      });
      console.log(class {
        [exports.foo]() {
        }
      });
      console.log(class {
        static [exports.foo];
      });
      console.log(class {
        static [exports.foo]() {
        }
      });
    }
    if (shouldBeThisNotExports) {
      console.log(class {
        foo = this;
      });
      console.log(class {
        foo() {
          this;
        }
      });
      console.log(class {
        static foo = this;
      });
      console.log(class {
        static foo() {
          this;
        }
      });
    }
  }
});
export default require_entry();
```
### rolldown
```js
//#region entry.js
if (shouldBeExportsNotThis) {
	console.log(void 0);
	console.log((x = void 0) => void 0);
	console.log({ x: void 0 });
	console.log(class extends (void 0).foo {});
	console.log(class {
		[(void 0).foo];
	});
	console.log(class {
		[(void 0).foo]() {}
	});
	console.log(class {
		static [(void 0).foo];
	});
	console.log(class {
		static [(void 0).foo]() {}
	});
}
if (shouldBeThisNotExports) {
	console.log(class {
		foo = this;
	});
	console.log(class {
		foo() {}
	});
	console.log(class {
		static foo = this;
	});
	console.log(class {
		static foo() {}
	});
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,43 +1,34 @@
-var require_entry = __commonJS({
-    "entry.js"(exports) {
-        if (shouldBeExportsNotThis) {
-            console.log(exports);
-            console.log((x = exports) => exports);
-            console.log({
-                x: exports
-            });
-            console.log(class extends exports.foo {});
-            console.log(class {
-                [exports.foo];
-            });
-            console.log(class {
-                [exports.foo]() {}
-            });
-            console.log(class {
-                static [exports.foo];
-            });
-            console.log(class {
-                static [exports.foo]() {}
-            });
-        }
-        if (shouldBeThisNotExports) {
-            console.log(class {
-                foo = this;
-            });
-            console.log(class {
-                foo() {
-                    this;
-                }
-            });
-            console.log(class {
-                static foo = this;
-            });
-            console.log(class {
-                static foo() {
-                    this;
-                }
-            });
-        }
-    }
-});
-export default require_entry();
+if (shouldBeExportsNotThis) {
+    console.log(void 0);
+    console.log((x = void 0) => void 0);
+    console.log({
+        x: void 0
+    });
+    console.log(class extends (void 0).foo {});
+    console.log(class {
+        [(void 0).foo];
+    });
+    console.log(class {
+        [(void 0).foo]() {}
+    });
+    console.log(class {
+        static [(void 0).foo];
+    });
+    console.log(class {
+        static [(void 0).foo]() {}
+    });
+}
+if (shouldBeThisNotExports) {
+    console.log(class {
+        foo = this;
+    });
+    console.log(class {
+        foo() {}
+    });
+    console.log(class {
+        static foo = this;
+    });
+    console.log(class {
+        static foo() {}
+    });
+}

```