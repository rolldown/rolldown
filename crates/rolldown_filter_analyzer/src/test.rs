use crate::filterable;

#[test]
fn unfilterable_case() {
  assert_eq!(
    filterable(
      r#"
function test() {
      throw new Error(
      )
}
  "#,
    ),
    false
  );

  assert_eq!(
    filterable(
      r#"
function test() {
  if (a) {
    call();
  }
  call();
}
  "#,
    ),
    false
  );

  assert_eq!(
    filterable(
      r#"
function test() {
  if (a) {
    call();
  }
  throw new Error();
}
  "#,
    ),
    false
  );

  assert_eq!(
    filterable(
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
    ),
    false
  );
}

#[test]
fn filterable_case() {
  // Different return case
  assert_eq!(
    filterable(
      r#"
function test() {
  if (a) {
    return;
  }
  call();
}

  "#,
    ),
    true
  );

  assert_eq!(
    filterable(
      r#"
function test() {
  if (a) {
    return undefined;
  }
  call();
}
  "#,
    ),
    true
  );

  // implicit return at the end of function
  assert_eq!(
    filterable(
      r#"
function test() {
  if (a) {
    call();
  }
}

  "#,
    ),
    true
  );

  assert_eq!(
    filterable(
      r#"
function test() {
  if (a) {
    return;
  }

  if (b) {
    call();
  }
}
  "#,
    ),
    true
  );
}
