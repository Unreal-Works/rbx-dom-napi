# rbx-dom-napi

Expose [rojo-rbx/rbx-dom](https://github.com/rojo-rbx/rbx-dom) through N-API.

## Install

```sh
npm install rbx-dom
```

The package uses the platform-specific native package produced by `napi-rs`. Referents, `UniqueId` values, 64-bit values, and 128-bit values are represented as strings so JavaScript cannot silently lose precision. Typed Roblox values use the upstream human-readable Serde representation. For example:

```js
const { createDom, types } = require("rbx-dom");

const dom = createDom({
  className: "DataModel",
  children: [
    {
      className: "Part",
      properties: {
        Size: types.taggedVariant("Vector3", types.vector3(2, 3, 4)),
      },
    },
  ],
});

const part = dom.children(dom.rootRef)[0];
console.log(dom.getProperty(part, "Size")); // { Vector3: [2, 3, 4] }
const xml = dom.toXml();
const binary = dom.toBinary({ compression: "zstd" });
```

The reusable reflector path is available through
`ReflectionDatabase.fromApiDump(apiDump)`, and utility functions are also
grouped under the `util` export (`convertFile`, `viewBinaryText`, and
`removeProperty`).
