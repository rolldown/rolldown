import NavBar from '@voidzero-dev/doc-kit-theme/components/NavBar.jsx';

import navigation from '../../navigation.json' with { type: 'json' };

const LOGO = {
  light: '/assets/brand/rolldown-dark.svg',
  dark: '/assets/brand/rolldown-light.svg',
  alt: 'Rolldown',
};

/**
 * The theme's navigation bar, with Rolldown's navigation and logo.
 *
 * @param {{ metadata: import('@doc-kit/generator-react/html/ui/types').SerializedMetadata }} props
 */
export default ({ metadata }) => (
  <NavBar
    metadata={metadata}
    nav={navigation.nav}
    socialLinks={navigation.socialLinks}
    logo={LOGO}
  />
);
