# Desktop architecture and implementation status

Embeder uses Tauri 2: an HTML/CSS/JavaScript renderer in WebView2 communicates
with Rust through commands. HTML is the presentation layer of the native app;
replacing HTML alone cannot add IDE capabilities.

## Working execution path

```text
Editor buffers → Save all changed files
  → start_task(simulate) via Tauri IPC (or POST /api/tasks in development)
  → desktop/api task service on a background thread
  → snapshot saved C, headers, assembly, startup and linker inputs
  → firmware MCP worker → arm-none-eabi-gcc → firmware.elf + diagnostics
  → optional simulation MCP worker → Renode → UART assertion
  → task_status(id) → actual stage events, Problems, output, artifact, evidence
```

The desktop task service owns scheduling and rejects overlapping tasks. The
renderer polls its events; it cannot invent completed stages. Build creates an
ELF without simulation. Build & Simulate runs that ELF and checks for the expected
USART2 banner. Every task fingerprints the source contents and records compiler
and simulator results in a separate provenance file. These fingerprints detect
changes; they are not cryptographic signatures.

The worker compiles the submitted snapshot, including its startup code and linker
script. It never replaces editor contents with generated sample code or injects
an intentional error. Failures remain failures. Compiler diagnostics link back to
workspace files. This build task has no generation step.

## Model-assisted generation path (BYOK)

```text
Goal (natural language)
  → start_generate_task(goal) via Tauri IPC (or POST /api/generate in development)
  → grounding brief built from the CMSIS-SVD (exact register addresses + citations)
  → codegen MCP worker → BYOK model (Gemini / Vercel AI Gateway / OpenAI-compatible)
  → drafted main.c compiled against the workspace startup + linker (in memory)
  → on a compile error: diagnostics fed back to the model, re-draft (bounded self-heal)
  → simulation MCP worker → Renode → USART2 assertion
  → task_status(id) → same stage events, plus drafted_by (provider/model) and the draft
```

Generation is **non-destructive**: the model's `main.c` is compiled from an in-memory
snapshot and returned as `drafted_source`; the user's workspace files are never
overwritten. The renderer surfaces the draft as an unsaved buffer to review and Save.
The model only proposes — the compile and simulation oracles decide the tier, exactly
as for the build task. With no API key configured the task falls back to deterministic
grounded synthesis, so it always completes. Provenance records a `draft` entry naming
the provider/model alongside the `ground`, `compile`, `simulate` and `verify` records.

## Module boundaries

| Component | Responsibility |
| --- | --- |
| `dist/` | Workbench presentation, buffers, navigation, command palette, task events |
| `src-tauri/` | Native lifecycle and IPC; blocking probes use background threads |
| `api/src/tasks.rs` | Task lifecycle, source snapshots, MCP orchestration, evidence |
| `api/src/lib.rs` | Workspace access, datasheet queries, worker tool dispatch |
| `core/` | GCC and Renode adapters, diagnostic parsing, provenance |
| `mcp/` | Process transport and JSON-RPC client |
| `server/` | Development HTTP adapter and native-worker integration tests |

## Next IDE capabilities

Build these against explicit service interfaces instead of adding more visual
controls to the renderer:

1. **Workspace service:** open a project directory, load a versioned project
   manifest, watch files, detect external edits and resolve save conflicts. The
   current workspace and board profile are fixed to STM32F4 in this checkout.
2. **Editor and language service:** replace the textarea/highlight overlay with
   Monaco models, then connect clangd through LSP for completion, definition,
   references and semantic diagnostics. Monaco alone does not supply clangd.
3. **Task lifecycle:** cancellation and process-tree cleanup, bounded logs,
   streaming output, reusable build directories, dependency-aware rebuilds,
   project-selectable CMake/Make/framework adapters. Current task events describe
   real stage transitions; raw process output is returned when a stage finishes.
4. **Debug service:** DAP adapter backed by GDB/OpenOCD or a simulator, with
   breakpoints, stack frames and variables. Compiling an ELF is not debugging.
5. **Extension host:** manifest, lifecycle, capability permissions and process
   supervision. Existing MCP tools are three built-in tool workers, not a VS Code
   extension marketplace. VS Code extensions cannot be installed here today.
6. **Distribution:** bundled starter projects/resources, a writable user workspace,
   installer, signing and updates. The native development executable currently
   locates its workspace and SVD in this source checkout.

Tauri remains suitable for this architecture. If compatibility with existing VS
Code extensions becomes a product requirement, evaluate Code-OSS or Theia as a
separate platform decision; copying the workbench styling does not provide that
compatibility.

## Verification and limits

`cargo test -p embeder-desktop --test real_desktop -- --ignored` exercises the
actual desktop binary, HTTP task service, stdio MCP workers, GCC and Renode.
It checks a successful build and UART assertion, a compiler error, and an invalid
linker script. It never alters the user's workspace to introduce these failures.

The current simulation assertion verifies ELF boot and expected UART text. It
does not assert GPIO edges or real hardware behavior. MCP path checks are an
application-level boundary; child processes are not contained by an OS sandbox.
Cancellation, process/resource quotas and complete compiler file-access isolation
remain necessary before running untrusted third-party projects.

References: [Tauri IPC](https://v2.tauri.app/concept/inter-process-communication/),
[Tauri commands](https://v2.tauri.app/develop/calling-rust/),
[VS Code language server architecture](https://code.visualstudio.com/api/language-extensions/language-server-extension-guide),
[VS Code extension hosts](https://code.visualstudio.com/api/advanced-topics/extension-host).
