// This file is intentionally marked as no side effect to check if rolldown will remove unused no-side-effect modules.

function foo() {
  globalThis.value = 'called'
}

/*#__PURE__*/ foo()