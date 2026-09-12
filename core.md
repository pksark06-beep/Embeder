# Embeder — Open Source Embedded AI OS & Desktop Workspace
**Embeder** is an open-source, local-first AI workspace and copilot engineered specifically for embedded systems engineering. It replaces gated SaaS interfaces with a transparent, local-first Model Context Protocol (MCP) architecture—enabling engineers to move seamlessly from raw idea to firmware compilation, instruction-level hardware simulation, and automated PCB netlist generation.
## 1. Executive Summary & Core Value Proposition
 * **Transparent AI Guidance:** Eliminates proprietary prompt locks by exposing structured engineering reasoning pipelines, schema enforcement, and tool execution logs.
 * **Local & Bring-Your-Own-Key (BYOK):** Connects to proprietary cloud models (OpenAI, Anthropic, Gemini) or fully local LLM runtimes (Ollama, Llama.cpp) with zero vendor lock-in.
 * **Closed-Loop Hardware Verification:** Automatically validates generated code via native compilation toolchains and instruction-level emulators (Renode/QEMU) before flashing hardware.
 * **Idea-to-PCB Pipeline:** Translates system constraints directly into KiCad layout netlists and production-ready manufacturing exports.
## 2. Target Microcontroller Architecture
| Tier Level | Hardware Targets | Key Focus Areas |
|---|---|---|
| **Tier 1: Basic Prototyping** | Arduino (Uno, Nano, Mega) | Basic GPIO manipulation, sensor polling, introductory C/C++ logic. |
| **Tier 2: Wireless & IoT** | ESP32, ESP8266 | Dual-core operations, Wi-Fi/BLE stacks, FreeRTOS task scheduling. |
| **Tier 3: Microcontrollers & Edge** | RP2040 (Raspberry Pi Pico), Raspberry Pi SBCs | C/C++ SDK, PIO state machines, high-level peripheral coordination. |
| **Tier 4: Industrial Microcontrollers** | STM32 (ARM Cortex-M series) | Direct register mapping, DMA controllers, precise clock trees, ultra-low power modes. |
## 3. Desktop Application Architecture (Tauri + Rust + Web)
Embeder utilizes a lightweight native desktop wrapper paired with a multi-threaded daemon to ensure UI responsiveness during heavy compilation and simulation background tasks.
```
embeder-workspace/
├── project.json # Global project configuration & MCU targets[span_21](start_span)[span_21](end_span)
├── src/ # C/C++ firmware source files[span_22](start_span)[span_22](end_span)
├── datasheets/ # Local vector index of MCU pinouts & manuals[span_23](start_span)[span_23](end_span)
├── simulation/ # Renode scripts, virtual logic logs, wave traces[span_24](start_span)[span_24](end_span)
└── pcb/ # KiCad schematic files (.kicad_sch) & layouts (.kicad_pcb)[span_25](start_span)[span_25](end_span)

```
### Core UI Modules
 * **Reasoning & Chat Panel:** Multi-step prompt execution, interactive decision paths, and real-time step explanations.
 * **Unified Firmware Editor:** Code viewer with live stdout/stderr streams from background compiler runs.
 * **Visual Hardware Canvas:** Interactive schematic viewer mapping physical pin connections and signal buses.
 * **PCB Design Studio:** Real-time preview of programmatically routed board traces and netlist updates.
 * **BYOK & Endpoint Manager:** Secure local storage for API credentials and local endpoint URIs.
