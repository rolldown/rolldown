exports.defaultProvider = async function test() {
  const { fromIni } = await import('./c.js');
  console.log(fromIni);
};
