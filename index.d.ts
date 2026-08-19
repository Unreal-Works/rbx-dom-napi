/// JSON-compatible values emitted by upstream rbx_types Serde implementations.
export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };
export type Variant = { [type: string]: JsonValue };

export interface InstanceSpec {
  className: string;
  name?: string;
  referent?: string;
  properties?: Record<string, JsonValue | Variant>;
  children?: InstanceSpec[];
}

export interface InstanceView {
  referent: string;
  parent: string;
  children: string[];
  name: string;
  className: string;
  properties: Record<string, Variant>;
}

export interface DomSnapshot {
  rootRef: string;
  instances: InstanceView[];
}

export interface IoOptions {
  propertyBehavior?:
    | "ignoreUnknown"
    | "readUnknown"
    | "errorOnUnknown"
    | "noReflection";
  compression?: "lz4" | "none" | "zstd";
  includeRoot?: boolean;
  refs?: string[];
  reflectionDatabase?: ReflectionDatabaseValue | ReflectionDatabase;
}

export class Dom {
  constructor(spec?: InstanceSpec);
  static fromXml(data: Uint8Array, options?: IoOptions): Dom;
  static fromBinary(data: Uint8Array): Dom;
  static fromRaw(value: RawDom): Dom;
  readonly rootRef: string;
  readonly instanceCount: number;
  sourceReferents(): Record<string, string>;
  xmlVersion(): string | undefined;
  root(): InstanceView;
  rootMut(): Instance;
  snapshot(): DomSnapshot;
  instance(referent: string): InstanceView | null;
  instanceObject(referent: string): Instance | null;
  raw(): RawDom;
  rawInstances(): Record<string, InstanceView>;
  children(referent: string): string[];
  descendants(referent?: string): InstanceView[];
  ancestorsOf(referent: string): InstanceView[];
  fullPath(referent: string, separator?: string): string;
  getProperty(referent: string, property: string): Variant | undefined;
  uniqueId(referent: string): string | undefined;
  setProperty(
    referent: string,
    property: string,
    value: JsonValue | Variant,
  ): this;
  removeProperty(referent: string, property: string): boolean;
  setName(referent: string, name: string): this;
  setClass(referent: string, className: string): this;
  insert(parent: string, spec: InstanceSpec): string;
  insertBuilder(parent: string, builder: InstanceBuilder): this;
  reserve(additional: number): this;
  destroy(referent: string): this;
  cloneWithin(referent: string): string;
  transferWithin(referent: string, parent: string): this;
  transfer(referent: string, destination: Dom, parent: string): this;
  cloneIntoExternal(referent: string, destination: Dom): string;
  cloneMultipleIntoExternal(referents: string[], destination: Dom): string[];
  view(): ViewedInstance;
  toXml(options?: IoOptions): Buffer;
  toBinary(options?: IoOptions): Buffer;
}

export interface RawDom {
  rootRef: string;
  instances: Record<string, InstanceView>;
}

export interface ViewedInstance {
  referent: string;
  name: string;
  class: string;
  properties: Record<string, JsonValue>;
  children: ViewedInstance[];
}

export class Instance {
  referent(): string;
  parent(): string;
  children(): string[];
  name(): string;
  className(): string;
  snapshot(): InstanceView;
  properties(): Record<string, Variant>;
  getProperty(property: string): Variant | undefined;
  setProperty(property: string, value: JsonValue | Variant): this;
  removeProperty(property: string): boolean;
  setName(name: string): this;
  setClass(className: string): this;
}

export class InstanceBuilder {
  constructor(className?: string, propertyCapacity?: number);
  referent(): string;
  className(): string;
  name(): string;
  setClass(className: string): this;
  setName(name: string): this;
  setReferent(referent: string): this;
  hasProperty(property: string): boolean;
  getProperty(property: string): Variant | undefined;
  setProperty(property: string, value: JsonValue | Variant): this;
  addProperty(property: string, value: JsonValue | Variant): this;
  addChild(child: InstanceBuilder): this;
}

