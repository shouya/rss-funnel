import { elt, $, $$ } from "./util.js";
import { Filter } from "./Filter.js";
import { basicSetup, EditorView } from "codemirror";
import { EditorState } from "@codemirror/state";
import { xml } from "@codemirror/lang-xml";
import { json } from "@codemirror/lang-json";
import Split from "split.js";
import HtmlSanitizer from "jitbit-html-sanitizer";
import JSONSchemaView from "json-schema-view-js";
import "json-schema-view-js/src/style.less";

export class FeedInspector {
  constructor() {
    this.config = null;
    this.config_error = null;
    this.feed_error = null;
    this.filter_schema = null;
    this.current_endpoint = null;
    this.raw_editor = null;
    this.json_preview_editor = null;
    this.preview = null;
  }

  async init() {
    this.setup_raw_editor();
    this.setup_json_preview_editor();
    this.setup_splitter();
    this.setup_view_mode_selector();
    this.setup_reload_config_handler();

    window.debug = this;

    await this.reload_config();

    await Promise.all([this.load_endpoints(), this.setup_param()]);
  }

  async reload_config() {
    const [resp, filter_schema] = await Promise.all([
      fetch("/_inspector/config"),
      fetch("/_inspector/filter_schema?filters=all"),
    ]);

    const resp_json = await resp.json();
    this.config_error = resp_json.config_error;
    this.config = resp_json.root_config;
    this.filter_schema = await filter_schema.json();

    if (this.config_error) {
      $("#config-error-message").innerText = this.config_error;
      $("#config-error").classList.remove("hidden");
      return;
    } else {
      $("#config-error").classList.add("hidden");
      $("#config-error-message").innerText = "";
    }

    if (!this.config) {
      console.error("Failed to load config");
      return;
    }

    if (this.config.auth) {
      $("#logout-button").classList.remove("hidden");
    } else {
      $("#logout-button").classList.add("hidden");
    }

    if (!this.current_endpoint) {
      await this.load_endpoints();
      await this.reset_main_ui();
    }

    for (const endpoint of this.config.endpoints) {
      if (this.current_endpoint?.path === endpoint.path) {
        this.current_endpoint = endpoint;
        this.update_request_param_controls();
        this.render_filters();
        this.render_feed_source();
        this.fetch_and_render_feed();
        return;
      }
    }

    // current endpoint was deleted, reset everything
    this.current_endpoint = null;
    await this.load_endpoints();
    await this.reset_main_ui();
  }

  async setup_reload_config_handler() {
    $("#reload-config-button").addEventListener("click", () => {
      this.reload_config();
    });
  }

  async setup_view_mode_selector() {
    for (const node of $$("#view-mode-selector input")) {
      node.addEventListener("change", () => this.render_feed());
    }
  }

  async setup_param() {
    [
      $("#request-param #source"),
      $("#request-param #limit-posts"),
      $("#request-param #limit-posts-checkbox"),
    ].forEach((input) => {
      input.addEventListener("change", () => this.fetch_and_render_feed());
    });

    $("#request-param #limit-filters").addEventListener("change", () => {
      this.render_filters();
      this.fetch_and_render_feed();
    });
    $("#request-param #limit-filters-checkbox").addEventListener(
      "change",
      () => {
        this.render_filters();
        this.fetch_and_render_feed();
      },
    );
  }

  async setup_raw_editor() {
    this.raw_editor = new EditorView({
      extensions: [
        basicSetup,
        xml(),
        EditorState.readOnly.of(true),
        EditorView.lineWrapping,
      ],
      parent: $("#feed-preview #raw"),
    });
  }

  async setup_json_preview_editor() {
    this.json_preview_editor = new EditorView({
      extensions: [
        basicSetup,
        json(),
        EditorState.readOnly.of(true),
        EditorView.lineWrapping,
      ],
      parent: $("#feed-preview #json"),
    });
  }

  async setup_splitter() {
    Split(["#sidebar-panel", "#main-panel"], {
      sizes: [20, 80],
      snapOffset: 0,
      gutterSize: 3,
      dragInterval: 3,
    });
  }

  async load_endpoints() {
    $("#sidebar-endpoint").classList.add("hidden");
    $("#sidebar-endpoints").classList.remove("hidden");
    $("#endpoint-list").innerHTML = "";

    for (const endpoint of this.config.endpoints) {
      const path_node = elt("div", { class: "endpoint-path" }, endpoint.path);
      path_node.addEventListener("click", () => {
        this.current_endpoint = endpoint;
        this.load_endpoint();
      });
      const copy_url_node = elt(
        "div",
        { class: "button", href: endpoint.path },
        "Copy URL",
      );
      copy_url_node.addEventListener("click", (e) => {
        e.preventDefault();
        this.copy_endpoint_url(endpoint);
      });

      const node = elt("li", { class: "endpoint" }, [
        elt("div", { class: "endpoint-header" }, [path_node, copy_url_node]),
        endpoint.note && elt("div", { class: "endpoint-note" }, endpoint.note),
      ]);
      $("#endpoint-list").appendChild(node);
    }
    $("#sidebar-endpoints").classList.remove("hidden");
  }

