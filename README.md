# rbx-dom-napi

Expose [rojo-rbx/rbx-dom](https://github.com/rojo-rbx/rbx-dom) through N-API.

## Install

```sh
npm install rbx-dom
```

## Usage

```js
const { createDom, readXml, types } = require("rbx-dom");

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

> [!WARNING]
> Referents, `UniqueId` values, 64-bit values, and 128-bit values are represented as strings so JavaScript cannot silently lose precision.

### XML

We retain the mapping from generated internal referents:

```js
const dom = readXml(xml);
const internalReferent = dom.children(dom.rootRef)[0];
const authoredReferent = dom.sourceReferents()[internalReferent];
```
