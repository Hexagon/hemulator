import lume from "lume/mod.ts";
import lumocs from "lumocs/mod.ts";

const site = lume({
  src: "src",
  location: new URL("https://hexagon.github.io/hemulator/"),
});

site.use(lumocs());

// Global metadata for SEO
site.data("metas", {
  site: "Hemulator - Multi-System Console Emulator",
  lang: "en",
  generator: true,
});

export default site;
