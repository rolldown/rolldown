# Diff
## /out.js
### esbuild
```js
// entry.js
function bar() {
}
var bare = foo(bar);
var at_no = /* @__PURE__ */ foo(bar());
var new_at_no = /* @__PURE__ */ new foo(bar());
var nospace_at_no = /* @__PURE__ */ foo(bar());
var nospace_new_at_no = /* @__PURE__ */ new foo(bar());
var num_no = /* @__PURE__ */ foo(bar());
var new_num_no = /* @__PURE__ */ new foo(bar());
var nospace_num_no = /* @__PURE__ */ foo(bar());
var nospace_new_num_no = /* @__PURE__ */ new foo(bar());
var dot_no = /* @__PURE__ */ foo(sideEffect()).dot(bar());
var new_dot_no = /* @__PURE__ */ new foo(sideEffect()).dot(bar());
var nested_no = [1, /* @__PURE__ */ foo(bar()), 2];
var new_nested_no = [1, /* @__PURE__ */ new foo(bar()), 2];
var single_at_no = /* @__PURE__ */ foo(bar());
var new_single_at_no = /* @__PURE__ */ new foo(bar());
var single_num_no = /* @__PURE__ */ foo(bar());
var new_single_num_no = /* @__PURE__ */ new foo(bar());
var bad_no = (
  /* __PURE__ */
  foo(bar)
);
var new_bad_no = (
  /* __PURE__ */
  new foo(bar)
);
var parens_no = foo(bar);
var new_parens_no = new foo(bar);
var exp_no = /* @__PURE__ */ foo() ** foo();
var new_exp_no = /* @__PURE__ */ new foo() ** foo();
```
### rolldown
```js
//#region entry.js
function bar() {}
foo(bar);
/* @__PURE__ */ foo(bar());
/* @__PURE__ */ new foo(bar());
/* @__PURE__ */ foo(bar());
/* @__PURE__ */ new foo(bar());
/* @__PURE__ */ foo(bar());
/* @__PURE__ */ new foo(bar());
/* @__PURE__ */ foo(bar());
/* @__PURE__ */ new foo(bar());
/* @__PURE__ */ foo(sideEffect()).dot(bar());
/* @__PURE__ */ new foo(sideEffect()).dot(bar());
[
	1,
	/* @__PURE__ */ foo(bar()),
	2
];
[
	1,
	/* @__PURE__ */ new foo(bar()),
	2
];
/* @__PURE__ */ foo(bar());
/* @__PURE__ */ new foo(bar());
/* @__PURE__ */ foo(bar());
/* @__PURE__ */ new foo(bar());
foo(bar);
new foo(bar);
foo(bar);
new foo(bar);
/* @__PURE__ */ foo() ** foo();
/* @__PURE__ */ new foo() ** foo();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,24 +1,24 @@
 function bar() {}
-var bare = foo(bar);
-var at_no = foo(bar());
-var new_at_no = new foo(bar());
-var nospace_at_no = foo(bar());
-var nospace_new_at_no = new foo(bar());
-var num_no = foo(bar());
-var new_num_no = new foo(bar());
-var nospace_num_no = foo(bar());
-var nospace_new_num_no = new foo(bar());
-var dot_no = foo(sideEffect()).dot(bar());
-var new_dot_no = new foo(sideEffect()).dot(bar());
-var nested_no = [1, foo(bar()), 2];
-var new_nested_no = [1, new foo(bar()), 2];
-var single_at_no = foo(bar());
-var new_single_at_no = new foo(bar());
-var single_num_no = foo(bar());
-var new_single_num_no = new foo(bar());
-var bad_no = foo(bar);
-var new_bad_no = new foo(bar);
-var parens_no = foo(bar);
-var new_parens_no = new foo(bar);
-var exp_no = foo() ** foo();
-var new_exp_no = new foo() ** foo();
+foo(bar);
+foo(bar());
+new foo(bar());
+foo(bar());
+new foo(bar());
+foo(bar());
+new foo(bar());
+foo(bar());
+new foo(bar());
+foo(sideEffect()).dot(bar());
+new foo(sideEffect()).dot(bar());
+[1, foo(bar()), 2];
+[1, new foo(bar()), 2];
+foo(bar());
+new foo(bar());
+foo(bar());
+new foo(bar());
+foo(bar);
+new foo(bar);
+foo(bar);
+new foo(bar);
+foo() ** foo();
+new foo() ** foo();

```