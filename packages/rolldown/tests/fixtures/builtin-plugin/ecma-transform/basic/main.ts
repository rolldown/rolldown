interface Foo {}
const a = 1;
const b = 2;

// Regression for https://github.com/rolldown/rolldown/issues/9312:
// a string-enum alias (`Default = Theme.Light`) must lower to the
// forward-only assignment `Theme["Default"] = "Light"`, not the
// reverse-mapping form that would overwrite `Theme["Light"]`.
enum Theme {
  Light = 'Light',
  Dark = 'Dark',
  Default = Theme.Light,
}

export { a, b, Theme };