## 4. MCP Server Ecosystem & Tool Specs
Embeder coordinates four core local Model Context Protocol (MCP) servers:
```
                       +-----------------------------------+
                       | Embeder UI Shell (Tauri) |
                       +-----------------------------------+
                                         |
                                         v
                       +-----------------------------------+
                       | Local MCP Orchestrator |
                       +-----------------------------------+
                                         |
        +-------------------+------------+------------+-------------------+
        | | | |
        v v v v
+---------------+ +---------------+ +---------------+ +---------------+
| Datasheet | | Firmware | | Simulation | | PCB EDA |
| MCP Server | | MCP Server | | MCP Server | | MCP Server |
+---------------+ +---------------+ +---------------+ +---------------+
| • search_ | | • compile_ | | • run_renode_ | | • generate_ |
| pinouts() | | firmware() | | simulation()| | netlist() |
| • get_ | | • get_ | | • capture_ | | • route_ |
| register_ | | compiler_ | | waveform() | | traces_ |
| map() | | errors() | | • check_ | | kicad() |
| • lookup_ | | • flash_ | | timing() | | • export_ |
| component_ | | target() | | | | gerber() |
| bom() | | | | | | |
+---------------+ +---------------+ +---------------+ +---------------+

```
### 1. Datasheet & Component Server
 * **search_pinouts(mcu_target, peripheral)**: Returns available pin assignments, alternate functions, and multiplexing constraints.
 * **get_register_map(mcu_target, address_block)**: Queries memory-mapped register configurations and bitfields.
 * **lookup_component_bom(query)**: Validates electronic component availability and pin specifications.
### 2. Firmware Compilation Server
 * **compile_firmware(target, src_path)**: Calls native compilers (avr-gcc, esp-idf, arm-none-eabi-gcc) in a headless subprocess.
 * **get_compiler_errors()**: Parses compiler stderr into clean JSON structures to enable automated LLM self-healing loops.
 * **flash_target(port, binary_path)**: Uploads verified binaries directly to attached microcontrollers.
### 3. Simulation & Validation Server
 * **run_renode_simulation(script_path)**: Boots instruction-level CPU emulators to execute compiled binaries headlessly.
 * **capture_waveform(signal_pins)**: Collects virtual GPIO/UART output logs and memory snapshots.
 * **check_timing(clock_cycles)**: Validates timing constraints and hardware interrupt routines.
### 4. PCB Layout & EDA Server
 * **generate_netlist(schematic_spec)**: Programmatically maps pins and net connections.
 * **route_traces_kicad(netlist_path)**: Executes KiCad Python automation scripts (pcbnew) to lay out board traces, power planes, and components.
 * **check_drc(board_path)**: Runs automated Design Rule Checks (DRC) for trace clearances and copper pour integrity.
 * **export_gerber(board_path)**: Outputs manufacturing files (Gerbers, NC Drill, BOM CSV, Pick-and-Place HTML).
## 5. End-to-End System Workflow
```
[User Goal Input]
       │
       ▼
[Stage 1: System Architecture] ──► Selects MCU, defines BOM & pinouts[span_53](start_span)[span_53](end_span)
       │
       ▼
[Stage 2: Firmware Synthesis] ──► Generates C/C++ code & runs native compilation[span_54](start_span)[span_54](end_span)
       │ ▲
       │ Fail │ (Self-Healing Error Feedback Loop)[span_55](start_span)[span_55](end_span)
       ├── [Build Verification] ────┘
       │ │ Pass
       ▼ ▼
[Stage 3: Renode Simulation] ──► Validates register states & timing headlessly[span_56](start_span)[span_56](end_span)[span_57](start_span)[span_57](end_span)
       │
       ▼
[Stage 4: KiCad PCB Design] ──► Programmatically routes board & exports Gerbers[span_58](start_span)[span_58](end_span)[span_59](start_span)[span_59](end_span)

```
## 6. Immediate MVP Development Milestones
### Milestone 1: Core Shell & BYOK Integration
 * Scaffold Tauri + Rust desktop app shell with split-pane layout.
 * Implement configuration engine for BYOK API keys and local Ollama endpoint routing.
### Milestone 2: Datasheet & Compilation MCP Servers
 * Build FastMCP Python server wrapper for datasheet lookup vectors.
 * Implement avr-gcc and esp-idf local build runners with structured error parsing.
### Milestone 3: Emulation Engine
 * Embed Renode CLI execution runner within the Simulation MCP server.
 * Display virtual serial outputs and stack status directly in the desktop UI.
### Milestone 4: KiCad Automation & Gerber Export
 * Develop KiCad pcbnew Python scripts for programmatic trace routing and netlist conversion.
 * Implement automated DRC checks and export one-click Gerber zip archives.
Would you like to start by setting up the Tauri desktop repository structure or building the first Python FastMCP compilation server?
