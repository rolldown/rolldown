import styled from 'styled-components';

// Smoke test that the `manualPureFunctions` option reaches the pipeline and drops a listed
// function's call. The retain/remove behavior lives in the Rust `stmt_eval_analyzer` unit tests
// (`test_manual_pure_chains_*`).
styled.div`
  color: blue;
`;
