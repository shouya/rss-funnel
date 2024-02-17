import { FeedInspector } from "./FeedInspector.js";

document.addEventListener("DOMContentLoaded", () => {
  const inspector = new FeedInspector();
  inspector.init();

  // Expose the inspector object to the global scope for debugging
  window.inspector = inspector;
});