  async reset_main_ui() {
    $("#endpoint-name").textContent = "";
    $("#main-panel").classList.add("hidden");
  }

  render_filters() {
    if (!this.current_endpoint) return;
    const { filters } = this.current_endpoint;
    const limit =
      $("#limit-filters-checkbox").checked && $("#limit-filters").value;
    const filter_ul_node = $("#filter-list");
    filter_ul_node.innerHTML = "";

    for (const [index, filter] of filters.entries()) {
      const [name, conf] = Object.entries(filter)[0];
      let schema_view = new JSONSchemaView(
        this.filter_schema[name],
        3,
      ).render();
      schema_view = elt("div", { class: "filter-schema-view" }, schema_view);

      const node = new Filter(name, conf).render_node();
      node.appendChild(schema_view);

      if (limit !== false && index >= limit) {
        node.classList.add("inactive");
      }

      filter_ul_node.appendChild(node);
    }
  }

  async fetch_and_render_feed() {
    await this.fetch_feed_preview();
    this.render_feed();
  }

  async render_feed() {
    const view_mode =
      ($("#view-mode-selector #rendered-radio-input").checked && "rendered") ||
      ($("#view-mode-selector #raw-radio-input").checked && "raw") ||
      ($("#view-mode-selector #json-radio-input").checked && "json") ||
      "rendered";

    ["rendered", "raw", "json"].forEach((mode) => {
      if (mode === view_mode) {
        $(`#feed-preview #${mode}`).classList.remove("hidden");
      } else {
        $(`#feed-preview #${mode}`).classList.add("hidden");
      }

      const preview = this.preview;
      const function_name = `render_feed_${mode}`;
      if (this[function_name]) {
        this[function_name](preview);
      }
    });
  }

  async render_feed_rendered({ unified }) {
    $("#feed-preview #rendered").innerHTML = "";
    const title_node = elt(
      "h3",
      { class: "feed-title" },
      elt("a", { href: unified.link }, unified.title),
    );
    const description_node = elt(
      "div",
      { class: "feed-description" },
      unified.description,
    );

    $("#feed-preview #rendered").appendChild(title_node);
    $("#feed-preview #rendered").appendChild(description_node);

    const sanitizer = new HtmlSanitizer({});

    for (const post of unified.posts) {
      const post_body = elt("div", { class: "feed-post-body" }, []);
      post_body.innerHTML = sanitizer.sanitizeHtml(post.body || "");

      let expand = elt("span", { class: "feed-post-show-all" }, "(expand)");
      expand.addEventListener("click", (e) => {
        post_body.classList.toggle("expanded");
        expand.innerText = post_body.classList.contains("expanded")
          ? "(collapse)"
          : "(expand)";
      });

      const post_node = elt("div", { class: "feed-post" }, [
        elt(
          "h3",
          { class: "feed-post-title" },
          elt("a", { class: "feed-post-link", href: post.link }, post.title),
        ),
        elt("div", { class: "feed-post-date" }, post.date),
        post_body,
        expand,
      ]);
      $("#feed-preview #rendered").appendChild(post_node);
    }
  }

  async render_feed_raw({ raw }) {
    if (this.raw_editor.state.doc.toString() === raw) {
      return;
    }
    this.raw_editor.dispatch({
      changes: {
        from: 0,
        to: this.raw_editor.state.doc.length,
        insert: raw,
      },
    });
  }

  async render_feed_json({ json }) {
    json = JSON.stringify(json, null, 2);
    if (this.json_preview_editor.state.doc.toString() === json) {
      return;
    }
    this.json_preview_editor.dispatch({
      changes: {
        from: 0,
        to: this.json_preview_editor.state.doc.length,
        insert: json,
      },
    });
  }

  update_request_param_controls() {
    if (!this.current_endpoint) return;

    const { source, filters } = this.current_endpoint;

    // switch main ui
    $("#request-param input#source").disabled = !!source;
    if (!source) {
      $("#request-param input#source").value = "";
      $("#request-param input#source").placeholder =
        "Please specify the feed source here.";
      $("#request-param input#source").readOnly = false;
    } else if (typeof source === "string") {
      $("#request-param input#source").value = source;
      $("#request-param input#source").readOnly = true;
    } else {
      $("#request-param input#source").value = "";
      $("#request-param input#source").placeholder =
        "The source is a feed from scratch";
      $("#request-param input#source").readOnly = true;
    }

    // update parameter input
    $("#limit-filters").setAttribute("max", filters.length);
    $("#limit-filters").value = filters.length;
    $("#limit-filters-checkbox").checked = false;
  }

