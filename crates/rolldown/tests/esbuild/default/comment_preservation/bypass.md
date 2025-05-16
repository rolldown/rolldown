# Reason
1. comments codegen related to `oxc`
2. the original test case is `ModePassThrough`
# Diff
## /out/entry.js
### esbuild
```js
console.log(
  import(
    /* before */
    foo
  ),
  import(
    /* before */
    "foo"
  ),
  import(
    foo
    /* after */
  ),
  import(
    "foo"
    /* after */
  )
);
console.log(
  import(
    "foo",
    /* before */
    { assert: { type: "json" } }
  ),
  import("foo", {
    /* before */
    assert: { type: "json" }
  }),
  import("foo", {
    assert:
      /* before */
      { type: "json" }
  }),
  import("foo", { assert: {
    /* before */
    type: "json"
  } }),
  import("foo", { assert: {
    type:
      /* before */
      "json"
  } }),
  import("foo", { assert: {
    type: "json"
    /* before */
  } }),
  import("foo", {
    assert: { type: "json" }
    /* before */
  }),
  import(
    "foo",
    { assert: { type: "json" } }
    /* before */
  )
);
console.log(
  require(
    /* before */
    foo
  ),
  require(
    /* before */
    "foo"
  ),
  require(
    foo
    /* after */
  ),
  require(
    "foo"
    /* after */
  )
);
console.log(
  require.resolve(
    /* before */
    foo
  ),
  require.resolve(
    /* before */
    "foo"
  ),
  require.resolve(
    foo
    /* after */
  ),
  require.resolve(
    "foo"
    /* after */
  )
);
let [
  /* foo */
] = [
  /* bar */
];
let [
  // foo
] = [
  // bar
];
let [
  /*before*/
  ...s
] = [
  /*before*/
  ...s
];
let [.../*before*/
s2] = [.../*before*/
s2];
let {
  /* foo */
} = {
  /* bar */
};
let {
  // foo
} = {
  // bar
};
let {
  /*before*/
  ...s3
} = {
  /*before*/
  ...s3
};
let { .../*before*/
s4 } = { .../*before*/
s4 };
let [
  /* before */
  x
] = [
  /* before */
  x
];
let [
  /* before */
  x2
  /* after */
] = [
  /* before */
  x2
  /* after */
];
let [
  // before
  x3
  // after
] = [
  // before
  x3
  // after
];
let {
  /* before */
  y
} = {
  /* before */
  y
};
let {
  /* before */
  y2
  /* after */
} = {
  /* before */
  y2
  /* after */
};
let {
  // before
  y3
  // after
} = {
  // before
  y3
  // after
};
let {
  /* before */
  [y4]: y4
} = {
  /* before */
  [y4]: y4
};
let { [
  /* before */
  y5
]: y5 } = { [
  /* before */
  y5
]: y5 };
let { [
  y6
  /* after */
]: y6 } = { [
  y6
  /* after */
]: y6 };
foo[
  /* before */
  x
] = foo[
  /* before */
  x
];
foo[
  x
  /* after */
] = foo[
  x
  /* after */
];
console.log(
  // before
  foo,
  /* comment before */
  bar
  // comment after
);
console.log([
  // before
  foo,
  /* comment before */
  bar
  // comment after
]);
console.log({
  // before
  foo,
  /* comment before */
  bar
  // comment after
});
console.log(class {
  // before
  foo;
  /* comment before */
  bar;
  // comment after
});
console.log(
  () => {
    return (
      /* foo */
      null
    );
  },
  () => {
    throw (
      /* foo */
      null
    );
  },
  () => {
    return (
      /* foo */
      null + 1
    );
  },
  () => {
    throw (
      /* foo */
      null + 1
    );
  },
  () => {
    return (
      // foo
      null + 1
    );
  },
  () => {
    throw (
      // foo
      null + 1
    );
  }
);
console.log(
  /*a*/
  a ? (
    /*b*/
    b
  ) : (
    /*c*/
    c
  ),
  a ? b : c
);
for (
  /*foo*/
  a;
  ;
) ;
for (
  ;
  /*foo*/
  a;
) ;
for (
  ;
  ;
  /*foo*/
  a
) ;
for (
  /*foo*/
  a in b
) ;
for (
  a in
  /*foo*/
  b
) ;
for (
  /*foo*/
  a of b
) ;
for (
  a of
  /*foo*/
  b
) ;
if (
  /*foo*/
  a
) ;
with (
  /*foo*/
  a
) ;
while (
  /*foo*/
  a
) ;
do {
} while (
  /*foo*/
  a
);
switch (
  /*foo*/
  a
) {
}
```
### rolldown
```js

//#region entry.js
console.log(import(foo), import("foo"), import(foo), import("foo"));
console.log(import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }));
console.log(require(foo), require("foo"), require(foo), require("foo"));
console.log(require.resolve(foo), require.resolve("foo"), require.resolve(foo), require.resolve("foo"));
let [ ...s] = [...s];
let [ ...s2] = [...s2];
let {} = {};
let {} = {};
let { ...s3 } = { ...s3 };
let { ...s4 } = { ...s4 };
let [x] = [x];
let { y } = { y };
let { y2 } = { y2 };
let { y3 } = { y3 };
let { [y4]: y4 } = { [y4]: y4 };
let { [y5]: y5 } = { [y5]: y5 };
let { [y6]: y6 } = { [y6]: y6 };
foo[x] = foo[x];
foo[x] = foo[x];
console.log(foo, bar);
console.log([foo, bar]);
console.log({
	foo,
	bar
});
console.log(class {
	foo;
	bar;
});
console.log(() => {
	return null;
}, () => {
	throw null;
}, () => {
	return 1;
}, () => {
	throw 1;
}, () => {
	return 1;
}, () => {
	throw 1;
});
console.log(a ? b : c, a ? b : c);
for (a;;);
for (; a;);
for (;; a);
for (a in b);
for (a in b);
for (a of b);
for (a of b);
if (a);
with(a);
while (a);
do;
while (a);
switch (a) {}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,350 +1,60 @@
-console.log(
-  import(
-    /* before */
-    foo
-  ),
-  import(
-    /* before */
-    "foo"
-  ),
-  import(
-    foo
-    /* after */
-  ),
-  import(
-    "foo"
-    /* after */
-  )
-);
-console.log(
-  import(
-    "foo",
-    /* before */
-    { assert: { type: "json" } }
-  ),
-  import("foo", {
-    /* before */
-    assert: { type: "json" }
-  }),
-  import("foo", {
-    assert:
-      /* before */
-      { type: "json" }
-  }),
-  import("foo", { assert: {
-    /* before */
-    type: "json"
-  } }),
-  import("foo", { assert: {
-    type:
-      /* before */
-      "json"
-  } }),
-  import("foo", { assert: {
-    type: "json"
-    /* before */
-  } }),
-  import("foo", {
-    assert: { type: "json" }
-    /* before */
-  }),
-  import(
-    "foo",
-    { assert: { type: "json" } }
-    /* before */
-  )
-);
-console.log(
-  require(
-    /* before */
-    foo
-  ),
-  require(
-    /* before */
-    "foo"
-  ),
-  require(
-    foo
-    /* after */
-  ),
-  require(
-    "foo"
-    /* after */
-  )
-);
-console.log(
-  require.resolve(
-    /* before */
-    foo
-  ),
-  require.resolve(
-    /* before */
-    "foo"
-  ),
-  require.resolve(
-    foo
-    /* after */
-  ),
-  require.resolve(
-    "foo"
-    /* after */
-  )
-);
-let [
-  /* foo */
-] = [
-  /* bar */
-];
-let [
-  // foo
-] = [
-  // bar
-];
-let [
-  /*before*/
-  ...s
-] = [
-  /*before*/
-  ...s
-];
-let [.../*before*/
-s2] = [.../*before*/
-s2];
-let {
-  /* foo */
-} = {
-  /* bar */
-};
-let {
-  // foo
-} = {
-  // bar
-};
-let {
-  /*before*/
-  ...s3
-} = {
-  /*before*/
-  ...s3
-};
-let { .../*before*/
-s4 } = { .../*before*/
-s4 };
-let [
-  /* before */
-  x
-] = [
-  /* before */
-  x
-];
-let [
-  /* before */
-  x2
-  /* after */
-] = [
-  /* before */
-  x2
-  /* after */
-];
-let [
-  // before
-  x3
-  // after
-] = [
-  // before
-  x3
-  // after
-];
-let {
-  /* before */
-  y
-} = {
-  /* before */
-  y
-};
-let {
-  /* before */
-  y2
-  /* after */
-} = {
-  /* before */
-  y2
-  /* after */
-};
-let {
-  // before
-  y3
-  // after
-} = {
-  // before
-  y3
-  // after
-};
-let {
-  /* before */
-  [y4]: y4
-} = {
-  /* before */
-  [y4]: y4
-};
-let { [
-  /* before */
-  y5
-]: y5 } = { [
-  /* before */
-  y5
-]: y5 };
-let { [
-  y6
-  /* after */
-]: y6 } = { [
-  y6
-  /* after */
-]: y6 };
-foo[
-  /* before */
-  x
-] = foo[
-  /* before */
-  x
-];
-foo[
-  x
-  /* after */
-] = foo[
-  x
-  /* after */
-];
-console.log(
-  // before
-  foo,
-  /* comment before */
-  bar
-  // comment after
-);
-console.log([
-  // before
-  foo,
-  /* comment before */
-  bar
-  // comment after
-]);
+
+//#region entry.js
+console.log(import(foo), import("foo"), import(foo), import("foo"));
+console.log(import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }), import("foo", { assert: { type: "json" } }));
+console.log(require(foo), require("foo"), require(foo), require("foo"));
+console.log(require.resolve(foo), require.resolve("foo"), require.resolve(foo), require.resolve("foo"));
+let [ ...s] = [...s];
+let [ ...s2] = [...s2];
+let {} = {};
+let {} = {};
+let { ...s3 } = { ...s3 };
+let { ...s4 } = { ...s4 };
+let [x] = [x];
+let { y } = { y };
+let { y2 } = { y2 };
+let { y3 } = { y3 };
+let { [y4]: y4 } = { [y4]: y4 };
+let { [y5]: y5 } = { [y5]: y5 };
+let { [y6]: y6 } = { [y6]: y6 };
+foo[x] = foo[x];
+foo[x] = foo[x];
+console.log(foo, bar);
+console.log([foo, bar]);
 console.log({
-  // before
-  foo,
-  /* comment before */
-  bar
-  // comment after
+	foo,
+	bar
 });
 console.log(class {
-  // before
-  foo;
-  /* comment before */
-  bar;
-  // comment after
+	foo;
+	bar;
 });
-console.log(
-  () => {
-    return (
-      /* foo */
-      null
-    );
-  },
-  () => {
-    throw (
-      /* foo */
-      null
-    );
-  },
-  () => {
-    return (
-      /* foo */
-      null + 1
-    );
-  },
-  () => {
-    throw (
-      /* foo */
-      null + 1
-    );
-  },
-  () => {
-    return (
-      // foo
-      null + 1
-    );
-  },
-  () => {
-    throw (
-      // foo
-      null + 1
-    );
-  }
-);
-console.log(
-  /*a*/
-  a ? (
-    /*b*/
-    b
-  ) : (
-    /*c*/
-    c
-  ),
-  a ? b : c
-);
-for (
-  /*foo*/
-  a;
-  ;
-) ;
-for (
-  ;
-  /*foo*/
-  a;
-) ;
-for (
-  ;
-  ;
-  /*foo*/
-  a
-) ;
-for (
-  /*foo*/
-  a in b
-) ;
-for (
-  a in
-  /*foo*/
-  b
-) ;
-for (
-  /*foo*/
-  a of b
-) ;
-for (
-  a of
-  /*foo*/
-  b
-) ;
-if (
-  /*foo*/
-  a
-) ;
-with (
-  /*foo*/
-  a
-) ;
-while (
-  /*foo*/
-  a
-) ;
-do {
-} while (
-  /*foo*/
-  a
-);
-switch (
-  /*foo*/
-  a
-) {
-}
\ No newline at end of file
+console.log(() => {
+	return null;
+}, () => {
+	throw null;
+}, () => {
+	return 1;
+}, () => {
+	throw 1;
+}, () => {
+	return 1;
+}, () => {
+	throw 1;
+});
+console.log(a ? b : c, a ? b : c);
+for (a;;);
+for (; a;);
+for (;; a);
+for (a in b);
+for (a in b);
+for (a of b);
+for (a of b);
+if (a);
+with(a);
+while (a);
+do;
+while (a);
+switch (a) {}
+
+//#endregion
\ No newline at end of file

```