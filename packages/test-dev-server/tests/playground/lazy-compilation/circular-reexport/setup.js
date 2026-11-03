const status = document.getElementById('circular-reexport-status');

status.textContent = 'ready';

document.getElementById('circular-reexport-btn').addEventListener('click', async () => {
  status.textContent = 'loading';
  try {
    const { getValueAB } = await import('./circular-dep-init.js');
    status.textContent = getValueAB();
  } catch (error) {
    status.textContent = `error: ${error.message}`;
  }
});
