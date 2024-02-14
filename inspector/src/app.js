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

    this.setup_feed_editor();
    this.update_endpoints();
  }

  async setup_feed_editor() {
    this.editor = new EditorView({
      extensions: [basicSetup, xml(), EditorState.readOnly.of(true)],
      parent: $("#feed-preview"),
    });
  }

  async update_endpoints() {
    $("#endpoint-list").innerHTML = "";
    for (const endpoint of this.config.endpoints) {
      const path_node = elt("div", { class: "endpoint-path" }, endpoint.path);
      path_node.addEventListener("click", () =>
        this.load_preview(endpoint.path),
      );

      const node = elt("li", { class: "endpoint" }, [
        path_node,
        endpoint.note && elt("div", { class: "endpoint-note" }, endpoint.note),
      ]);
      $("#endpoint-list").appendChild(node);
    }
  }

  async load_preview(path) {
    this.current_endpoint = path;
    const resp = await fetch(`${path}?pp=1`);
    const text = await resp.text();

    this.editor.dispatch({
      changes: { from: 0, to: this.editor.state.doc.length, insert: text },
    });
  }
}

document.addEventListener("DOMContentLoaded", () => {
  const inspector = new FeedInspector();
  inspector.init();

  // Expose the inspector object to the global scope for debugging
  window.inspector = inspector;
});
