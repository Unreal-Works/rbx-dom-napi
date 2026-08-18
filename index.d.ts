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
}

export class Dom {
  constructor(spec?: InstanceSpec);
  static fromXml(data: Uint8Array, options?: IoOptions): Dom;
  static fromBinary(data: Uint8Array): Dom;
  readonly rootRef: string;
  readonly instanceCount: number;
  snapshot(): DomSnapshot;
  instance(referent: string): InstanceView | null;
  children(referent: string): string[];
  descendants(referent?: string): InstanceView[];
  fullPath(referent: string, separator?: string): string;
  getProperty(referent: string, property: string): Variant | undefined;
  setProperty(
    referent: string,
    property: string,
    value: JsonValue | Variant,
  ): this;
  removeProperty(referent: string, property: string): boolean;
  setName(referent: string, name: string): this;
  setClass(referent: string, className: string): this;
  insert(parent: string, spec: InstanceSpec): string;
  destroy(referent: string): this;
  cloneWithin(referent: string): string;
  transferWithin(referent: string, parent: string): this;
  toXml(options?: IoOptions): Buffer;
  toBinary(options?: IoOptions): Buffer;
}

export interface ReflectionDatabase {
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
  brickColorByName(name: string): JsonValue;
  brickColorByNumber(number: number): JsonValue;
  enumValue(value: number): number;
  enumItem(type: string, value: number): JsonValue;
  refFromString(value: string): string;
  refNone(): string;
  uniqueIdNow(): string;
  uniqueId(index: number, time: number, random: string | number): string;
  uniqueIdParts(value: string): JsonValue;
  axesFromBits(bits: number): JsonValue;
  axesBits(value: JsonValue): number;
  facesFromBits(bits: number): JsonValue;
  facesBits(value: JsonValue): number;
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
  sharedStringHash(data: Uint8Array): string;
  netAssetRefHash(data: Uint8Array): string;
  tagsEncode(tags: string[]): Buffer;
  tagsDecode(data: Uint8Array): string[];
  materialColorsDefault(): JsonValue;
  materialColorsEncode(value: JsonValue): Buffer;
  materialColorsDecode(data: Uint8Array): JsonValue;
  variant(value: JsonValue): Variant;
  taggedVariant(type: string, value: JsonValue): Variant;
  variantType(value: JsonValue): string;
};

export const reflection: {
  version(): number[];
  classNames(): string[];
  enumNames(): string[];
  database(): ReflectionDatabase;
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
export function readBinary(data: Uint8Array): Dom;
export function convertFile(
  data: Uint8Array,
  fromFormat: string,
  toFormat: string,
  options?: IoOptions,
): Buffer;
export function viewBinary(data: Uint8Array): JsonValue;
export function removeProperty(
  data: Uint8Array,
  format: string,
  className: string,
  propertyName: string,
): Buffer;
export function metadata(): JsonValue;
