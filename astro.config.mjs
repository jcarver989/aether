// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  integrations: [
    starlight({
      title: "Aether",
      customCss: [
        "./src/styles/global.css",
        "./src/styles/starlight.css",
        "./src/styles/themes.css",
      ],
      components: {
        Head: "./src/components/StarlightHead.astro",
        ThemeSelect: "./src/components/ThemeSwitcher.astro",
      },
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/joshka/aether",
        },
      ],
      /* EC config in ec.config.mjs (themeCssSelector isn't JSON-serializable) */
      sidebar: [
        {
          label: "Getting Started",
          items: [
            { label: "Introduction", slug: "guides/introduction" },
            { label: "Quick Start", slug: "guides/quickstart" },
          ],
        },
        {
          label: "Packages",
          autogenerate: { directory: "reference" },
        },
      ],
    }),
  ],
  vite: { plugins: [tailwindcss()] },
});
