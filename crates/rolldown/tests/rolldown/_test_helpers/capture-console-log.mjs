export async function captureConsoleLog(run) {
  const logs = [];
  const originalLog = console.log;
  console.log = (...args) => {
    logs.push(args.join(' '));
  };
  try {
    await run();
  } finally {
    console.log = originalLog;
  }
  return logs;
}
