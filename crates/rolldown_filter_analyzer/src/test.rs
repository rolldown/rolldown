use crate::filterable;

#[test]
fn not_filterable_case() {
  assert!(!filterable(
    r"
function test() {
      throw new Error(
      )
}
  ",
  ),);

  assert!(!filterable(
    r"
function test() {
  if (a) {
    call();
  }
  call();
}
  ",
  ),);

  assert!(!filterable(
    r"
function test() {
  if (a) {
    call();
  }
  throw new Error();
}
  ",
  ),);

  assert!(!filterable(
    r#"
function test() {
  if (a) {
    call();
  }
  return {
    code: "test"
  };
}
  "#,
  ),);
}

#[test]
fn filterable_case() {
  // Different return case
  assert!(filterable(
    r"
function test() {
  if (a) {
    return;
  }
  call();
}

  ",
  ),);

  assert!(filterable(
    r"
function test() {
  if (a) {
    return undefined;
  }
  call();
}
  ",
  ));

  // implicit return at the end of function
  assert!(filterable(
    r"
function test() {
  if (a) {
    call();
  }
}

  ",
  ),);

  assert!(filterable(
    r"
function test() {
  if (a) {
    return;
  }

  if (b) {
    call();
  }
}
  ",
  ),);
}
