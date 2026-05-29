async function run() {
  const m = await import("./lazy.js");
  console.log(m.value);
}
run();
