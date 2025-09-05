function unusedFunction() {
  console.log('this should be removed by minifier');
  return 'unused';
}

function keepThis() {
  const message = 'Hello, World!';
  return message;
}

export { keepThis };