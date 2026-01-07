import lume from "lume/mod.ts";
import lumocs from "lumocs/mod.ts";
import metas from "lume/plugins/metas.ts";
import sitemap from "lume/plugins/sitemap.ts";

const site = lume({
  src: "src",
  location: new URL("https://hexagon.github.io/hemulator/"),
});

site.use(lumocs());
site.use(metas());
site.use(sitemap());

// Global metadata for SEO
site.data("metas", {
  site: "Hemulator - Multi-System Console Emulator",
  lang: "en",
  generator: true,
});

export default site;
