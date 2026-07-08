import { createRequire } from 'node:module';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import nodePath from 'node:path';

const require = createRequire(import.meta.url);
const bindingPath = process.env.ROLLDOWN_TEST_BINDING_PATH ?? '../../../src/binding.cjs';
const binding = require(bindingPath);
const {
  __rolldownTestRetainSchedulerWaker,
  getRuntimeCapabilities,
  registerCurrentThreadTaskHost,
  shutdownAsyncRuntime,
} = binding;
const capabilities = getRuntimeCapabilities();
const NativePromise = globalThis.Promise;
const unhandled = [];
let callbackCalls = 0;
let constructorGetterCalls = 0;

process.on('unhandledRejection', (reason) => {
  unhandled.push(reason instanceof Error ? reason.message : String(reason));
});

const hostileCallback = () => {
  callbackCalls += 1;
  const rejected = NativePromise.reject(new Error('task-host callback must never run'));
  Object.defineProperty(rejected, 'constructor', {
    configurable: true,
    get() {
      constructorGetterCalls += 1;
      throw new Error('poisoned native Promise constructor getter');
    },
  });
  return rejected;
};

let registrationError;
try {
  registerCurrentThreadTaskHost(hostileCallback);
} catch (error) {
  registrationError = error instanceof Error ? error.message : String(error);
}

if (registrationError !== 'registerCurrentThreadTaskHost does not accept a JavaScript callback') {
  throw new Error(`Unexpected task-host registration result: ${registrationError}`);
}
if (callbackCalls !== 0 || constructorGetterCalls !== 0) {
  throw new Error('Rejected task-host callbacks must not be invoked or inspected');
}
if (
  typeof binding.driveCurrentThreadRuntimeTasks !== 'undefined' ||
  typeof binding.cancelCurrentThreadRuntimeTaskDispatch !== 'undefined'
) {
  throw new Error('CurrentThread task delivery capabilities must remain native-owned');
}

registerCurrentThreadTaskHost();

if (typeof __rolldownTestRetainSchedulerWaker !== 'function') {
  throw new Error('The async-runtime binding was built without the scheduler-waker test probe');
}

const directory = mkdtempSync(nodePath.join(tmpdir(), 'rolldown-task-host-contract-'));
const armed = nodePath.join(directory, 'armed');
const release = nodePath.join(directory, 'release');
const completed = nodePath.join(directory, 'completed');

const waitFor = async (path) => {
  const deadline = Date.now() + 10_000;
  while (!existsSync(path)) {
    if (Date.now() >= deadline) {
      throw new Error(`Timed out waiting for ${path}`);
    }
    await new NativePromise((resolve) => setTimeout(resolve, 5));
  }
};

try {
  __rolldownTestRetainSchedulerWaker(armed, release, completed);
  await waitFor(armed);
  writeFileSync(release, 'release');
  await waitFor(completed);
  await new NativePromise((resolve) => setImmediate(resolve));
  await new NativePromise((resolve) => setImmediate(resolve));

  const result = {
    backend: capabilities.backend,
    callbackCalls,
    completed: existsSync(completed),
    constructorGetterCalls,
    flavor: capabilities.flavor,
    registrationError,
    unhandled,
  };
  console.log(JSON.stringify(result));
  if (
    !result.completed ||
    callbackCalls !== 0 ||
    constructorGetterCalls !== 0 ||
    unhandled.length !== 0
  ) {
    process.exitCode = 1;
  }
} finally {
  try {
    shutdownAsyncRuntime();
  } finally {
    rmSync(directory, { recursive: true, force: true, maxRetries: 5, retryDelay: 10 });
  }
}
