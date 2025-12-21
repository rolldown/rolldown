---
# https://vitepress.dev/reference/default-theme-home-page
layout: home
theme: dark
---

<Hero/>
<TrustedBy :logos="['linear', 'framer', 'mercedes', 'beehiiv', 'excalidraw']" />
<HeadingSection
heading="Performance without sacrificing familiarity"
/>
<RolldownFeatureGrid />
<Sponsors
  description="Rolldown is free and open source, made possible by a full-time team and passionate open-source contributors."
  sponsorLinkText="Contribute"
  sponsorLink="/contribution-guide"
/>
<Spacer />
<Footer
  heading="Optimize bundling with Rolldown"
  subheading="Bundle at the speed of native code with more flexible chunk split control, module-level persistent cache, and more."
  button-text="Get started"
  button-link="/guide/getting-started"
/>
