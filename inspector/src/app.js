import { elt, $ } from "./util.js";
import { basicSetup, EditorView } from "codemirror";
import { EditorState } from "@codemirror/state";
import { xml } from "@codemirror/lang-xml";

class FeedInspector {
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
      this.update_endpoints(),
      this.setup_param_event_handler(),
    ]);
  }

  async setup_param_event_handler() {
    $("#request-param")
      .querySelectorAll("input")
      .forEach((input) => {
        input.addEventListener("change", () => this.load_preview());
      });
  }

  async setup_feed_editor() {
    $("#feed-preview").style.visibility = "hidden";
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

  async update_endpoints() {
    $("#endpoint-list").innerHTML = "";
    for (const endpoint of this.config.endpoints) {
      const path_node = elt("div", { class: "endpoint-path" }, endpoint.path);
      path_node.addEventListener("click", () => {
        this.current_endpoint = endpoint;
        this.load_preview();
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
  }

  async load_preview() {
    if (!this.current_endpoint) return;

    const path = this.current_endpoint.path;
    const params = this.feed_request_param();
    const resp = await fetch(`${path}?${params}`);
    const text = await resp.text();

    this.editor.dispatch({
      changes: { from: 0, to: this.editor.state.doc.length, insert: text },
    });

    $("#feed-preview").style.visibility = "visible";
    $("#request-param").style.visibility = "visible";
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

  async copy_feed_url(endpoint) {
    const parent = $("#request-param");
    const source = $("#source", parent).value;
    let base = new URL(endpoint.path, window.location);

    if (!endpoint.source) {
      base.searchParams.set("source", source);
    }

    const url = base.href;
    navigator.clipboard.writeText(url);
    alert("URL copied to clipboard");
  }
}

document.addEventListener("DOMContentLoaded", () => {
  const inspector = new FeedInspector();
  inspector.init();

  // Expose the inspector object to the global scope for debugging
  window.inspector = inspector;
});
