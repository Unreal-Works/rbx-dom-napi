"use strict";

import * as native from "./native/index.js";
export * as native from "./native/index.js";

function json(value) {
  return JSON.stringify(value);
}

function parse(value) {
  return JSON.parse(value);
}

export class Dom {
  constructor(spec = { className: "DataModel" }) {
    this._native =
      spec instanceof native.Dom ? spec : new native.Dom(json(spec));
  }

  static _fromNative(value) {
    const dom = Object.create(Dom.prototype);
    dom._native = value;
    return dom;
  }

  static fromXml(data, options) {
    if (options?.reflectionDatabase instanceof ReflectionDatabase) {
      const { reflectionDatabase, ...ioOptions } = options;
      return Dom._fromNative(
        native.readXmlWithDatabase(
          data,
          reflectionDatabase._native,
          json(ioOptions),
        ),
      );
    }
    return Dom._fromNative(
      native.readXml(data, options === undefined ? undefined : json(options)),
    );
  }

  static fromBinary(data, options) {
    if (options?.reflectionDatabase instanceof ReflectionDatabase) {
      const { reflectionDatabase, ...ioOptions } = options;
      return Dom._fromNative(
        native.readBinaryWithDatabase(
          data,
          reflectionDatabase._native,
          json(ioOptions),
        ),
      );
    }
    return Dom._fromNative(
      native.readBinary(
        data,
        options === undefined ? undefined : json(options),
      ),
    );
  }

  static fromRaw(value) {
    return Dom._fromNative(native.Dom.fromRaw(json(value)));
  }

  get rootRef() {
    return this._native.rootRef();
  }

  get instanceCount() {
    return this._native.instanceCount();
  }

  sourceReferents() {
    return parse(this._native.sourceReferents());
  }

  xmlVersion() {
    const value = this._native.xmlVersion();
    return value === null ? undefined : value;
  }

  root() {
    return parse(this._native.root());
  }

  snapshot() {
    return parse(this._native.snapshot());
  }

  toJSON() {
    return this.snapshot();
  }

  instance(referent) {
    const value = this._native.instance(referent);
    return value === null ? null : parse(value);
  }

  instanceObject(referent) {
    const value = this._native.instanceObject(referent);
    return value === null ? null : Instance._fromNative(value);
  }

  rootMut() {
    return Instance._fromNative(this._native.rootMut());
  }

  raw() {
    return parse(this._native.raw());
  }

  rawInstances() {
    return parse(this._native.rawInstances());
  }

  children(referent) {
    return this._native.children(referent);
  }

  descendants(referent) {
    const value = this._native.descendants(referent);
    return parse(value);
  }

  ancestorsOf(referent) {
    return parse(this._native.ancestorsOf(referent));
  }

  fullPath(referent, separator = ".") {
    return this._native.fullPath(referent, separator);
  }

  getProperty(referent, property) {
    const value = this._native.getProperty(referent, property);
    return value === null ? undefined : parse(value);
  }

  uniqueId(referent) {
    const value = this._native.uniqueId(referent);
    return value === null ? undefined : value;
  }

  setProperty(referent, property, value) {
    this._native.setProperty(referent, property, json(value));
    return this;
  }

  removeProperty(referent, property) {
    return this._native.removeProperty(referent, property);
  }

  setName(referent, name) {
    this._native.setName(referent, name);
    return this;
  }

  setClass(referent, className) {
    this._native.setClass(referent, className);
    return this;
  }

  insert(parent, spec) {
    return this._native.insert(parent, json(spec));
  }

  insertBuilder(parent, builder) {
    this._native.insertBuilder(parent, builder._native);
    return this;
  }

  reserve(additional) {
    this._native.reserve(additional);
    return this;
  }

  destroy(referent) {
    this._native.destroy(referent);
    return this;
  }

  cloneWithin(referent) {
    return this._native.cloneWithin(referent);
  }

  transferWithin(referent, parent) {
    this._native.transferWithin(referent, parent);
    return this;
  }

  transfer(referent, destination, parent) {
    this._native.transfer(referent, destination._native, parent);
    return this;
  }

  cloneIntoExternal(referent, destination) {
    return this._native.cloneIntoExternal(referent, destination._native);
  }

  cloneMultipleIntoExternal(referents, destination) {
    return this._native.cloneMultipleIntoExternal(
      referents,
      destination._native,
    );
  }

  view() {
    return parse(this._native.view());
  }