  async load_endpoint() {
    if (!this.current_endpoint) return;
    const { path } = this.current_endpoint;

    // switch sidebar ui
    $("#sidebar-endpoints").classList.add("hidden");
    $("#endpoint-name").textContent = path;
    $("#back-to-endpoints").addEventListener("click", () => {
      this.load_endpoints();
    });
    $("#copy-endpoint-url").addEventListener("click", () => {
      this.copy_endpoint_url(this.current_endpoint);
    });
    $("#sidebar-endpoint").classList.remove("hidden");

    // show feed source
    this.render_feed_source();

    // show filter list
    this.render_filters();

    // show main ui
    this.update_request_param_controls();
    $("#main-panel").classList.remove("hidden");

    // show feed preview
    await this.fetch_feed_preview();
    this.render_feed();
  }

  async render_feed_source() {
    if (!this.current_endpoint) return;
    const { source } = this.current_endpoint;
    const header = $("#source-info > header");
    const content = $("#source-info > content");

    content.innerHTML = "";

    if (!source) {
      header.innerText = "Dynamic source";
      content.innerHTML =
        "Please specify it in the <code>source</code> parameter.";
      return;
    }

    // if source is a string, then render it as an <a>
    if (typeof source === "string") {
      header.innerText = "Source";
      content.appendChild(
        elt(
          "a",
          { class: "feed-link", href: source, target: "_blank" },
          source,
        ),
      );
      return;
    }

    // if source is an object with a format field, indicate the source is a blank feed
    // with specified title, link, and description field.
    if (source.format) {
      header.innerText = "Feed from scratch";
      const table = elt("table", { class: "scratch-feed" }, [
        elt("tr", { class: "feed-property" }, [
          elt("th", {}, "Title"),
          elt("td", {}, source.title),
        ]),
        elt("tr", { class: "feed-property" }, [
          elt("th", {}, "Link"),
          elt("td", {}, source.link),
        ]),
        elt("tr", { class: "feed-property" }, [
          elt("th", {}, "Description"),
          elt("td", {}, source.description),
        ]),
      ]);
      content.appendChild(table);
      return;
    }
  }

  async fetch_feed_preview() {
    if (!this.current_endpoint) return;
    const { path } = this.current_endpoint;

    const params = this.feed_request_param();
    $("#feed-preview").classList.add("loading");

    const time_start = performance.now();
    $("#fetch-status").innerText = `Fetching preview for ${path}...`;

    const resp = await fetch(`/_inspector/preview?endpoint=${path}&${params}`);
    let status_text = "";

    if (resp.status != 200) {
      status_text = `Failed fetching ${path} (status: ${resp.status} ${resp.statusText})`;
      this.update_feed_error(await resp.text());
    } else {
      this.update_feed_error(null);
      const preview = await resp.json();
      this.preview = preview;
      status_text = `Fetched feed with ${preview.post_count} posts from ${path} (${preview.content_type})`;
    }

    status_text += ` in ${performance.now() - time_start}ms.`;
    $("#fetch-status").innerText = status_text;
    $("#feed-preview").classList.remove("loading");
  }

  update_feed_error(error) {
    if (error) {
      $("#feed-error").classList.remove("hidden");
      $("#feed-error-message").innerText = error;
    } else {
      $("#feed-error-message").innerText = "";
      $("#feed-error").classList.add("hidden");
    }
  }

  feed_request_param() {
    const parent = $("#request-param");
    const source = $("#source", parent).value;
    const limit_posts =
      $("#limit-posts-checkbox", parent).checked &&
      $("#limit-posts", parent).value;
    const limit_filters =
      $("#limit-filters-checkbox", parent).checked &&
      $("#limit-filters", parent).value;

    const params = [];
    if (!this.current_endpoint.source && source)
      params.push(`source=${source}`);
    if (limit_posts) params.push(`limit_posts=${limit_posts}`);
    if (limit_filters) params.push(`limit_filters=${limit_filters}`);

    return params.join("&");
  }

  full_feed_url(endpoint) {
    const parent = $("#request-param");
    const source = $("#source", parent).value;
    let url = new URL(endpoint.path, window.location);

    if (!endpoint.source) {
      url.searchParams.set("source", source);
    }

    return url.href;
  }

  copy_endpoint_url(endpoint) {
    const url = this.full_feed_url(endpoint);
    navigator.clipboard.writeText(url);
    const node = elt("div", { class: "popup-alert", style: "opacity: 1" }, [
      elt("div", { class: "alert-header" }, "URL Copied"),
      elt("div", { class: "alert-body" }, url),
    ]);
    document.body.appendChild(node);
    setTimeout(() => (node.style.opacity = 0), 3000);
    setTimeout(() => node.remove(), 4000);
  }
}