export class DomViewer {
  view(dom: Dom): ViewedInstance;
  viewChildren(dom: Dom): ViewedInstance[];
}

export class Attributes {
  constructor(value?: Record<string, JsonValue | Variant>);
  static decode(data: Uint8Array): Attributes;
  get(key: string): Variant | undefined;
  set(key: string, value: JsonValue | Variant): Variant | undefined;
  remove(key: string): Variant | undefined;
  clear(): this;
  readonly length: number;
  readonly isEmpty: boolean;
  toJSON(): Record<string, Variant>;
  encode(): Buffer;
}

export class ReflectionDatabase {
  constructor(value?: ReflectionDatabaseValue);
  static fromBinary(data: Uint8Array): ReflectionDatabase;
  static fromApiDump(value: JsonValue): ReflectionDatabase;
  version(): number[];
  classNames(): string[];
  enumNames(): string[];
  toJSON(): ReflectionDatabaseValue;
  class(name: string): JsonValue;
  property(className: string, propertyName: string): JsonValue;
  defaultProperty(className: string, propertyName: string): JsonValue;
  propertyNames(className: string): string[];
  enum(name: string): JsonValue;
  enumItems(name: string): JsonValue;
  isA(className: string, superclassName: string): boolean;
  superclasses(className: string): string[];
}

export interface ReflectionDatabaseValue {
  Version: number[];
  Classes: Record<string, JsonValue>;
  Enums: Record<string, JsonValue>;
}

import * as Native from "./native";
export const native: typeof Native;

