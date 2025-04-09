# Diff
## /out.js
### esbuild
```js
const a = 1;
console.log(1), console.log(2), unknownFn(3);
for (const c = x; ; ) console.log(c);
for (const d in x) console.log(d);
for (const e of x) console.log(e);
```
### rolldown
```js

//#region bar.js
var bar$1 = class {
	static base = true;
};
var bar$ = class {
	static base = true;
};

//#endregion
//#region expression.js
{
	let barb = class extends bar$ {
		static test() {
			assert.ok(bar$.base);
		}
	};
	barb.test();
}
{
	let bar = class extends bar$1 {
		static test() {
			assert.ok(bar.base);
		}
	};
	assert.strictEqual(bar.name, "bar");
	bar.test();
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,23 @@
-const a = 1;
-(console.log(1), console.log(2), unknownFn(3));
-for (const c = x; ; ) console.log(c);
-for (const d in x) console.log(d);
-for (const e of x) console.log(e);
+var bar$1 = class {
+    static base = true;
+};
+var bar$ = class {
+    static base = true;
+};
+{
+    let barb = class extends bar$ {
+        static test() {
+            assert.ok(bar$.base);
+        }
+    };
+    barb.test();
+}
+{
+    let bar = class extends bar$1 {
+        static test() {
+            assert.ok(bar.base);
+        }
+    };
+    assert.strictEqual(bar.name, "bar");
+    bar.test();
+}

```