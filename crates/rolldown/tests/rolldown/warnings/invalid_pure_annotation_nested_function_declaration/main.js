function outer() {
  // Pure annotation before a nested function declaration — should suggest NO_SIDE_EFFECTS.
  /* #__PURE__ */ function inner() {}
  inner();
}

outer();
