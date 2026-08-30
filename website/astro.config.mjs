import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";

const site = "https://vellum.doriankarter.com";
const socialImage = `${site}/og-image.png`;

export default defineConfig({
  site,
  integrations: [
    starlight({
      title: "Vellum",
      description: "A fast, customizable terminal command palette built in Rust for live, searchable workflows.",
      favicon: "/favicon.png",
      logo: { src: "./src/assets/logo.png", alt: "" },
      customCss: ["./src/styles/starlight.css"],
      lastUpdated: true,
      editLink: { baseUrl: "https://github.com/dkarter/vellum/edit/main/website/" },
      social: [{ icon: "github", label: "GitHub", href: "https://github.com/dkarter/vellum" }],
      head: [
        { tag: "meta", attrs: { property: "og:image", content: socialImage } },
        { tag: "meta", attrs: { property: "og:image:type", content: "image/png" } },
        { tag: "meta", attrs: { property: "og:image:width", content: "1200" } },
        { tag: "meta", attrs: { property: "og:image:height", content: "630" } },
        {
          tag: "meta",
          attrs: {
            property: "og:image:alt",
            content: "Vellum terminal palette filtering live agent sessions",
          },
        },
        { tag: "meta", attrs: { name: "twitter:card", content: "summary_large_image" } },
        { tag: "meta", attrs: { name: "twitter:image", content: socialImage } },
      ],
      sidebar: [
        {
          label: "Start here",
          items: [
            { label: "Overview", slug: "docs" },
            { label: "Installation", slug: "docs/installation" },
            { label: "Quick start", slug: "docs/quick-start" },
          ],
        },
        {
          label: "Authoring",
          items: [
            { label: "Configuration", slug: "docs/configuration" },
            { label: "Palette authoring", slug: "docs/palette-authoring" },
            { label: "Sources", slug: "docs/sources" },
            { label: "Item templates", slug: "docs/item-templates" },
            { label: "Actions", slug: "docs/actions" },
            { label: "Filters and input", slug: "docs/filters-input" },
            { label: "Frecency", slug: "docs/frecency" },
          ],
        },
        {
          label: "Reference",
          items: [
            { label: "Official palettes", slug: "docs/official-palettes" },
            { label: "Schemas", slug: "docs/schemas" },
            { label: "CLI reference", slug: "docs/cli-reference" },
          ],
        },
      ],
    }),
  ],
});
