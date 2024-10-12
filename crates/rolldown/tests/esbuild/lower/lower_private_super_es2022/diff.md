# Diff
## /out.js
### esbuild
```js
// foo1.js
var foo1_default = class extends x {
  #foo() {
    super.foo();
  }
};

// foo2.js
var foo2_default = class extends x {
  #foo() {
    super.foo++;
  }
};

// foo3.js
var foo3_default = class extends x {
  static #foo() {
    super.foo();
  }
};

// foo4.js
var foo4_default = class extends x {
  static #foo() {
    super.foo++;
  }
};

// foo5.js
var foo5_default = class extends x {
  #foo = () => {
    super.foo();
  };
};

// foo6.js
var foo6_default = class extends x {
  #foo = () => {
    super.foo++;
  };
};

// foo7.js
var foo7_default = class extends x {
  static #foo = () => {
    super.foo();
  };
};

// foo8.js
var foo8_default = class extends x {
  static #foo = () => {
    super.foo++;
  };
};
export {
  foo1_default as foo1,
  foo2_default as foo2,
  foo3_default as foo3,
  foo4_default as foo4,
  foo5_default as foo5,
  foo6_default as foo6,
  foo7_default as foo7,
  foo8_default as foo8
};
```
### rolldown
```js

//#region foo1.js
class foo1_default extends x {
	#foo() {
		super.foo();
	}
}

//#endregion
//#region foo2.js
class foo2_default extends x {
	#foo() {
		super.foo++;
	}
}

//#endregion
//#region foo3.js
class foo3_default extends x {
	static #foo() {
		super.foo();
	}
}

//#endregion
//#region foo4.js
class foo4_default extends x {
	static #foo() {
		super.foo++;
	}
}

//#endregion
//#region foo5.js
class foo5_default extends x {
	#foo = () => {
		super.foo();
	};
}

//#endregion
//#region foo6.js
class foo6_default extends x {
	#foo = () => {
		super.foo++;
	};
}

//#endregion
//#region foo7.js
class foo7_default extends x {
	static #foo = () => {
		super.foo();
	};
}

//#endregion
//#region foo8.js
class foo8_default extends x {
	static #foo = () => {
		super.foo++;
	};
}

//#endregion
export { foo1_default as foo1, foo2_default as foo2, foo3_default as foo3, foo4_default as foo4, foo5_default as foo5, foo6_default as foo6, foo7_default as foo7, foo8_default as foo8 };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,41 +1,41 @@
-var foo1_default = class extends x {
+class foo1_default extends x {
     #foo() {
         super.foo();
     }
-};
-var foo2_default = class extends x {
+}
+class foo2_default extends x {
     #foo() {
         super.foo++;
     }
-};
-var foo3_default = class extends x {
+}
+class foo3_default extends x {
     static #foo() {
         super.foo();
     }
-};
-var foo4_default = class extends x {
+}
+class foo4_default extends x {
     static #foo() {
         super.foo++;
     }
-};
-var foo5_default = class extends x {
+}
+class foo5_default extends x {
     #foo = () => {
         super.foo();
     };
-};
-var foo6_default = class extends x {
+}
+class foo6_default extends x {
     #foo = () => {
         super.foo++;
     };
-};
-var foo7_default = class extends x {
+}
+class foo7_default extends x {
     static #foo = () => {
         super.foo();
     };
-};
-var foo8_default = class extends x {
+}
+class foo8_default extends x {
     static #foo = () => {
         super.foo++;
     };
-};
+}
 export {foo1_default as foo1, foo2_default as foo2, foo3_default as foo3, foo4_default as foo4, foo5_default as foo5, foo6_default as foo6, foo7_default as foo7, foo8_default as foo8};

```