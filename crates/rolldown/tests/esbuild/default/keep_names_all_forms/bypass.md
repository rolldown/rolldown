# Reason
1. part of minifier
# Diff
## /out/keep.js
### esbuild
```js
function fn() {
}
__name(fn, "fn");
function foo(fn2 = function() {
}) {
}
__name(foo, "foo");
var fn = /* @__PURE__ */ __name(function() {
}, "fn");
var obj = { "f n": /* @__PURE__ */ __name(function() {
}, "f n") };
class Foo0 {
  static {
    __name(this, "Foo0");
  }
  "f n" = /* @__PURE__ */ __name(function() {
  }, "f n");
}
class Foo1 {
  static {
    __name(this, "Foo1");
  }
  static "f n" = /* @__PURE__ */ __name(function() {
  }, "f n");
}
class Foo2 {
  static {
    __name(this, "Foo2");
  }
  accessor "f n" = /* @__PURE__ */ __name(function() {
  }, "f n");
}
class Foo3 {
  static {
    __name(this, "Foo3");
  }
  static accessor "f n" = /* @__PURE__ */ __name(function() {
  }, "f n");
}
class Foo4 {
  static {
    __name(this, "Foo4");
  }
  #fn = /* @__PURE__ */ __name(function() {
  }, "#fn");
}
class Foo5 {
  static {
    __name(this, "Foo5");
  }
  static #fn = /* @__PURE__ */ __name(function() {
  }, "#fn");
}
class Foo6 {
  static {
    __name(this, "Foo6");
  }
  accessor #fn = /* @__PURE__ */ __name(function() {
  }, "#fn");
}
class Foo7 {
  static {
    __name(this, "Foo7");
  }
  static accessor #fn = /* @__PURE__ */ __name(function() {
  }, "#fn");
}
fn = /* @__PURE__ */ __name(function() {
}, "fn");
fn ||= /* @__PURE__ */ __name(function() {
}, "fn");
fn &&= /* @__PURE__ */ __name(function() {
}, "fn");
fn ??= /* @__PURE__ */ __name(function() {
}, "fn");
var [fn = /* @__PURE__ */ __name(function() {
}, "fn")] = [];
var { fn = /* @__PURE__ */ __name(function() {
}, "fn") } = {};
for (var [fn = /* @__PURE__ */ __name(function() {
}, "fn")] = []; ; ) ;
for (var { fn = /* @__PURE__ */ __name(function() {
}, "fn") } = {}; ; ) ;
for (var [fn = /* @__PURE__ */ __name(function() {
}, "fn")] in obj) ;
for (var { fn = /* @__PURE__ */ __name(function() {
}, "fn") } in obj) ;
for (var [fn = /* @__PURE__ */ __name(function() {
}, "fn")] of obj) ;
for (var { fn = /* @__PURE__ */ __name(function() {
}, "fn") } of obj) ;
function foo([fn2 = /* @__PURE__ */ __name(function() {
}, "fn")]) {
}
__name(foo, "foo");
function foo({ fn: fn2 = /* @__PURE__ */ __name(function() {
}, "fn") }) {
}
__name(foo, "foo");
[fn = /* @__PURE__ */ __name(function() {
}, "fn")] = [];
({ fn = /* @__PURE__ */ __name(function() {
}, "fn") } = {});
```
### rolldown
```js
//#region keep.js
// Initializers
function fn() {}
var fn = function() {};
var obj = { "f n": function() {} };
// Assignments
fn = function() {};
fn ||= function() {};
fn &&= function() {};
fn ??= function() {};
// Destructuring
var [fn = function() {}] = [];
var { fn = function() {} } = {};
for (var [fn = function() {}] = [];;);
for (var { fn = function() {} } = {};;);
for (var [fn = function() {}] in obj);
for (var { fn = function() {} } in obj);
for (var [fn = function() {}] of obj);
for (var { fn = function() {} } of obj);
[fn = function() {}] = [];
({fn = function() {}} = {});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/keep.js
+++ rolldown	keep.js
@@ -1,103 +1,23 @@
-function fn() {
-}
-__name(fn, "fn");
-function foo(fn2 = function() {
-}) {
-}
-__name(foo, "foo");
-var fn = /* @__PURE__ */ __name(function() {
-}, "fn");
-var obj = { "f n": /* @__PURE__ */ __name(function() {
-}, "f n") };
-class Foo0 {
-  static {
-    __name(this, "Foo0");
-  }
-  "f n" = /* @__PURE__ */ __name(function() {
-  }, "f n");
-}
-class Foo1 {
-  static {
-    __name(this, "Foo1");
-  }
-  static "f n" = /* @__PURE__ */ __name(function() {
-  }, "f n");
-}
-class Foo2 {
-  static {
-    __name(this, "Foo2");
-  }
-  accessor "f n" = /* @__PURE__ */ __name(function() {
-  }, "f n");
-}
-class Foo3 {
-  static {
-    __name(this, "Foo3");
-  }
-  static accessor "f n" = /* @__PURE__ */ __name(function() {
-  }, "f n");
-}
-class Foo4 {
-  static {
-    __name(this, "Foo4");
-  }
-  #fn = /* @__PURE__ */ __name(function() {
-  }, "#fn");
-}
-class Foo5 {
-  static {
-    __name(this, "Foo5");
-  }
-  static #fn = /* @__PURE__ */ __name(function() {
-  }, "#fn");
-}
-class Foo6 {
-  static {
-    __name(this, "Foo6");
-  }
-  accessor #fn = /* @__PURE__ */ __name(function() {
-  }, "#fn");
-}
-class Foo7 {
-  static {
-    __name(this, "Foo7");
-  }
-  static accessor #fn = /* @__PURE__ */ __name(function() {
-  }, "#fn");
-}
-fn = /* @__PURE__ */ __name(function() {
-}, "fn");
-fn ||= /* @__PURE__ */ __name(function() {
-}, "fn");
-fn &&= /* @__PURE__ */ __name(function() {
-}, "fn");
-fn ??= /* @__PURE__ */ __name(function() {
-}, "fn");
-var [fn = /* @__PURE__ */ __name(function() {
-}, "fn")] = [];
-var { fn = /* @__PURE__ */ __name(function() {
-}, "fn") } = {};
-for (var [fn = /* @__PURE__ */ __name(function() {
-}, "fn")] = []; ; ) ;
-for (var { fn = /* @__PURE__ */ __name(function() {
-}, "fn") } = {}; ; ) ;
-for (var [fn = /* @__PURE__ */ __name(function() {
-}, "fn")] in obj) ;
-for (var { fn = /* @__PURE__ */ __name(function() {
-}, "fn") } in obj) ;
-for (var [fn = /* @__PURE__ */ __name(function() {
-}, "fn")] of obj) ;
-for (var { fn = /* @__PURE__ */ __name(function() {
-}, "fn") } of obj) ;
-function foo([fn2 = /* @__PURE__ */ __name(function() {
-}, "fn")]) {
-}
-__name(foo, "foo");
-function foo({ fn: fn2 = /* @__PURE__ */ __name(function() {
-}, "fn") }) {
-}
-__name(foo, "foo");
-[fn = /* @__PURE__ */ __name(function() {
-}, "fn")] = [];
-({ fn = /* @__PURE__ */ __name(function() {
-}, "fn") } = {});
\ No newline at end of file
+//#region keep.js
+// Initializers
+function fn() {}
+var fn = function() {};
+var obj = { "f n": function() {} };
+// Assignments
+fn = function() {};
+fn ||= function() {};
+fn &&= function() {};
+fn ??= function() {};
+// Destructuring
+var [fn = function() {}] = [];
+var { fn = function() {} } = {};
+for (var [fn = function() {}] = [];;);
+for (var { fn = function() {} } = {};;);
+for (var [fn = function() {}] in obj);
+for (var { fn = function() {} } in obj);
+for (var [fn = function() {}] of obj);
+for (var { fn = function() {} } of obj);
+[fn = function() {}] = [];
+({fn = function() {}} = {});
+
+//#endregion
\ No newline at end of file

```
## /out/do-not-keep.js
### esbuild
```js
class Foo0 {
  static {
    __name(this, "Foo0");
  }
  fn() {
  }
}
class Foo1 {
  static {
    __name(this, "Foo1");
  }
  *fn() {
  }
}
class Foo2 {
  static {
    __name(this, "Foo2");
  }
  get fn() {
  }
}
class Foo3 {
  static {
    __name(this, "Foo3");
  }
  set fn(_) {
  }
}
class Foo4 {
  static {
    __name(this, "Foo4");
  }
  async fn() {
  }
}
class Foo5 {
  static {
    __name(this, "Foo5");
  }
  static fn() {
  }
}
class Foo6 {
  static {
    __name(this, "Foo6");
  }
  static *fn() {
  }
}
class Foo7 {
  static {
    __name(this, "Foo7");
  }
  static get fn() {
  }
}
class Foo8 {
  static {
    __name(this, "Foo8");
  }
  static set fn(_) {
  }
}
class Foo9 {
  static {
    __name(this, "Foo9");
  }
  static async fn() {
  }
}
class Bar0 {
  static {
    __name(this, "Bar0");
  }
  #fn() {
  }
}
class Bar1 {
  static {
    __name(this, "Bar1");
  }
  *#fn() {
  }
}
class Bar2 {
  static {
    __name(this, "Bar2");
  }
  get #fn() {
  }
}
class Bar3 {
  static {
    __name(this, "Bar3");
  }
  set #fn(_) {
  }
}
class Bar4 {
  static {
    __name(this, "Bar4");
  }
  async #fn() {
  }
}
class Bar5 {
  static {
    __name(this, "Bar5");
  }
  static #fn() {
  }
}
class Bar6 {
  static {
    __name(this, "Bar6");
  }
  static *#fn() {
  }
}
class Bar7 {
  static {
    __name(this, "Bar7");
  }
  static get #fn() {
  }
}
class Bar8 {
  static {
    __name(this, "Bar8");
  }
  static set #fn(_) {
  }
}
class Bar9 {
  static {
    __name(this, "Bar9");
  }
  static async #fn(_) {
  }
}
const Baz0 = { fn() {
} };
const Baz1 = { *fn() {
} };
const Baz2 = { get fn() {
} };
const Baz3 = { set fn(_) {
} };
const Baz4 = { async fn() {
} };
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/do-not-keep.js
+++ rolldown	do-not-keep.js
@@ -1,135 +0,0 @@
-class Foo0 {
-    static {
-        __name(this, "Foo0");
-    }
-    fn() {}
-}
-class Foo1 {
-    static {
-        __name(this, "Foo1");
-    }
-    *fn() {}
-}
-class Foo2 {
-    static {
-        __name(this, "Foo2");
-    }
-    get fn() {}
-}
-class Foo3 {
-    static {
-        __name(this, "Foo3");
-    }
-    set fn(_) {}
-}
-class Foo4 {
-    static {
-        __name(this, "Foo4");
-    }
-    async fn() {}
-}
-class Foo5 {
-    static {
-        __name(this, "Foo5");
-    }
-    static fn() {}
-}
-class Foo6 {
-    static {
-        __name(this, "Foo6");
-    }
-    static *fn() {}
-}
-class Foo7 {
-    static {
-        __name(this, "Foo7");
-    }
-    static get fn() {}
-}
-class Foo8 {
-    static {
-        __name(this, "Foo8");
-    }
-    static set fn(_) {}
-}
-class Foo9 {
-    static {
-        __name(this, "Foo9");
-    }
-    static async fn() {}
-}
-class Bar0 {
-    static {
-        __name(this, "Bar0");
-    }
-    #fn() {}
-}
-class Bar1 {
-    static {
-        __name(this, "Bar1");
-    }
-    *#fn() {}
-}
-class Bar2 {
-    static {
-        __name(this, "Bar2");
-    }
-    get #fn() {}
-}
-class Bar3 {
-    static {
-        __name(this, "Bar3");
-    }
-    set #fn(_) {}
-}
-class Bar4 {
-    static {
-        __name(this, "Bar4");
-    }
-    async #fn() {}
-}
-class Bar5 {
-    static {
-        __name(this, "Bar5");
-    }
-    static #fn() {}
-}
-class Bar6 {
-    static {
-        __name(this, "Bar6");
-    }
-    static *#fn() {}
-}
-class Bar7 {
-    static {
-        __name(this, "Bar7");
-    }
-    static get #fn() {}
-}
-class Bar8 {
-    static {
-        __name(this, "Bar8");
-    }
-    static set #fn(_) {}
-}
-class Bar9 {
-    static {
-        __name(this, "Bar9");
-    }
-    static async #fn(_) {}
-}
-const Baz0 = {
-    fn() {}
-};
-const Baz1 = {
-    *fn() {}
-};
-const Baz2 = {
-    get fn() {}
-};
-const Baz3 = {
-    set fn(_) {}
-};
-const Baz4 = {
-    async fn() {}
-};

```