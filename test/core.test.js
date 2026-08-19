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
  const strippedBinary = rbx.removeProperty(
    dom.toXml(),
    "xml",
    "Part",
    "Anchored",
    "binary",
  );
  const strippedBinaryDom = rbx.readBinary(strippedBinary);
  const strippedBinaryPart = strippedBinaryDom.children(strippedBinaryDom.rootRef)[0];
  assert.equal(
    strippedBinaryDom.getProperty(strippedBinaryPart, "Anchored"),
    undefined,
  );
  assert.match(rbx.viewBinaryText(converted), /chunks:/);
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

test("XML reads expose internal-to-authored source referents", () => {
  const xml = Buffer.from(`
    <roblox version="4" xmlns:source="urn:source">
      <External><Item class="Folder" referent="Ignored" /></External>
      <Item class="Folder" referent="IgnoredFirst" source:referent="AuthoredTop">
        <Item class="Folder" referent="AuthoredNested" />
        <Item class="Folder" />
      </Item>
      <Item class="Folder" referent="AuthoredSibling" />
    </roblox>
    <roblox version="4"><Item class="Folder" referent="IgnoredTrailing" /></roblox>
  `);
  const dom = rbx.readXml(xml);
  const [top, sibling] = dom.children(dom.rootRef);
  const [nested, withoutReferent] = dom.children(top);

  assert.deepEqual(dom.sourceReferents(), {
    [nested]: "AuthoredNested",
    [sibling]: "AuthoredSibling",
    [top]: "AuthoredTop",
  });
  assert.equal(dom.sourceReferents()[withoutReferent], undefined);
  assert.deepEqual(rbx.readBinary(dom.toBinary()).sourceReferents(), {});
});

test("live instance handles, builders, raw DOMs, and external operations are available", () => {
  const source = rbx.createDom({
    className: "DataModel",
    children: [
      {
        className: "Folder",
        name: "Source",
        children: [{ className: "Part", name: "Block" }],
      },
    ],
  });
  const destination = rbx.createDom({
    className: "DataModel",
    children: [{ className: "Folder", name: "Destination" }],
  });
  const sourceFolder = source.children(source.rootRef)[0];
  const sourcePart = source.children(sourceFolder)[0];
  const destinationFolder = destination.children(destination.rootRef)[0];

  const instance = source.instanceObject(sourcePart);
  instance.setName("Renamed").setProperty("Anchored", true);
  assert.equal(source.instance(sourcePart).name, "Renamed");
  assert.deepEqual(instance.getProperty("Anchored"), { Bool: true });
  assert.deepEqual(
    source.ancestorsOf(sourcePart).map(({ name }) => name),
    ["Renamed", "Source", "DataModel"],
  );

  const builder = new rbx.InstanceBuilder("Folder")
    .setName("Built")
    .setProperty("Value", 5);
  source.insertBuilder(source.rootRef, builder);
  assert.equal(source.instanceCount, 4);

  const clone = source.cloneIntoExternal(sourceFolder, destination);
  assert.equal(destination.instance(clone).name, "Source");
  source.transfer(sourcePart, destination, destinationFolder);
  assert.equal(source.instance(sourcePart), null);
  assert.equal(destination.instance(sourcePart).parent, destinationFolder);

  const raw = source.raw();
  const restored = rbx.Dom.fromRaw(raw);
  assert.deepEqual(restored.raw(), raw);
  assert.equal(restored.instanceCount, source.instanceCount);
  assert.equal(restored.rootMut().name(), "DataModel");
});

test("viewer, attributes, full shared-string access, and custom reflection work", () => {
  const dom = rbx.createDom({ className: "DataModel" });
  const viewer = new rbx.DomViewer();
  assert.equal(viewer.view(dom).referent, "referent-0");

  const attributes = new rbx.Attributes({ Enabled: true });
  attributes.set("Count", 3);
  assert.deepEqual(attributes.get("Count"), { Int32: 3 });
  const decoded = rbx.Attributes.decode(attributes.encode());
  assert.deepEqual(decoded.toJSON(), {
    Count: { Int32: 3 },
    Enabled: { Bool: true },
  });

  const shared = rbx.types.sharedString(Buffer.from("payload"));
  assert.equal(rbx.types.sharedStringData(shared).toString(), "payload");
  assert.equal(rbx.types.sharedStringHashBytes(shared).length, 32);
  const net = rbx.types.netAssetRef(Buffer.from("asset"));
  assert.equal(rbx.types.netAssetRefData(net).toString(), "asset");

  const database = new rbx.ReflectionDatabase({
    Version: [0, 0, 0, 0],
    Classes: {},
    Enums: {},
  });
  assert.deepEqual(database.version(), [0, 0, 0, 0]);
  const xml = Buffer.from(
    '<roblox version="4"><Item class="Folder" referent="0"/></roblox>',
  );
  assert.equal(
    rbx.readXml(xml, {
      propertyBehavior: "noReflection",
      reflectionDatabase: database,
    }).instanceCount,
    2,
  );
  const binary = rbx.createDom({ className: "DataModel" }).toBinary({
    reflectionDatabase: database,
  });
  assert.equal(
    rbx.readBinary(binary, { reflectionDatabase: database }).instanceCount,
    1,
  );
});

test("reflection databases can be generated from an API dump", () => {
  const database = rbx.ReflectionDatabase.fromApiDump({
    Classes: [
      {
        Name: "Folder",
        Superclass: "Instance",
        Members: [
          {
            MemberType: "Property",
            Name: "Enabled",
            ValueType: { Name: "bool", Category: "Primitive" },
            Serialization: { CanSave: true, CanLoad: true },
            Security: { Read: "None", Write: "None" },
          },
        ],
      },
    ],
    Enums: [],
  });
  assert.deepEqual(database.version(), [0, 0, 0, 0]);
  assert.deepEqual(database.propertyNames("Folder"), ["Enabled"]);
  assert.equal(database.property("Folder", "Enabled").DataType.Value, "Bool");
});