export const types: {
  vector2(x: number, y: number): JsonValue;
  vector2int16(x: number, y: number): JsonValue;
  vector3(x: number, y: number, z: number): JsonValue;
  vector3int16(x: number, y: number, z: number): JsonValue;
  vector3NormalId(value: JsonValue): number | null;
  color3(r: number, g: number, b: number): JsonValue;
  color3uint8(r: number, g: number, b: number): JsonValue;
  cframeIdentity(): JsonValue;
  cframeFromPosition(position: JsonValue): JsonValue;
  cframeFromMatrix(
    position: JsonValue,
    x: JsonValue,
    y: JsonValue,
    z: JsonValue,
  ): JsonValue;
  ray(origin: JsonValue, direction: JsonValue): JsonValue;
  region3(min: JsonValue, max: JsonValue): JsonValue;
  region3int16(min: JsonValue, max: JsonValue): JsonValue;
  rect(min: JsonValue, max: JsonValue): JsonValue;
  udim(scale: number, offset: number): JsonValue;
  udim2(x: JsonValue, y: JsonValue): JsonValue;
  numberRange(min: number, max?: number): JsonValue;
  numberSequenceKeypoint(
    time: number,
    value: number,
    envelope: number,
  ): JsonValue;
  numberSequence(keypoints: JsonValue[]): JsonValue;
  colorSequenceKeypoint(time: number, color: JsonValue): JsonValue;
  colorSequence(keypoints: JsonValue[]): JsonValue;
  font(family: string, weight: number, style: number): JsonValue;
  fontRegular(family: string): JsonValue;
  fontParts(value: JsonValue): JsonValue;
  brickColorByName(name: string): JsonValue;
  brickColorByNumber(number: number): JsonValue;
  brickColorToColor3uint8(value: JsonValue): JsonValue;
  enumValue(value: number): number;
  enumItem(type: string, value: number): JsonValue;
  refFromString(value: string): string;
  refNone(): string;
  refNew(): string;
  refIsSome(value: string): boolean;
  refIsNone(value: string): boolean;
  uniqueIdNow(): string;
  uniqueId(index: number, time: number, random: string | number): string;
  uniqueIdParts(value: string): JsonValue;
  axesFromBits(bits: number): JsonValue;
  axesBits(value: JsonValue): number;
  axesEmpty(): JsonValue;
  axesAll(): JsonValue;
  axesContains(value: JsonValue, other: JsonValue): boolean;
  facesFromBits(bits: number): JsonValue;
  facesBits(value: JsonValue): number;
  facesEmpty(): JsonValue;
  facesAll(): JsonValue;
  facesContains(value: JsonValue, other: JsonValue): boolean;
  securityCapabilitiesBits(bits: string | number): string;
  contentNone(): JsonValue;
  contentUri(uri: string): JsonValue;
  contentObject(referent: string): JsonValue;
  contentId(value: string): JsonValue;
  physicalProperties(
    density: number,
    friction: number,
    elasticity: number,
    frictionWeight: number,
    elasticityWeight: number,
    acousticAbsorption: number,
  ): JsonValue;
  binaryString(data: Uint8Array): Buffer;
  sharedString(data: Uint8Array): string;
  sharedStringData(value: JsonValue): Buffer;
  sharedStringHashBytes(value: JsonValue): Buffer;
  sharedStringHash(data: Uint8Array): string;
  netAssetRef(data: Uint8Array): string;
  netAssetRefData(value: JsonValue): Buffer;
  netAssetRefHashBytes(value: JsonValue): Buffer;
  netAssetRefHash(data: Uint8Array): string;
  tagsEncode(tags: string[]): Buffer;
  tagsDecode(data: Uint8Array): string[];
  materialColorsDefault(): JsonValue;
  materialColorsEncode(value: JsonValue): Buffer;
  materialColorsDecode(data: Uint8Array): JsonValue;
  materialColorsGet(value: JsonValue, material: string): JsonValue;
  materialColorsSet(
    value: JsonValue,
    material: string,
    color: JsonValue,
  ): JsonValue;
  physicalPropertiesDefault(): JsonValue;
  physicalPropertiesParts(value: JsonValue): JsonValue | undefined;
  contentKind(value: JsonValue): string;
  contentUriValue(value: JsonValue): string | undefined;
  contentObjectValue(value: JsonValue): string | undefined;
  contentIdValue(value: JsonValue): string;
  matrixTranspose(value: JsonValue): JsonValue;
  matrixBasicRotationId(value: JsonValue): number | null;
  matrixFromBasicRotationId(id: number): JsonValue;
  tagsLength(data: Uint8Array): number;
  tagsIsEmpty(data: Uint8Array): boolean;
  uniqueIdNil(): string;
  variant(value: JsonValue): Variant;
  taggedVariant(type: string, value: JsonValue): Variant;
  variantType(value: JsonValue): string;
};

export const reflection: {
  version(): number[];
  classNames(): string[];
  enumNames(): string[];
  database(): ReflectionDatabaseValue;
  class(name: string): JsonValue;
  property(className: string, propertyName: string): JsonValue;
  defaultProperty(className: string, propertyName: string): JsonValue;
  propertyNames(className: string): string[];
  enum(name: string): JsonValue;
  enumItems(name: string): JsonValue;
  isA(className: string, superclassName: string): boolean;
  superclasses(className: string): string[];
  localDatabasePath(): string | undefined;
};

export function createDom(spec?: InstanceSpec): Dom;
export function readXml(data: Uint8Array, options?: IoOptions): Dom;
export function readBinary(data: Uint8Array, options?: IoOptions): Dom;
export function convertFile(
  data: Uint8Array,
  fromFormat: string,
  toFormat: string,
  options?: IoOptions,
): Buffer;
export function viewBinary(data: Uint8Array): JsonValue;
export function viewBinaryText(data: Uint8Array): string;
export function removeProperty(
  data: Uint8Array,
  format: string,
  className: string,
  propertyName: string,
  outputFormat?: string,
): Buffer;
export const util: {
  convertFile: typeof convertFile;
  viewBinary: typeof viewBinary;
  viewBinaryText: typeof viewBinaryText;
  removeProperty: typeof removeProperty;
};
export function metadata(): JsonValue;
