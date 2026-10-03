import SiteFooter from '@voidzero-dev/doc-kit-theme/components/SiteFooter.jsx';

import navigation from '../../navigation.json' with { type: 'json' };
import NavBar from '../components/NavBar.jsx';

/**
 * A marketing page (`layout: home`): the content spans the whole width,
 * between the header and the site footer.
 */
export default ({ metadata, children }) => (
  <div class="flex min-h-screen flex-col">
    <NavBar metadata={metadata} />
    {/* Not a <main>: doc-kit styles headings in <main> as API entries */}
    <div class="home" role="main">
      {children}
    </div>
    <SiteFooter footer={navigation.footer} socialLinks={navigation.socialLinks} />
  </div>
);
