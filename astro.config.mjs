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
            { label: "Introduction", slug: "getting-started/introduction" },
            { label: "Installation", slug: "getting-started/installation" },
            { label: "Quick Start", slug: "getting-started/quickstart" },
          ],
        },
        {
          label: "Aether",
          items: [
            {
              label: "Configuration",
              items: [
                { label: "Agents", slug: "aether/configuration/agent-settings" },
                { label: "LLMs", slug: "aether/configuration/llm-providers" },
                { label: "Prompts", slug: "aether/configuration/system-prompts" },
                { label: "Tools", slug: "aether/configuration/mcp-servers" },
              ],
            },
            {
              label: "Built-in MCP Servers",
              items: [
                { label: "Coding", slug: "aether/built-in-servers/coding" },
                { label: "Skills & Commands", slug: "aether/built-in-servers/skills-commands" },
                { label: "Tasks", slug: "aether/built-in-servers/tasks" },
                { label: "Sub-Agents", slug: "aether/built-in-servers/subagents" },
              ],
            },
            {
              label: "Running",
              items: [
                { label: "TUI", slug: "aether/running/tui" },
                { label: "IDE (ACP)", slug: "aether/running/editor-integration" },
                { label: "Headless", slug: "aether/running/headless" },
              ],
            },
          ],
        },
        {
          label: "Wisp",
          items: [
            { label: "Overview", slug: "wisp/overview" },
            { label: "Keybindings & Commands", slug: "wisp/keybindings" },
            { label: "Git Diff View", slug: "wisp/git-diff" },
            { label: "Settings & Themes", slug: "wisp/settings" },
            { label: "Sessions", slug: "wisp/sessions" },
            { label: "Embedding Wisp", slug: "wisp/embedding" },
          ],
        },
        {
          label: "Libraries",
          items: [
            { label: "Architecture", slug: "libraries/architecture" },
            {
              label: "aether-core",
              collapsed: true,
              items: [
                { label: "Agent Builder", slug: "libraries/aether-core/agent-builder" },
                { label: "Events & Streaming", slug: "libraries/aether-core/events" },
                { label: "MCP Integration", slug: "libraries/aether-core/mcp-integration" },
              ],
            },
            {
              label: "llm",
              collapsed: true,
              items: [
                { label: "Provider Interface", slug: "libraries/llm/provider-interface" },
                { label: "Custom Providers", slug: "libraries/llm/custom-providers" },
              ],
            },
            {
              label: "mcp-servers",
              collapsed: true,
              items: [
                { label: "Embedding Servers", slug: "libraries/mcp-servers/embedding" },
              ],
            },
            {
              label: "tui",
              collapsed: true,
              items: [
                { label: "Components", slug: "libraries/tui/components" },
                { label: "Rendering", slug: "libraries/tui/rendering" },
              ],
            },
            {
              label: "crucible",
              collapsed: true,
              items: [
                { label: "Writing Evals", slug: "libraries/crucible/evals" },
              ],
            },
          ],
        },
      ],
    }),
  ],
  vite: { plugins: [tailwindcss()] },
});
