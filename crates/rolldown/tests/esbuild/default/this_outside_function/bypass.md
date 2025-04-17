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

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports) {
	if (shouldBeExportsNotThis) {
		console.log(exports);
		console.log((x = exports) => exports);
		console.log({ x: exports });
		console.log(class extends exports.foo {});
		console.log(class {
			[exports.foo];
		});
		console.log(class {
			[exports.foo]() {}
		});
		console.log(class {
			static [exports.foo];
		});
		console.log(class {
			static [exports.foo]() {}
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
} });

export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,10 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
 var require_entry = __commonJS({
     "entry.js"(exports) {
         if (shouldBeExportsNotThis) {
             console.log(exports);
@@ -24,19 +30,15 @@
             console.log(class {
                 foo = this;
             });
             console.log(class {
-                foo() {
-                    this;
-                }
+                foo() {}
             });
             console.log(class {
                 static foo = this;
             });
             console.log(class {
-                static foo() {
-                    this;
-                }
+                static foo() {}
             });
         }
     }
 });

```