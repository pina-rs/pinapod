<!-- {@podFeatureTable} -->

| Feature                          | Adds                                                                                    |
| -------------------------------- | --------------------------------------------------------------------------------------- |
| `compact-commit-full-validation` | The full semantic walk at the compact commit entry, instead of the layout-only default  |
| `fixed`                          | Mappings for signed and unsigned `fixed` 1.30.0 values, re-exported as `pinapod::fixed` |
| `floats`                         | `PodF32`/`PodF64` and mappings for native `f32`/`f64`                                   |
| `kani`                           | Kani proof harnesses, compiled by `cargo kani --features kani`                          |
| `solana-address`                 | A mapping for `solana_address::Address`                                                 |
| `solana-program-error`           | Conversion from `PinaPodError` to `ProgramError`                                        |
| `wincode`                        | Canonical `SchemaRead` and `SchemaWrite` implementations                                |

<!-- {/podFeatureTable} -->

<!-- {@podFeatureDefaultsContract} -->

No feature is enabled by default, so the core crate stays `no_std` and dependency-free.

Enable only what a program reads from or writes to the wire.

<!-- {/podFeatureDefaultsContract} -->
