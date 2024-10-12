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
	console.log(this);
	console.log((x = this) => this);
	console.log({ x: this });
	console.log(class extends this.foo {});
	console.log(class {
		[this.foo];
	});
	console.log(class {
		[this.foo]() {}
	});
	console.log(class {
		static [this.foo];
	});
	console.log(class {
		static [this.foo]() {}
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

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,43 +1,38 @@
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
+if (shouldBeExportsNotThis) {
+    console.log(this);
+    console.log((x = this) => this);
+    console.log({
+        x: this
+    });
+    console.log(class extends this.foo {});
+    console.log(class {
+        [this.foo];
+    });
+    console.log(class {
+        [this.foo]() {}
+    });
+    console.log(class {
+        static [this.foo];
+    });
+    console.log(class {
+        static [this.foo]() {}
+    });
+}
+if (shouldBeThisNotExports) {
+    console.log(class {
+        foo = this;
+    });
+    console.log(class {
+        foo() {
+            this;
         }
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
+    });
+    console.log(class {
+        static foo = this;
+    });
+    console.log(class {
+        static foo() {
+            this;
         }
-    }
-});
-export default require_entry();
+    });
+}

```