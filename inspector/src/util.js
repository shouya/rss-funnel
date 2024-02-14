export function elt(type, props, children) {
  if (!Array.isArray(children)) children = [children];

  let dom = document.createElement(type);
  if (props) {
    for (const name in props) dom.setAttribute(name, props[name]);
  }

  for (const child of children) {
    if (!child) continue;
    if (typeof child === "string") {
      dom.appendChild(document.createTextNode(child));
      continue;
    }
    dom.appendChild(child);
  }
  return dom;
}

export const $ = (selector, parent = document) =>
  parent.querySelector(selector);
