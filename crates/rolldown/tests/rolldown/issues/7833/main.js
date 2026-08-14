const load = async () => {
  const result = await import('./imp.js');
  console.log(result.imp);
};
load();
