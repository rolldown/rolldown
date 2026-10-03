import ReferenceIndex from '@voidzero-dev/doc-kit-theme/components/ReferenceIndex.jsx';

import { referenceSections } from '../reference.mjs';

/**
 * The reference's landing page: every option and API, filterable by name.
 */
export default () => (
  <ReferenceIndex
    title="Options & APIs Reference"
    description="These are the automatically generated references for Rolldown's options and APIs. Use the sidebar navigation to browse specific options and APIs."
    sections={referenceSections}
  />
);
