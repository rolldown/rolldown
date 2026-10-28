const status = document.getElementById('circular-import-binding-status');

status.textContent = 'ready';

document.getElementById('circular-import-binding-btn').addEventListener('click', async () => {
  status.textContent = 'loading';
  try {
    const { defaults } = await import('./a.js');
    status.textContent = `button=${defaults.button.name}`;
  } catch (error) {
    status.textContent = `error: ${error.message}`;
  }
});
