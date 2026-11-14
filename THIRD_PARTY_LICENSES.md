# Third-Party Licences

This project includes the following direct dependencies:

| Crate | Version | Licence | Repository |
|-------|---------|---------|------------|
| [dirs](https://github.com/soc/dirs-rs) | 6.0.0 | MIT OR Apache-2.0 | https://github.com/soc/dirs-rs |
| [image](https://github.com/image-rs/image) | 0.24.9 | MIT OR Apache-2.0 | https://github.com/image-rs/image |
| [libcosmic](https://github.com/pop-os/libcosmic) | 0.1.0 (git) | MPL-2.0 | https://github.com/pop-os/libcosmic |
| [serde](https://github.com/serde-rs/serde) | 1.0.228 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| [serde_json](https://github.com/serde-rs/json) | 1.0.149 | MIT OR Apache-2.0 | https://github.com/serde-rs/json |
| [systemstat](https://github.com/valpackett/systemstat) | 0.2.5 | Unlicense | https://github.com/valpackett/systemstat |
| [thiserror](https://github.com/dtolnay/thiserror) | 2.0.17 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| [tokio](https://github.com/tokio-rs/tokio) | 1.49.0 | MIT | https://github.com/tokio-rs/tokio |
| [tracing](https://github.com/tokio-rs/tracing) | 0.1.44 | MIT | https://github.com/tokio-rs/tracing |
| [tracing-subscriber](https://github.com/tokio-rs/tracing) | 0.3.22 | MIT | https://github.com/tokio-rs/tracing |

> **Note:** libcosmic does not declare a licence in its Cargo.toml metadata, but
> the repository is published under the MPL-2.0 licence as stated in its
> [LICENCE file](https://github.com/pop-os/libcosmic/blob/master/LICENSE).
> The systemstat crate is released under the Unlicense, which dedicates the work
> to the public domain.

## Artwork

The runner sprites are taken unmodified from
[RunCat365](https://github.com/runcat-dev/RunCat365) and are licensed under
the Apache License 2.0. A copy of that licence is in
`resources/LICENSE-RunCat365`:

- `resources/cat-run-0.png` to `resources/cat-run-4.png`
- `resources/horse-run-0.png` to `resources/horse-run-4.png`
- `resources/parrot-run-0.png` to `resources/parrot-run-9.png`

`resources/cat-sleep.png` and `resources/horse-sleep.png` are original
drawings made in the same style by `tools/draw_sleep_sprites.py`.