  toXml(options) {
    if (options?.reflectionDatabase instanceof ReflectionDatabase) {
      const { reflectionDatabase, ...ioOptions } = options;
      return this._native.toXmlWithDatabase(
        reflectionDatabase._native,
        json(ioOptions),
      );
    }
    return this._native.toXml(
      options === undefined ? undefined : json(options),
    );
  }

  toBinary(options) {
    if (options?.reflectionDatabase instanceof ReflectionDatabase) {
      const { reflectionDatabase, ...ioOptions } = options;
      return this._native.toBinaryWithDatabase(
        reflectionDatabase._native,
        json(ioOptions),
      );
    }
    return this._native.toBinary(
      options === undefined ? undefined : json(options),
    );
  }
}

export class Instance {
  constructor() {
    throw new TypeError("Instance objects are created by a Dom");
  }

  static _fromNative(value) {
    const instance = Object.create(Instance.prototype);
    instance._native = value;
    return instance;
  }

  referent() {
    return this._native.referent();
  }

  parent() {
    return this._native.parent();
  }

  children() {
    return this._native.children();
  }

  name() {
    return this._native.name();
  }

  className() {
    return this._native.className();
  }

  snapshot() {
    return parse(this._native.snapshot());
  }

  properties() {
    return parse(this._native.properties());
  }

  getProperty(property) {
    const value = this._native.getProperty(property);
    return value === null ? undefined : parse(value);
  }

  setProperty(property, value) {
    this._native.setProperty(property, json(value));
    return this;
  }

  removeProperty(property) {
    return this._native.removeProperty(property);
  }

  setName(name) {
    this._native.setName(name);
    return this;
  }

  setClass(className) {
    this._native.setClass(className);
    return this;
  }
}

export class InstanceBuilder {
  constructor(className, propertyCapacity) {
    this._native = new native.InstanceBuilder(className, propertyCapacity);
  }

  referent() {
    return this._native.referent();
  }

  className() {
    return this._native.className();
  }

  name() {
    return this._native.name();
  }

  setClass(className) {
    this._native.setClass(className);
    return this;
  }

  setName(name) {
    this._native.setName(name);
    return this;
  }

  setReferent(referent) {
    this._native.setReferent(referent);
    return this;
  }

  hasProperty(property) {
    return this._native.hasProperty(property);
  }

  getProperty(property) {
    const value = this._native.getProperty(property);
    return value === null ? undefined : parse(value);
  }

  setProperty(property, value) {
    this._native.setProperty(property, json(value));
    return this;
  }

  addProperty(property, value) {
    this._native.addProperty(property, json(value));
    return this;
  }

  addChild(child) {
    this._native.addChild(child._native);
    return this;
  }
}

export class DomViewer {
  constructor() {
    this._native = new native.DomViewer();
  }

  view(dom) {
    return parse(this._native.view(dom._native));
  }

  viewChildren(dom) {
    return parse(this._native.viewChildren(dom._native));
  }
}

export class Attributes {
  constructor(value = undefined) {
    this._native = new native.Attributes();
    if (value !== undefined) {
      for (const [key, entry] of Object.entries(value)) {
        this.set(key, entry);
      }
    }
  }

  static decode(data) {
    const value = Object.create(Attributes.prototype);
    value._native = native.attributesDecode(data);
    return value;
  }

  get(key) {
    const value = this._native.get(key);
    return value === null ? undefined : parse(value);
  }

  set(key, value) {
    const previous = this._native.set(key, json(value));
    return previous === null ? undefined : parse(previous);
  }

  remove(key) {
    const value = this._native.remove(key);
    return value === null ? undefined : parse(value);
  }

  clear() {
    this._native.clear();
    return this;
  }

  get length() {
    return this._native.length();
  }

  get isEmpty() {
    return this._native.isEmpty();
  }

  toJSON() {
    return parse(this._native.toJson());
  }

  encode() {
    return this._native.encode();
  }
}

export class ReflectionDatabase {
  constructor(value = undefined) {
    this._native = new native.ReflectionDatabase(
      value === undefined ? undefined : json(value),
    );
  }

  static fromBinary(data) {
    const value = Object.create(ReflectionDatabase.prototype);
    value._native = native.reflectionDatabaseFromBinary(data);
    return value;
  }

  static fromApiDump(value) {
    const result = Object.create(ReflectionDatabase.prototype);
    result._native = native.ReflectionDatabase.fromApiDump(json(value));
    return result;
  }

