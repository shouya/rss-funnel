import { elt, $ } from "./util.js";
import { Filter } from "./Filter.js";
import { basicSetup, EditorView } from "codemirror";
import { EditorState } from "@codemirror/state";
import { xml } from "@codemirror/lang-xml";

export class FeedInspector {
  constructor() {
    this.config = null;
    this.current_endpoint = null;
    this.current_preview = null;
    this.editor = null;
  }

  async init() {
    const resp = await fetch("/_inspector/config");
    this.config = await resp.json();

    await Promise.all([
      this.setup_feed_editor(),
      this.load_endpoints(),
      this.setup_param_event_handler(),
    ]);
  }

  async setup_param_event_handler() {
    $("#request-param")
      .querySelectorAll("input")
      .forEach((input) => {
        input.addEventListener("change", () => this.load_endpoint());
      });
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

  async load_endpoints() {
    $("#nav-endpoints").classList.remove("hidden");
    $("#endpoint-list").innerHTML = "";

    for (const endpoint of this.config.endpoints) {
      const path_node = elt("div", { class: "endpoint-path" }, endpoint.path);
      path_node.addEventListener("click", () => {
        this.current_endpoint = endpoint;
        this.load_endpoint();
      });
      const copy_url_node = elt(
        "a",
        { class: "tool", href: endpoint.path },
        "copy",
      );
      copy_url_node.addEventListener("click", (e) => {
        e.preventDefault();
        this.copy_feed_url(endpoint);
      });

      const node = elt("li", { class: "endpoint" }, [
        elt("div", { class: "endpoint-header" }, [path_node, copy_url_node]),
        endpoint.note && elt("div", { class: "endpoint-note" }, endpoint.note),
      ]);
      $("#endpoint-list").appendChild(node);
    }
    $("#nav-endpoints").classList.remove("hidden");
  }

  async load_endpoint() {
    if (!this.current_endpoint) return;
    const { path, source, filters } = this.current_endpoint;

    // switch navigation ui
    $("#nav-endpoints").classList.add("hidden");
    $("#endpoint-name").textContent = path;
    $("#back-to-endpoints").addEventListener("click", () => {
      this.current_endpoint = null;
      this.load_endpoints();
    });
    $("#nav-filters").classList.remove("hidden");

    // switch main ui
    $("input#source", $("#request-param")).disabled = !!source;
    if (source) {
      $("input#source", $("#request-param")).placeholder = source;
      $("input#source", $("#request-param")).value = "";
    }
    $("#request-param").classList.remove("hidden");

    // show filter list
    const filter_ul_node = $("#filter-list");
    filter_ul_node.innerHTML = "";
    for (const filter of filters) {
      const [name, conf] = Object.entries(filter)[0];
      const node = new Filter(name, conf).render_node();
      filter_ul_node.appendChild(node);
    }

    // show feed preview
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

  async full_feed_url(endpoint) {
    const parent = $("#request-param");
    const source = $("#source", parent).value;
    let url = new URL(endpoint.path, window.location);

    if (!endpoint.source) {
      url.searchParams.set("source", source);
    }

    return url.href;
  }

  async copy_feed_url(endpoint) {
    const url = this.full_feed_url(endpoint);
    navigator.clipboard.writeText(url);
    alert("URL copied to clipboard");
  }
}
