# CTLV

> Compact Type-Length-Value

CTLV is a format for binary [type-length-value](https://en.wikipedia.org/wiki/Type-length-value) data. Types are [compactly encoded](https://github.com/AljoschaMeyer/varu64-rs) unsigned 64 bit integers. Some of these types imply a certain length, so that the length does not have to be encoded explicitly. But in all cases, the length of a ctlv is fully self-describing, so parsers can safely skip over ctlvs of unknown types. The format itself does not assign any semantics to types, that can be done via separate tables.

## Specification

A ctlv consists of a `type` (unsigned 64 bit integer), a `length` (unsigned 64 bit integer), and a `value` (a sequence of `length` many bytes).

### Binary Encoding

The binary encodings is the concatenation of an encoding of the type, an encoding of the length (sometimes omitted) and the raw bytes of the value.

The `type` is an unsigned 64 bit integer, encoded as a [VarU64](https://github.com/AljoschaMeyer/varu64-rs). If `type` is `128` or more, it is followed by another VarU64 encoding the `length`. If `type` is less than `128`, the value of `length` is computed as `2 ^ (type >> 3)`. In both cases, the remainder of the encoding consists of `length` many bytes of payload (the `value`).

## Misc

There are eight length-skipping slots for each power of two from 2^0 to 2^15, leaving 120 1-byte types with explicitly encoded lengths.

Related work: This format is basically the [multiformats multihash](https://multiformats.io/multihash/) format, but without semantic assumptions, using a different varint, and adding the greedy optimization to enable dropping length bytes in certain cases.

The specification (this file) is licensed as [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/), the code in this repository is licensed under [AGPL-3.0](https://www.gnu.org/licenses/agpl-3.0.html)
