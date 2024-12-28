export function es2015() {
  let temp;
  try {
    temp = JSON.parse("[1, 2, [3]]");
  } catch {  }
  console.log(temp)
  console.log([1, 2, [3]].flat())
}