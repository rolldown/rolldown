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
    name: 'Yunfei He',
    links: [
      { icon: 'github', link: 'https://github.com/hyf0' },
      { icon: 'twitter', link: 'https://twitter.com/_hyf0' }
    ]
  },
  {
    avatar: 'https://www.github.com/underfin.png',
    name: 'Kui Li (underfin)',
    links: [
      { icon: 'github', link: 'https://github.com/underfin' }
    ]
  }
]
</script>

# Team

The Rolldown project was originally created by [Yinan Long](https://github.com/Brooooooklyn) (aka Brooooooklyn, author of [NAPI-RS](https://napi.rs/)), and is now led by [Evan You](https://github.com/yyx990803) (creator of [Vite](https://vitejs.dev/)).

<VPTeamMembers size="small" :members="members" />

## Join Us!

Rolldown is still in early stage. We have a lot of ground to cover, and we won't be able to do this without the help from community contributors. We are also actively looking for more team members with long term commitment in improving JavaScript tooling with Rust.

### Useful Links

- [GitHub](https://github.com/rolldown-rs/rolldown)
- [Contribution Guide](/contrib-guide/)
- [Discord Chat](https://discord.gg/vsZxvsfgC5)
