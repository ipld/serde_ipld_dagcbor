# Changelog

## [0.7.0] - 2026-08-03

### Changed

- **Breaking:** deserialization is now as strict as possible and will error on non-spec-compliant DAG-CBOR. If things break, please check your data producer. Last resort is enabling the newly introduced `less-strict-decoding` feature.
- **Breaking:** align with DAG-CBOR spec and disallow `-0.0`, serialize it as `0.0` instead.
- **Breaking:** the `error` enum of the decoder changed due to uprading `cbor4ii` from v0.2.14 to v1.2.2.
- The MSRV is now v1.81.

### Removed

- **Breaking:** the `codec` feature was removed, the `Codec` trait is now implemented by default. If you've used that feature, just remove the usage.

[0.7.0]: https://github.com/ipld/serde_ipld_dagcbor/compare/v0.6.4...v0.7.0
