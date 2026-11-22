try {
  fromDefine();
} catch (error) {
  globalThis.calls.push(`define annotation: ${error.name}`);
}

const fromDefine = ANNOTATED;
