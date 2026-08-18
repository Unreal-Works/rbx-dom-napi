import assert from "node:assert/strict";
import test from "node:test";
import { createDom, types } from "rbx-dom";

test("a consumer can build and query a Roblox DOM", () => {
  const dom = createDom({
    className: "DataModel",
    children: [
      {
        className: "Folder",
        name: "Example",
        properties: {
          Color: types.taggedVariant("Color3", types.color3(1, 0, 0)),
        },
      },
    ],
  });
  const folder = dom.children(dom.rootRef)[0];
  assert.equal(dom.fullPath(folder), "Example");
  assert.deepEqual(dom.getProperty(folder, "Color"), { Color3: [1, 0, 0] });
});
