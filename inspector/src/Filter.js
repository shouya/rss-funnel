import { elt, isBlankValue } from "./util.js";
import Prism from "prismjs";
import "prismjs/themes/prism.css";

export class Filter {
  constructor(name, config) {
    this.name = name;
    this.config = config;
  }

  render_node() {
    const header_node = elt("div", { class: "filter-header" }, this.name);
    const conf_node = elt(
      "div",
      { class: "filter-config" },
      this.render_config(),
    );
    if (conf_node.innerText === "") {
      conf_node.innerHTML = "No config";
    }

    return elt("li", { class: "filter" }, [header_node, conf_node]);
  }

  render_config() {
    const method = `render_${this.name}_config`;
    const output = this[method] ? this[method]() : this.render_default_config();

    if (output instanceof Element) {
      return output;
    } else if (typeof output === "string") {
      return output;
    } else if (output === null) {
      return null;
    } else {
      console.error(`Invalid output from ${method}`, output);
      return null;
    }
  }

  render_config_value(value) {
    if (value === null || value === undefined) {
      return highlight_json_value(value);
    }

    if (Array.isArray(value)) {
      return elt(
        "ul",
        {},
        value.map((v) => elt("li", {}, this.render_config_value(v))),
      );
    }

    if (typeof value === "object") {
      return this.render_config_object_value(value);
    }

    return highlight_json_value(value);
  }

  render_config_object_value(value) {
    const dl_node = elt("dl", {}, []);
    const non_blank_entries = Object.entries(value).filter(
      ([k, v]) => !isBlankValue(v),
    );
    for (const [k, v] of non_blank_entries) {
      const dt_node = elt("dt", {}, k);
      const dd_node = elt("dd", {}, this.render_config_value(v));

      dl_node.appendChild(dt_node);
      dl_node.appendChild(dd_node);
    }
    return dl_node;
  }

  render_default_config() {
    if (isBlankValue(this.config)) {
      return null;
    }

    return this.render_config_value(this.config);

    // return highlight_json(JSON.stringify(this.config, null, 2));
  }

  render_js_config() {
    return highlight_js(this.config);
  }
}

function highlight_json_value(value) {
  const json = JSON.stringify(value, null, 2);
  const html = Prism.highlight(json, Prism.languages.json, "json");
  const code_node = elt("code", {}, []);
  code_node.innerHTML = html;
  return code_node;
}

function highlight_js(code) {
  const html = Prism.highlight(code, Prism.languages.javascript, "js");
  const code_node = elt("code", {}, []);
  code_node.innerHTML = html;
  return elt("pre", {}, code_node);
}
