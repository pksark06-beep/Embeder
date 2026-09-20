# Contributing to Embeder

Embeder is built around one invariant: probabilistic generation may propose firmware,
but only deterministic oracles can promote it to verified. Changes must preserve that
boundary and report failures honestly.

## Development setup

Install Rust, Python 3, Arm GNU `arm-none-eabi-gcc`, and Renode. Native Windows shell
work also requires the MSVC C++ Build Tools and the Rust MSVC toolchain.

```powershell
git clone https://github.com/pksark06-beep/Embeder.git
cd Embeder
cargo test --workspace --all-targets
```

Run the real integration suite when changing compiler, simulator, MCP, workspace, or
desktop task behavior:

```powershell
cargo test --workspace -- --ignored --test-threads=1
```

Build the native shell:

```powershell
cd desktop\src-tauri
cargo +stable-x86_64-pc-windows-msvc tauri build --target x86_64-pc-windows-msvc
```

## Read before changing architecture

- [`docs/SYSTEM_SPEC.md`](docs/SYSTEM_SPEC.md) defines the product contract.
- [`docs/adr/`](docs/adr/) records architectural decisions.
- [`desktop/ARCHITECTURE.md`](desktop/ARCHITECTURE.md) describes the desktop boundary.
- [`docs/INSTALLATION.md`](docs/INSTALLATION.md) describes the supported user setup.

## Pull request expectations

- Keep model output outside the trust boundary.
- Add or update tests for every behavioral change.
- Preserve path checks and bounded inputs around workspace and MCP operations.
- Report unavailable external tools explicitly; do not replace real work with a fixture.
- Keep secrets out of commits, logs, fixtures, and screenshots.
- Update documentation when commands, environment variables, or dependencies change.
- Confirm you have the right to contribute each submitted file and use
  `git commit -s` to add a Developer Certificate of Origin sign-off.
- Keep the MPL-2.0 license notice on existing source files. New Embeder source
  files should include an `SPDX-License-Identifier: MPL-2.0` comment.

Small, focused pull requests are easiest to verify. Include the commands you ran and any
hardware or toolchain assumptions needed to reproduce the result.

Contributors keep copyright in their contributions and license them under
[MPL-2.0](LICENSE). A sign-off records your certification under the
[Developer Certificate of Origin](https://developercertificate.org/); it does
not assign your copyright to the maintainers. The Embeder name and logo follow
the separate [trademark policy](TRADEMARKS.md).

## Support

Bug reports, reproducible firmware cases, documentation fixes, and new board profiles
are welcome. If you also want to fund continued development, visit
[buymeacoffee.com/sekweb](https://buymeacoffee.com/sekweb).
