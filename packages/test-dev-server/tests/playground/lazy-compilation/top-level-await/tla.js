const value = await Promise.resolve('ok');
document.getElementById('top-level-await-status').textContent = `TLA ${value}`;
