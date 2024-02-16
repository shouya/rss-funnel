import { elt } from "./util.js";
import Prism from "prismjs";
import "prismjs/themes/prism.css";

export class Filter {
  constructor(name, config) {
    this.name = name;
    this.config = config;
  }

  render_node() {
    const header_node = elt("div", { class: "filter-header" }, this.name);
    const conf_html = this.render_config();
    const conf_node = conf_html && elt("div", { class: "filter-config" }, []);
    if (conf_html) conf_node.innerHTML = conf_html;

    return elt("li", { class: "filter" }, [header_node, conf_node]);
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
    return highlight_json(JSON.stringify(this.config, null, 2));
  }

  render_js_config() {
    return highlight_js(this.config);
  }
}

function highlight_json(code) {
  const html = Prism.highlight(code, Prism.languages.json, "json");
  return `<pre><code>${html}</code></pre>`;
}
function highlight_js(code) {
  const html = Prism.highlight(code, Prism.languages.javascript, "js");
  return `<pre><code>${html}</code></pre>`;
}
