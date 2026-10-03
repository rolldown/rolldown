async function test() {
  const a = await import('./hello');
  const b = await import('./hello');

  console.log(a === b);
}

test();
