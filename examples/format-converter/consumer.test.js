const assert = require("node:assert/strict");
const test = require("node:test");

const { createDom, convertFile, readBinary, readXml } = require("rbx-dom");

test("a consumer can convert XML to binary and back", () => {
  const source = createDom({
    className: "DataModel",
    children: [{ className: "Part", name: "ConvertedPart" }],
  });
  const xml = source.toXml();
  const binary = convertFile(xml, "xml", "binary", { compression: "zstd" });
  const xmlAgain = convertFile(binary, "binary", "xml");
  const result = readXml(xmlAgain);
  const part = result.children(result.rootRef)[0];
  const binaryResult = readBinary(binary);
  const binaryPart = binaryResult.children(binaryResult.rootRef)[0];

  assert.equal(binaryResult.instance(binaryPart).name, "ConvertedPart");
  assert.equal(result.instance(part).className, "Part");
});
