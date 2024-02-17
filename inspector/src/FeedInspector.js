import { elt, $ } from "./util.js";
import { Filter } from "./Filter.js";
import { basicSetup, EditorView } from "codemirror";
import { EditorState } from "@codemirror/state";
import { xml } from "@codemirror/lang-xml";
import Split from "split.js";

export class FeedInspector {
  constructor() {
    this.config = null;
    this.current_endpoint = null;
    this.current_preview = null;
    this.editor = null;
  }

  async init() {
    await this.setup_feed_editor();
    await this.setup_splitter();

    const resp = await fetch("/_inspector/config");
    this.config = await resp.json();

    await Promise.all([this.load_endpoints(), this.setup_param()]);
  }

  async setup_param() {
    [
      $("#request-param #source"),
      $("#request-param #limit-posts"),
      $("#request-param #limit-posts-checkbox"),
    ].forEach((input) => {
      input.addEventListener("change", () => this.render_preview());
    });

    $("#request-param #limit-filters").addEventListener("change", () => {
      this.render_filters();
      this.render_preview();
    });
    $("#request-param #limit-filters-checkbox").addEventListener(
      "change",
      () => {
        this.render_filters();
        this.render_preview();
      },
    );
  }

  async setup_feed_editor() {
    $("#feed-preview").classList.add("hidden");
    this.editor = new EditorView({
      extensions: [
        basicSetup,
        xml(),
        EditorState.readOnly.of(true),
        EditorView.lineWrapping,
      ],
      parent: $("#feed-preview"),
    });
  }

  async setup_splitter() {
    Split(["#sidebar-panel", "#main-panel"], {
      sizes: [20, 80],
      snapOffset: 0,
      gutterSize: 3,
      minSize: [300, 500],
    });
  }

  async load_endpoints() {
    $("#sidebar-filters").classList.add("hidden");
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

  render_filters() {
    if (!this.current_endpoint) return;
    const { filters } = this.current_endpoint;
    const limit =
      $("#limit-filters-checkbox").checked && $("#limit-filters").value;
    const filter_ul_node = $("#filter-list");
    filter_ul_node.innerHTML = "";
    for (const [index, filter] of filters.entries()) {
      const [name, conf] = Object.entries(filter)[0];
      const node = new Filter(name, conf).render_node();

      if (limit !== false && index >= limit) {
        node.classList.add("inactive");
      }

      filter_ul_node.appendChild(node);
    }
  }

  async render_preview() {
    if (!this.current_endpoint) return;
    const { path } = this.current_endpoint;

    const params = this.feed_request_param();
    $("#feed-preview").classList.remove("hidden");
    $("#feed-preview").classList.add("loading");

    const time_start = performance.now();
    const request_path = `${path}?${params}`;
    $("#fetch-status").innerText = `Fetching ${request_path}...`;
    const resp = await fetch(`${path}?${params}`);
    const content_type = resp.headers.get("content-type");
    const text = await resp.text();

    $("#fetch-status").innerText = `Fetched ${request_path} in ${
      performance.now() - time_start
    }ms. Content-type: ${content_type}`;

    this.editor.dispatch({
      changes: { from: 0, to: this.editor.state.doc.length, insert: text },
    });
    $("#feed-preview").classList.remove("loading");
  }

  async load_endpoint() {
    if (!this.current_endpoint) return;
    const { path, source, filters } = this.current_endpoint;

    // switch sidebarigation ui
    $("#sidebar-endpoints").classList.add("hidden");
    $("#endpoint-name").textContent = path;
    $("#back-to-endpoints").addEventListener("click", () => {
      this.current_endpoint = null;
      this.load_endpoints();
    });
    $("#copy-endpoint-url").addEventListener("click", () => {
      this.copy_endpoint_url(this.current_endpoint);
    });
    $("#sidebar-filters").classList.remove("hidden");

    // switch main ui
    $("input#source", $("#request-param")).disabled = !!source;
    if (source) {
      $("input#source", $("#request-param")).placeholder = source;
      $("input#source", $("#request-param")).value = "";
    } else {
      $("input#source", $("#request-param")).placeholder =
        "Source not configured. Please specify it here.";
    }
    $("#request-param").classList.remove("hidden");

    // update parameter input
    $("#limit-filters").setAttribute("max", filters.length);
    $("#limit-filters").value = filters.length;
    $("#limit-filters-checkbox").checked = false;

    // show filter list
    this.render_filters();

    // show feed preview
    this.render_preview();
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
    if (source) params.push(`source=${source}`);
    if (limit_posts) params.push(`limit_posts=${limit_posts}`);
    if (limit_filters) params.push(`limit_filters=${limit_filters}`);

    params.push("pp=1");
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
    sidebarigator.clipboard.writeText(url);
    const node = elt("div", { class: "popup-alert", style: "opacity: 1" }, [
      elt("div", { class: "alert-header" }, "URL Copied"),
      elt("div", { class: "alert-body" }, url),
    ]);
    document.body.appendChild(node);
    setTimeout(() => (node.style.opacity = 0), 3000);
    setTimeout(() => node.remove(), 4000);
  }
}