  version() {
    return this._native.version();
  }

  classNames() {
    return this._native.classNames();
  }

  enumNames() {
    return this._native.enumNames();
  }

  toJSON() {
    return parse(this._native.toJson());
  }

  class(name) {
    return parse(this._native.class(name));
  }

  property(className, propertyName) {
    return parse(this._native.property(className, propertyName));
  }

  defaultProperty(className, propertyName) {
    return parse(this._native.defaultProperty(className, propertyName));
  }

  propertyNames(className) {
    return this._native.propertyNames(className);
  }

  enum(name) {
    return parse(this._native.enum(name));
  }

  enumItems(name) {
    return parse(this._native.enumItems(name));
  }

  isA(className, superclassName) {
    return this._native.isA(className, superclassName);
  }

  superclasses(className) {
    return this._native.superclasses(className);
  }
}

const valueFunction =
  (name) =>
  (...args) =>
    parse(native[name](...args));
const valueWithJson =
  (name) =>
  (value, ...args) =>
    parse(native[name](json(value), ...args));

export const types = {
  vector2: valueFunction("vector2"),
  vector2int16: valueFunction("vector2int16"),
  vector3: valueFunction("vector3"),
  vector3int16: valueFunction("vector3int16"),
  vector3NormalId: (value) => native.vector3NormalId(json(value)),
  color3: valueFunction("color3"),
  color3uint8: valueFunction("color3uint8"),
  cframeIdentity: valueFunction("cframeIdentity"),
  cframeFromPosition: valueWithJson("cframeFromPosition"),
  cframeFromMatrix: (position, x, y, z) =>
    parse(native.cframeFromMatrix(json(position), json(x), json(y), json(z))),
  ray: (origin, direction) => parse(native.ray(json(origin), json(direction))),
  region3: (min, max) => parse(native.region3(json(min), json(max))),
  region3int16: (min, max) => parse(native.region3int16(json(min), json(max))),
  rect: (min, max) => parse(native.rect(json(min), json(max))),
  udim: valueFunction("udim"),
  udim2: (x, y) => parse(native.udim2(json(x), json(y))),
  numberRange: valueFunction("numberRange"),
  numberSequenceKeypoint: valueFunction("numberSequenceKeypoint"),
  numberSequence: (keypoints) => parse(native.numberSequence(json(keypoints))),
  colorSequenceKeypoint: (time, color) =>
    parse(native.colorSequenceKeypoint(time, json(color))),
  colorSequence: (keypoints) => parse(native.colorSequence(json(keypoints))),
  font: valueFunction("font"),
  fontRegular: valueFunction("fontRegular"),
  fontParts: (value) => parse(native.fontParts(json(value))),
  brickColorByName: valueFunction("brickColorByName"),
  brickColorByNumber: valueFunction("brickColorByNumber"),
  brickColorToColor3uint8: (value) =>
    parse(native.brickColorToColor3uint8(json(value))),
  enumValue: (value) => native.enumValue(value),
  enumItem: valueFunction("enumItem"),
  refFromString: (value) => native.refFromString(value),
  refNone: () => native.refNone(),
  refNew: () => native.refNew(),
  refIsSome: (value) => native.refIsSome(value),
  refIsNone: (value) => native.refIsNone(value),
  uniqueIdNow: () => native.uniqueIdNow(),
  uniqueId: (index, time, random) =>
    native.uniqueId(index, time, String(random)),
  uniqueIdParts: (value) => parse(native.uniqueIdParts(value)),
  axesFromBits: (bits) => parse(native.axesFromBits(bits)),
  axesBits: (value) => native.axesBits(json(value)),
  axesEmpty: () => parse(native.axesEmpty()),
  axesAll: () => parse(native.axesAll()),
  axesContains: (value, other) => native.axesContains(json(value), json(other)),
  facesFromBits: (bits) => parse(native.facesFromBits(bits)),
  facesBits: (value) => native.facesBits(json(value)),
  facesEmpty: () => parse(native.facesEmpty()),
  facesAll: () => parse(native.facesAll()),
  facesContains: (value, other) =>
    native.facesContains(json(value), json(other)),
  securityCapabilitiesBits: (bits) =>
    native.securityCapabilitiesBits(String(bits)),
  contentNone: valueFunction("contentNone"),
  contentUri: valueFunction("contentUri"),
  contentObject: valueFunction("contentObject"),
  contentId: valueFunction("contentId"),
  physicalProperties: valueFunction("physicalProperties"),
  binaryString: (data) => native.binaryString(data),
  sharedString: (data) => parse(native.sharedString(data)),
  sharedStringData: (value) => native.sharedStringData(json(value)),
  sharedStringHashBytes: (value) => native.sharedStringHashBytes(json(value)),
  sharedStringHash: (data) => native.sharedStringHash(data),
  netAssetRef: (data) => parse(native.netAssetRef(data)),
  netAssetRefData: (value) => native.netAssetRefData(json(value)),
  netAssetRefHashBytes: (value) => native.netAssetRefHashBytes(json(value)),
  netAssetRefHash: (data) => native.netAssetRefHash(data),
  tagsEncode: (tags) => native.tagsEncode(json(tags)),
  tagsDecode: (data) => parse(native.tagsDecode(data)),
  materialColorsDefault: valueFunction("materialColorsDefault"),
  materialColorsEncode: (value) => native.materialColorsEncode(json(value)),
  materialColorsDecode: (data) => parse(native.materialColorsDecode(data)),
  materialColorsGet: (value, material) =>
    parse(native.materialColorsGet(json(value), material)),
  materialColorsSet: (value, material, color) =>
    parse(native.materialColorsSet(json(value), material, json(color))),
  materialColorsDefaultValue: valueFunction("materialColorsDefault"),
  physicalPropertiesDefault: valueFunction("physicalPropertiesDefault"),
  physicalPropertiesParts: (value) => {
    const result = native.physicalPropertiesParts(json(value));
    return result === null ? undefined : parse(result);
  },
  contentKind: (value) => native.contentKind(json(value)),
  contentUriValue: (value) => native.contentUriValue(json(value)),
  contentObjectValue: (value) => native.contentObjectValue(json(value)),
  contentIdValue: (value) => native.contentIdValue(json(value)),
  matrixTranspose: (value) => parse(native.matrixTranspose(json(value))),
  matrixBasicRotationId: (value) => native.matrixBasicRotationId(json(value)),
  matrixFromBasicRotationId: (id) =>
    parse(native.matrixFromBasicRotationId(id)),
  tagsLength: (data) => native.tagsLength(data),
  tagsIsEmpty: (data) => native.tagsIsEmpty(data),
  uniqueIdNil: () => native.uniqueIdNil(),
  variant: (value) => parse(native.variant(json(value))),
  variantType: (value) => native.variantType(json(value)),
  taggedVariant: (type, value) => types.variant({ [type]: value }),
};

