# Diff
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
+++ rolldown	
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