# Reason
1. not support const enum inline
# Diff
## /out.js
### esbuild
```js
var Foo = /* @__PURE__ */ ((Foo2) => {
  Foo2[Foo2["NAN"] = NaN] = "NAN";
  Foo2[Foo2["POS_INF"] = Infinity] = "POS_INF";
  Foo2[Foo2["NEG_INF"] = -Infinity] = "NEG_INF";
  return Foo2;
})(Foo || {});
//! It's ok to use "NaN" and "Infinity" here
console.log(
  NaN /* NAN */,
  Infinity /* POS_INF */,
  -Infinity /* NEG_INF */
);
checkPrecedence(
  1 / NaN /* NAN */,
  1 / Infinity /* POS_INF */,
  1 / -Infinity /* NEG_INF */
);
//! We must not use "NaN" or "Infinity" inside "with"
with (x) {
  console.log(
    0 / 0 /* NAN */,
    1 / 0 /* POS_INF */,
    -1 / 0 /* NEG_INF */
  );
  checkPrecedence(
    1 / (0 / 0) /* NAN */,
    1 / (1 / 0) /* POS_INF */,
    1 / (-1 / 0) /* NEG_INF */
  );
}
```
### rolldown
```js

//#region entry.ts
var Foo = /* @__PURE__ */ function(Foo$1) {
	Foo$1[Foo$1["NAN"] = NaN] = "NAN";
	Foo$1[Foo$1["POS_INF"] = Infinity] = "POS_INF";
	Foo$1[Foo$1["NEG_INF"] = -Infinity] = "NEG_INF";
	return Foo$1;
}(Foo || {});
//! It's ok to use "NaN" and "Infinity" here
console.log(Foo.NAN, Foo.POS_INF, Foo.NEG_INF);
checkPrecedence(1 / Foo.NAN, 1 / Foo.POS_INF, 1 / Foo.NEG_INF);
//! We must not use "NaN" or "Infinity" inside "with"
with(x) {
	console.log(Foo.NAN, Foo.POS_INF, Foo.NEG_INF);
	checkPrecedence(1 / Foo.NAN, 1 / Foo.POS_INF, 1 / Foo.NEG_INF);
}

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,30 +1,16 @@
-var Foo = /* @__PURE__ */ ((Foo2) => {
-  Foo2[Foo2["NAN"] = NaN] = "NAN";
-  Foo2[Foo2["POS_INF"] = Infinity] = "POS_INF";
-  Foo2[Foo2["NEG_INF"] = -Infinity] = "NEG_INF";
-  return Foo2;
-})(Foo || {});
+
+//#region entry.ts
+var Foo = /* @__PURE__ */ function(Foo$1) {
+	Foo$1[Foo$1["NAN"] = NaN] = "NAN";
+	Foo$1[Foo$1["POS_INF"] = Infinity] = "POS_INF";
+	Foo$1[Foo$1["NEG_INF"] = -Infinity] = "NEG_INF";
+	return Foo$1;
+}(Foo || {});
 //! It's ok to use "NaN" and "Infinity" here
-console.log(
-  NaN /* NAN */,
-  Infinity /* POS_INF */,
-  -Infinity /* NEG_INF */
-);
-checkPrecedence(
-  1 / NaN /* NAN */,
-  1 / Infinity /* POS_INF */,
-  1 / -Infinity /* NEG_INF */
-);
+console.log(Foo.NAN, Foo.POS_INF, Foo.NEG_INF);
+checkPrecedence(1 / Foo.NAN, 1 / Foo.POS_INF, 1 / Foo.NEG_INF);
 //! We must not use "NaN" or "Infinity" inside "with"
-with (x) {
-  console.log(
-    0 / 0 /* NAN */,
-    1 / 0 /* POS_INF */,
-    -1 / 0 /* NEG_INF */
-  );
-  checkPrecedence(
-    1 / (0 / 0) /* NAN */,
-    1 / (1 / 0) /* POS_INF */,
-    1 / (-1 / 0) /* NEG_INF */
-  );
-}
\ No newline at end of file
+with(x) {
+	console.log(Foo.NAN, Foo.POS_INF, Foo.NEG_INF);
+	checkPrecedence(1 / Foo.NAN, 1 / Foo.POS_INF, 1 / Foo.NEG_INF);
+}

```