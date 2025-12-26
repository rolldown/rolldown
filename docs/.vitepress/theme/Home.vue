<script setup>
import Hero from './Hero.vue'
// import TrustedBy from '@components/oss/TrustedBy.vue'
import HeadingSection from '@components/oss/HeadingSection.vue'
import RolldownFeatureGrid from './FeatureGrid.vue'
import Sponsors from '@components/oss/Sponsors.vue'
import Spacer from '@components/shared/Spacer.vue'
import Footer from '@components/oss/Footer.vue'
</script>

<template>
  <Hero />
  <!-- 'framer' requires public/trusted-by/framer.svg -->
  <!-- <TrustedBy :logos="['framer', 'angular']" /> -->
  <HeadingSection heading="Performance + Features + Ecosystem" />
  <RolldownFeatureGrid />
  <Sponsors
    description="Rolldown is free and open source, made possible by a full-time team and passionate open-source contributors."
    sponsorLinkText="Contribute" sponsorLink="/contribution-guide" />
  <Spacer />
  <Footer heading="Optimize your JavaScript code with Rolldown"
    subheading="Why do we still need bundlers?"
    button-text="Learn more" button-link="/in-depth/why-bundlers" />
</template>
