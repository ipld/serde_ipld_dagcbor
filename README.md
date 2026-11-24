Serde IPLD DAG-CBOR
===================

[![Crates.io](https://img.shields.io/crates/v/serde_ipld_dagcbor.svg)](https://crates.io/crates/serde_ipld_dagcbor)
[![Documentation](https://docs.rs/serde_ipld_dagcbor/badge.svg)](https://docs.rs/serde_ipld_dagcbor)

This is a [Serde] implementation for [DAG-CBOR]. It can be use in conjunction with [ipld-core].

The underlying library for CBOR encoding/decoding is [cbor4ii]. Their Serde implementation is used as a basis. A copy of their code was copied into this repo on April 2022 and is used under the MIT/Apache-2.0 license, with permission from [@quininer].

This crate started as a fork of [serde_cbor], thanks everyone involved there.

[Serde]: https://github.com/serde-rs/serde
[DAG-CBOR]: https://ipld.io/specs/codecs/dag-cbor/spec/
[ipld-core]: https://github.com/ipld/rust-ipld-core
[cbor4ii]: https://github.com/quininer/cbor4ii
[@quininer]: https://github.com/quininer/
[serde_cbor]: https://github.com/pyfisch/cbor


Usage
-----

Storing and loading Rust types is easy and requires only
minimal modifications to the program code.

```rust
use serde_derive::{Deserialize, Serialize};
use std::error::Error;
use std::fs::File;
use std::io::BufReader;

// Types annotated with `Serialize` can be stored as DAG-CBOR.
// To be able to load them again add `Deserialize`.
#[derive(Debug, Serialize, Deserialize)]
struct Mascot {
    name: String,
    species: String,
    year_of_birth: u32,
}

fn main() -> Result<(), Box<dyn Error>> {
    let ferris = Mascot {
        name: "Ferris".to_owned(),
        species: "crab".to_owned(),
        year_of_birth: 2015,
    };

    let ferris_file = File::create("examples/ferris.cbor")?;
    // Write Ferris to the given file.
    // Instead of a file you can use any type that implements `io::Write`
    // like a HTTP body, database connection etc.
    serde_ipld_dagcbor::to_writer(ferris_file, &ferris)?;

    let tux_file = File::open("examples/tux.cbor")?;
    let tux_reader = BufReader::new(tux_file);
    // Load Tux from a file.
    // Serde IPLD DAG-CBOR performs roundtrip serialization meaning that
    // the data will not change in any way.
    let tux: Mascot = serde_ipld_dagcbor::from_reader(tux_reader)?;

    println!("{:?}", tux);
    // prints: Mascot { name: "Tux", species: "penguin", year_of_birth: 1996 }

    Ok(())
}
```


Features
--------

### `less-strict-decoding`

The decoding of data is as strict as possible. When enabling this feature, it's a bit less strict. If you run into decoding errors for your not fully DAG-CBOR compliant data, try this feature flag first.


### `no-cid-as-bytes`

Sometimes it is desired that a CID is not accidentally deserialized into bytes. This can happen because the intermediate serde data model does not retain enough information to be able to differentiate between a bytes container and a CID container when there is a conflicting choice to be made, as in the case of some enum cases. The `no-cid-as-bytes` feature can be enabled in order to error at runtime in such cases.

The problem with that feature is, that it breaks Serde's derive attributes for [internally tagged enums](https://serde.rs/enum-representations.html#internally-tagged) (`#[serde(tag = "sometag")]`) and [untagged enums](https://serde.rs/enum-representations.html#untagged) (`#serde(untagged)`). If this feature is enabled and you still need similar functionality, you could implement a deserializer manually. Examples of how to do that are in the [enum example](examples/enums.rs).


### `_experimental-integral-floats`

This feature is experimental, it may change or be removed in a non-breaking release.

It adds options to treat floats with an integral value as integers, both are off by default:

 - `Deserializer::integral_floats(true)` decodes integers into `f32`/`f64` fields if they are exactly representable. That's all integers up to 2^24 (`f32`) or 2^53 (`f64`) in absolute value, larger ones only if they don't lose precision, e.g. 2^53 + 2 is representable as `f64`, 2^53 + 1 isn't. Integers below -2^63 are rejected. Integers decoded into other types, e.g. `i64` or `Ipld`, stay integers.
 - `Serializer::integral_floats(true)` encodes floats with an integral value that fits into `u64`/`i64` as integers, e.g. `4.0` as `4`. Hence `Ipld::Float(4.0)` is decoded as `Ipld::Integer(4)`.

`from_slice_integral_floats`/`from_reader_integral_floats` in `de` and `to_vec_integral_floats`/`to_writer_integral_floats` in `ser` enable the options, all other functions use the defaults.

When serde buffers values (e.g. for untagged enums or `#[serde(flatten)]`), it converts numbers into floats itself, which may lose precision. Use `de::lossless_f32`/`de::lossless_f64` with `#[serde(deserialize_with = "…")]` for those fields.

The options don't change what is valid DAG-CBOR. The encoded data is still valid DAG-CBOR, and only valid DAG-CBOR is decoded. DAG-CBOR distinguishes integers and floats though, so with `Serializer::integral_floats(true)` a value like `4.0` is encoded as the integer `4`. Other DAG-CBOR decoders see it as an integer, and the encoded bytes, and hence the CID, differ from the default encoding. The encoding is similar to the numeric reduction of [dCBOR], but floats are always encoded as 64-bit, NaN and Infinity are rejected, and integral floats encoded as floats are still accepted when decoding.

[dCBOR]: https://datatracker.ietf.org/doc/draft-mcnally-deterministic-cbor/


License
-------

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
