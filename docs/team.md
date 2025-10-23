---
outline: false
---

<script setup>
import { VPTeamMembers } from 'vitepress/theme'

const members = [
  {
    avatar: 'https://www.github.com/yyx990803.png',
    name: 'Evan You',
    links: [
      { icon: 'github', link: 'https://github.com/yyx990803' },
      { icon: 'twitter', link: 'https://twitter.com/youyuxi' }
    ]
  },
  {
    avatar: 'https://www.github.com/Brooooooklyn.png',
    name: 'Yinan Long (Brooooooklyn)',
    links: [
      { icon: 'github', link: 'https://github.com/Brooooooklyn' },
      { icon: 'twitter', link: 'https://twitter.com/Brooooook_lyn' }
    ]
  },
  {
    avatar: 'https://www.github.com/hyf0.png',
    name: 'Yunfei He (hyf0)',
    links: [
      { icon: 'github', link: 'https://github.com/hyf0' },
      { icon: 'twitter', link: 'https://twitter.com/_hyf0' }
    ]
  },
  {
    avatar: 'https://www.github.com/iwanabethatguy.png',
    name: 'Xiangjun He (iwanabethatguy)',
    links: [
      { icon: 'github', link: 'https://github.com/iwanabethatguy' }
    ]
  },
  {
    avatar: 'https://www.github.com/boshen.png',
    name: 'Boshen',
    links: [
      { icon: 'github', link: 'https://github.com/boshen' },
      { icon: 'twitter', link: 'https://twitter.com/boshen_c' },
      { icon: 'bluesky', link: 'https://bsky.app/profile/boshen.github.io' }
    ]
  },
  {
    name: 'shulaoda',
    avatar: 'https://www.github.com/shulaoda.png',
    links: [
      { icon: 'github', link: 'https://github.com/shulaoda' },
      { icon: 'twitter', link: 'https://x.com/dalaoshv' }
    ]
  },
  {
    name: 'Kevin Deng (sxzz)',
    avatar: 'https://www.github.com/sxzz.png',
    links: [
      { icon: 'github', link: 'https://github.com/sxzz' },
      { icon: 'twitter', link: 'https://x.com/sanxiaozhizi' },
      { icon: 'bluesky', link: 'https://bsky.app/profile/sxzz.dev' }
    ]
  },
  {
    name: '翠 (sapphi-red)',
    avatar: 'https://www.github.com/sapphi-red.png',
    links: [
      { icon: 'github', link: 'https://github.com/sapphi-red' },
      { icon: 'twitter', link: 'https://x.com/sapphi_red' },
      { icon: 'bluesky', link: 'https://bsky.app/profile/sapphi.red' }
    ]
  },
  {
    name: 'Alexander Lichter',
    avatar: 'https://www.github.com/TheAlexLichter.png',
    links: [
      { icon: 'github', link: 'https://github.com/TheAlexLichter' },
      { icon: 'twitter', link: 'https://x.com/TheAlexLichter' },
      { icon: 'bluesky', link: 'https://bsky.app/profile/thealexlichter.com' }
    ]
  }
]
</script>

# Team

The team members work full time on the Rolldown project and are responsible for its development, maintenance, and community engagement.

<VPTeamMembers size="small" :members="members" />

## Past Contributors

You can find the past team members and other people who significantly contributed to Rolldown over the years on the [acknowledgements](./acknowledgements.md) page.

## Join Us!

Rolldown is still in early stage. We have a lot of ground to cover, and we won't be able to do this without the help from community contributors. We are also actively looking for more team members with long term commitment in improving JavaScript tooling with Rust.

### Useful Links

- [GitHub](https://github.com/rolldown/rolldown)
- [Contribution Guide](/contribution-guide/)
- [Discord Chat](https://chat.rolldown.rs)
