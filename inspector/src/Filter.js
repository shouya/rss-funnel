import { elt } from "./util.js";

export class Filter {
  constructor(name, config) {
    this.name = name;
    this.config = config;
  }

  render_config() {
    const method = `render_${this.name}_config`;
    if (this[method]) {
      return this[method]();
    } else {
      return this.render_default_config();
    }
  }

  render_default_config() {
    return elt("pre", {}, JSON.stringify(this.config, null, 2));
  }

  render_js_config() {
    return elt(
      "pre",
      {},
      elt("code", { class: "language-javascript" }, this.config),
    );
  }
}
