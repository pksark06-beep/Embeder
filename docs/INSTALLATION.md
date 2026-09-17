# Installing Embeder

Embeder v0.2.0 is available as a Windows x64 desktop application. The installer contains
the workbench, the bundled STM32F4 datasheet excerpt, starter firmware, and the native
MCP workers. Rust, Cargo, Visual Studio, and MSVC are build-time dependencies only; they
are not required to run the installed application.

## 1. Install the desktop application

Download `Embeder_0.2.0_x64-setup.exe` from the
[latest GitHub release](https://github.com/pksark06-beep/Embeder/releases/latest).

The NSIS setup installs Embeder for the current Windows user under `%LOCALAPPDATA%` and
normally does not require administrator privileges. Windows 10 or Windows 11 x64 is
recommended.

The v0.2.0 community build is not Authenticode-signed. If Windows marks the download as
an unfamiliar publisher, first confirm that it came from the official repository and
verify its SHA-256 digest:

```text
6E1CBEF44BEF8DAC29F943E31A77E72E7AFFAEECBFF452EE9FC1067323858540  Embeder_0.2.0_x64-setup.exe
```

Organizations that prohibit unsigned applications can build Embeder from source using
the instructions below.

## 2. Install the verification tools

The workbench, editor, datasheet views, and deterministic grounding are available after
the desktop install. The real verification stages use two external engineering tools:

| Tool | Required for | Validated baseline |
| --- | --- | --- |
| [Arm GNU Toolchain](https://developer.arm.com/downloads/-/arm-gnu-toolchain-downloads) | Build | 12.2.1 |
| [Renode](https://github.com/renode/renode/releases) | Build & Simulate | 1.16.x |
| [Python](https://www.python.org/downloads/windows/) | Model-backed generation only | 3.12+ |

### Arm GNU Toolchain

Embeder looks for `arm-none-eabi-gcc` in this order:

1. `EMBEDER_GCC`, when set to the full compiler path.
2. `arm-none-eabi-gcc` on `PATH`.
3. `%LOCALAPPDATA%\Programs\ArmGNU\bin\arm-none-eabi-gcc.exe`.

To configure an explicit compiler path for your user account:

```powershell
[Environment]::SetEnvironmentVariable(
  "EMBEDER_GCC",
  "C:\path\to\ArmGNU\bin\arm-none-eabi-gcc.exe",
  "User"
)
```

Open a new Embeder process after changing an environment variable.

### Renode

Embeder first checks `EMBEDER_RENODE`, then the default Windows installation path:

```text
C:\Program Files\Renode\bin\Renode.exe
```

If Renode is elsewhere, configure it for your user account:

```powershell
[Environment]::SetEnvironmentVariable(
  "EMBEDER_RENODE",
  "C:\path\to\Renode.exe",
  "User"
)
```

## 3. Run the first verification

1. Launch Embeder from the Start menu.
2. Open the bundled `stm32-blink-uart` workspace.
3. Edit and save `src/main.c`.
4. Select **Build** to compile a real ELF with Arm GCC.
5. Select **Build & Simulate** to boot the ELF in Renode and check the USART2 output.
6. Inspect the Problems, Output, Citations, Evidence, and Verifiability Boundary views.

The installed workspace is stored under the current user's Embeder application-data
directory. The bundled starter is copied once; later application launches do not
overwrite user edits.

## 4. Optional model providers

No provider is required. Without an API key, Embeder uses deterministic grounded
synthesis. For model-backed drafting, install Python and set one supported key before
launching Embeder:

```powershell
$env:GEMINI_API_KEY = "..."
# or
$env:OPENAI_API_KEY = "..."
# or
$env:AI_GATEWAY_API_KEY = "..."
```

Optional overrides:

```powershell
$env:EMBEDER_LLM_PROVIDER = "gemini"  # gemini | openai | vercel | none
$env:EMBEDER_LLM_MODEL = "provider-model-name"
```

Generated firmware remains an untrusted draft until the compiler and simulator stages
pass. Generation never overwrites the saved workspace automatically.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| Build reports the compiler unavailable | Run `arm-none-eabi-gcc --version`, or set `EMBEDER_GCC`. |
| Simulation reports Renode unavailable | Confirm `Renode.exe` exists, or set `EMBEDER_RENODE`. |
| Model generation falls back to synthesis | Confirm Python and the selected provider key are visible to the Embeder process. |
| The desktop opens but renders an empty window | Install or repair Microsoft Edge WebView2, then relaunch. |
| A build fails | Read the returned compiler diagnostics; Embeder deliberately preserves real failures. |

## Build from source

Source builds require Git, Rust, the MSVC C++ Build Tools and Tauri CLI v2:

```powershell
git clone https://github.com/pksark06-beep/Embeder.git
cd Embeder

rustup toolchain install stable-x86_64-pc-windows-msvc
cargo install tauri-cli --version "^2" --locked
cargo test --workspace

cd desktop\src-tauri
cargo +stable-x86_64-pc-windows-msvc tauri build --target x86_64-pc-windows-msvc
```

The Windows installers are written below
`desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/`.

## Uninstall

Open **Windows Settings → Apps → Installed apps**, find **Embeder**, and select
**Uninstall**. The editable workspace lives in application data so users can back it up
before removing local data.