export const reflection = {
  version: () => native.reflectionVersion(),
  classNames: () => native.reflectionClassNames(),
  enumNames: () => native.reflectionEnumNames(),
  database: () => parse(native.reflectionDatabase()),
  class: (name) => parse(native.reflectionClass(name)),
  property: (className, propertyName) =>
    parse(native.reflectionProperty(className, propertyName)),
  defaultProperty: (className, propertyName) =>
    parse(native.reflectionDefaultProperty(className, propertyName)),
  propertyNames: (className) => native.reflectionPropertyNames(className),
  enum: (name) => parse(native.reflectionEnum(name)),
  enumItems: (name) => parse(native.reflectionEnumItems(name)),
  isA: (className, superclassName) =>
    native.reflectionIsA(className, superclassName),
  superclasses: (className) => native.reflectionSuperclasses(className),
  localDatabasePath: () => native.reflectionLocalDatabasePath(),
};

export function createDom(spec) {
  return new Dom(spec);
}

export const readXml = Dom.fromXml;
export const readBinary = Dom.fromBinary;

export function convertFile(data, fromFormat, toFormat, options) {
  return native.convertFile(
    data,
    fromFormat,
    toFormat,
    options === undefined ? undefined : json(options),
  );
}

export function viewBinary(data) {
  return parse(native.viewBinary(data));
}

export function viewBinaryText(data) {
  return native.viewBinaryText(data);
}

export function removeProperty(
  data,
  format,
  className,
  propertyName,
  outputFormat,
) {
  return native.removeProperty(
    data,
    format,
    className,
    propertyName,
    outputFormat,
  );
}

export const util = Object.freeze({
  convertFile,
  viewBinary,
  viewBinaryText,
  removeProperty,
});

export function metadata() {
  return parse(native.bindingMetadata());
}
