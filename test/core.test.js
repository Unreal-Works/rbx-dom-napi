import assert from "node:assert/strict";
import test from "node:test";
import * as rbx from "../index.js";

test("typed values retain upstream rbx_types representations", () => {
  const vector = rbx.types.vector3(1, 2, 3);
  assert.deepEqual(vector, [1, 2, 3]);

  const tagged = rbx.types.taggedVariant("Vector3", vector);
  assert.equal(rbx.types.variantType(tagged), "Vector3");
  assert.deepEqual(rbx.types.variant(tagged), tagged);
  assert.deepEqual(
    rbx.types.tagsDecode(rbx.types.tagsEncode(["a", "a", "b"])),
    ["a", "a", "b"],
  );
});

test("DOM mutations and XML/binary round trips preserve instances and properties", () => {
  const dom = rbx.createDom({
    className: "DataModel",
    name: "game",
    children: [
      {
        className: "Part",
        name: "Block",
        properties: {
          Anchored: true,
          Size: rbx.types.taggedVariant("Vector3", rbx.types.vector3(2, 3, 4)),
        },
      },
    ],
  });

  const part = dom.children(dom.rootRef)[0];
  assert.equal(dom.fullPath(part), "Block");
  assert.deepEqual(dom.getProperty(part, "Size"), { Vector3: [2, 3, 4] });

  const clone = dom.cloneWithin(part);
  dom.setName(clone, "Copy");
  assert.equal(dom.instanceCount, 2);
  assert.equal(dom.instance(clone).name, "Copy");
  assert.equal(dom.fullPath(clone), "Copy");

  const xmlRoundTrip = rbx.readXml(dom.toXml());
  assert.equal(xmlRoundTrip.instanceCount, 2);
  const xmlPart = xmlRoundTrip.children(xmlRoundTrip.rootRef)[0];
  assert.deepEqual(xmlRoundTrip.getProperty(xmlPart, "Size"), {
    Vector3: [2, 3, 4],
  });

  const binaryRoundTrip = rbx.readBinary(dom.toBinary());
  assert.equal(binaryRoundTrip.instanceCount, 2);
  const binaryPart = binaryRoundTrip.children(binaryRoundTrip.rootRef)[0];
  assert.deepEqual(binaryRoundTrip.getProperty(binaryPart, "Size"), {
    Vector3: [2, 3, 4],
  });
});

test("bundled reflection database and utility bindings are available", () => {
  assert.deepEqual(rbx.reflection.version().slice(0, 3), [0, 728, 0]);
  assert.equal(rbx.reflection.isA("Part", "Instance"), true);
  assert.ok(rbx.reflection.classNames().includes("Part"));
  assert.ok(rbx.reflection.propertyNames("BasePart").includes("Size"));

  const dom = rbx.createDom({
    className: "DataModel",
    children: [{ className: "Part", properties: { Anchored: true } }],
  });
  const converted = rbx.convertFile(dom.toXml(), "xml", "binary");
  const viewed = rbx.viewBinary(converted);
  assert.equal(typeof viewed, "object");
  assert.ok(Array.isArray(viewed.chunks));

  const stripped = rbx.removeProperty(dom.toXml(), "xml", "Part", "Anchored");
  const strippedDom = rbx.readXml(stripped);
  const strippedPart = strippedDom.children(strippedDom.rootRef)[0];
  assert.equal(strippedDom.getProperty(strippedPart, "Anchored"), undefined);
});

test("DOM validation prevents upstream invariant-breaking operations", () => {
  const dom = rbx.createDom({ className: "DataModel" });
  const missingParent = "1".repeat(32);
  const candidate = "2".repeat(32);

  assert.throws(() =>
    dom.insert(missingParent, { className: "Folder", referent: candidate }),
  );
  assert.equal(dom.instance(candidate), null);

  assert.throws(() =>
    rbx.createDom({
      className: "DataModel",
      referent: "3".repeat(32),
      children: [{ className: "Folder", referent: "3".repeat(32) }],
    }),
  );

  const parent = "4".repeat(32);
  const child = "5".repeat(32);
  const tree = rbx.createDom({
    className: "DataModel",
    children: [
      {
        className: "Folder",
        referent: parent,
        children: [{ className: "Folder", referent: child }],
      },
    ],
  });
  assert.throws(() => tree.transferWithin(parent, child));
  assert.deepEqual(tree.children(tree.rootRef), [parent]);
});

test("camelCase options and large integer variants stay lossless", () => {
  const empty = rbx.createDom({ className: "DataModel" });
  assert.equal(empty.toXml().toString().includes('class="DataModel"'), false);
  assert.equal(
    empty.toXml({ includeRoot: true }).toString().includes('class="DataModel"'),
    true,
  );

  const unknownXml = Buffer.from(
    '<roblox version="4"><Item class="Folder" referent="0"><Properties><string name="FutureProperty">hello</string></Properties></Item></roblox>',
  );
  const ignored = rbx.readXml(unknownXml);
  const ignoredFolder = ignored.children(ignored.rootRef)[0];
  assert.equal(ignored.getProperty(ignoredFolder, "FutureProperty"), undefined);
  const retained = rbx.readXml(unknownXml, { propertyBehavior: "readUnknown" });
  const retainedFolder = retained.children(retained.rootRef)[0];
  assert.deepEqual(retained.getProperty(retainedFolder, "FutureProperty"), {
    String: "hello",
  });

  const large = "9007199254740993";
  const dom = rbx.createDom({
    className: "DataModel",
    children: [
      { className: "Folder", properties: { Value: { Int64: large } } },
    ],
  });
  const folder = dom.children(dom.rootRef)[0];
  assert.deepEqual(dom.getProperty(folder, "Value"), { Int64: large });
  assert.deepEqual(rbx.types.variant({ Int64: large }), { Int64: large });
});
