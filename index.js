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
    return Dom._fromNative(
      native.readXml(data, options === undefined ? undefined : json(options)),
    );
  }

  static fromBinary(data) {
    return Dom._fromNative(native.readBinary(data));
  }

  get rootRef() {
    return this._native.rootRef();
  }

  get instanceCount() {
    return this._native.instanceCount();
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

  children(referent) {
    return this._native.children(referent);
  }

  descendants(referent) {
    const value = this._native.descendants(referent);
    return parse(value);
  }

  fullPath(referent, separator = ".") {
    return this._native.fullPath(referent, separator);
  }

  getProperty(referent, property) {
    const value = this._native.getProperty(referent, property);
    return value === null ? undefined : parse(value);
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

  toXml(options) {
    return this._native.toXml(
      options === undefined ? undefined : json(options),
    );
  }

  toBinary(options) {
    return this._native.toBinary(
      options === undefined ? undefined : json(options),
    );
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
  brickColorByName: valueFunction("brickColorByName"),
  brickColorByNumber: valueFunction("brickColorByNumber"),
  enumValue: (value) => native.enumValue(value),
  enumItem: valueFunction("enumItem"),
  refFromString: (value) => native.refFromString(value),
  refNone: () => native.refNone(),
  uniqueIdNow: () => native.uniqueIdNow(),
  uniqueId: (index, time, random) =>
    native.uniqueId(index, time, String(random)),
  uniqueIdParts: (value) => parse(native.uniqueIdParts(value)),
  axesFromBits: (bits) => parse(native.axesFromBits(bits)),
  axesBits: (value) => native.axesBits(json(value)),
  facesFromBits: (bits) => parse(native.facesFromBits(bits)),
  facesBits: (value) => native.facesBits(json(value)),
  securityCapabilitiesBits: (bits) =>
    native.securityCapabilitiesBits(String(bits)),
  contentNone: valueFunction("contentNone"),
  contentUri: valueFunction("contentUri"),
  contentObject: valueFunction("contentObject"),
  contentId: valueFunction("contentId"),
  physicalProperties: valueFunction("physicalProperties"),
  binaryString: (data) => native.binaryString(data),
  sharedStringHash: (data) => native.sharedStringHash(data),
  netAssetRefHash: (data) => native.netAssetRefHash(data),
  tagsEncode: (tags) => native.tagsEncode(json(tags)),
  tagsDecode: (data) => parse(native.tagsDecode(data)),
  materialColorsDefault: valueFunction("materialColorsDefault"),
  materialColorsEncode: (value) => native.materialColorsEncode(json(value)),
  materialColorsDecode: (data) => parse(native.materialColorsDecode(data)),
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

export const removeProperty = native.removeProperty;

export function metadata() {
  return parse(native.bindingMetadata());
}
