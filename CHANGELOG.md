# Changelog

All notable changes to Embeder are documented here.

## 0.2.0 — 2026-09-17

### Added

- Native Tauri 2 workbench for Windows.
- Shared Rust desktop API used by both the native shell and development server.
- Sandboxed per-user STM32F4 starter workspace.
- Native stdio MCP workers for datasheet, firmware compilation, and simulation.
- Real workspace builds with Arm GNU `arm-none-eabi-gcc`.
- Renode simulation with USART2 evidence and a verifiability boundary.
- BYOK model drafting with deterministic grounded synthesis fallback.
- Windows NSIS and MSI installers with bundled SVD, template, and codegen resources.
- Public documentation site, installation guide, and contributor guide.

### Verification

- 21 default workspace tests pass.
- 11 real Arm GCC, Renode, desktop-worker, and MCP integration tests pass.
- The packaged Windows executable passes a responsive-startup smoke test.

### Known limitations

- The Windows community installer is not Authenticode-signed.
- Arm GNU and Renode are external runtime dependencies.
- The packaged workspace is currently fixed to the STM32F4 Discovery profile.
- MCP workers use application-level path isolation, not an operating-system sandbox.
