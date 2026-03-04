<a href="https://carrycode.ai">
  <p align="center" style="max-width: 50%;">
    <img src="https://cdn.carrycode.ai/imgs/carrycode-ai-banner-light.png">
  </p>
</a>

<p align="center">
  <strong>Your AI Coding Agent, Right in the Terminal.</strong>
</p>


<p align="center">
  <img src="https://visitor-badge.laobi.icu/badge?page_id=deepcarryai-carrycode-repo" />
  <span ><a href="https://carrycode.ai"><img alt="Static Badge" src="https://img.shields.io/badge/HOMEPAGE-CARRYCODE-blue?style=flat&logoColor=orange&color=pink&logo=operagx"></a></span>
  <span class="dot"></span>
  <span ><a href="http://cdn.carrycode.ai/extension/carrycode-vscode-0.6.11.vsix" target="_blank"><img alt="Static Badge" src="https://img.shields.io/badge/Extension-VSCode-8A2BE2?style=flat&logo=applearcade"></a></span>
  <span class="dot"></span>
  <span ><a href="https://github.com/deepcarryai/carrycode" target="_blank"><img alt="Static Badge" src="https://img.shields.io/badge/GITHUB-v0.6.7-green?style=flat&logo=github"></a></span>
  <span class="dot"></span>
  <span ><a href="./README_CN.md"><img src="https://img.shields.io/badge/README-CN-red?style=flat&logo=readme" /></a><span >
</p>

---

**CarryCode** is a terminal-native AI coding agent that helps you write, refactor, debug, and understand code — all through natural conversation. It connects to 17+ LLM providers(OpenAI, Anthropic Claude, Google Gemini, OpenRouter, Ollma, vllm, GLM, KIMI, MiniMax, DeepSeek, Alibaba Claude etc), supports the **MCP** protocol for extensibility, supports the **SKILL** which is compatiable with claude code, supports **AGENTS.md** as project RULES, and delivers a beautiful terminal UI with themes, syntax highlighting, and code diff previews.

<details open>
  <summary><strong>Ocean of Stars</strong></summary>

  <br/>

  <img src="https://carrycode.ai/imgs/carrycode-ai-ocean-of-stars.png" alt="Ocean of Stars" width="100%">

</details>

<details>
  <summary><strong>Morning Sunglow</strong></summary>

  <br/>

  <img src="https://carrycode.ai/imgs/carrycode-ai-morning-sunglow.png" alt="Morning Sunglow " width="100%">

</details>

## ✨ Highlights

- 🤖 **Dual Agent Modes** — **Build** mode for autonomous code generation and editing; **Plan** mode for read-only analysis and planning.
- 🔌 **17+ LLM Providers** — OpenAI, Claude, Gemini, DeepSeek, Kimi, GLM, MiniMax, Qwen, Grok, Ollama, vLLM, and any OpenAI-compatible endpoint.
- 🚀 **240+ LLM Models** - GPT-5.2, Claude-Opus-4.6, Claude-Sonnet-4.5, Gemini-3-Pro, Gemini-3-Flash, Kimi-2.5, MiniMax-M2, GLM-4.7, DeepSeek-V3.2, Qwen3 etc. The latest SOTA models in the field of programming.
- 🧩 **MCP Protocol** — Extend your agent with Model Context Protocol servers. Add, edit, and manage MCP servers via `/mcp`.
- 🎯 **Skills System** — Load predefined or custom skills to guide how CarryCode performs specific tasks. Manage via `/skill`.
- 📋 **AGENTS.md** — Drop an `AGENTS.md` file in your project root to give CarryCode project-specific instructions and conventions.
- 🎨 **Beautiful Terminal UI** — Rich TUI with gradient banners, Markdown rendering, syntax-highlighted code blocks, and inline diff previews.
- 🌗 **Themes** — Switch between light and dark themes for code highlighting and diff previews via `/theme`.
- 🌍 **Multi-Language** — English and Chinese interface. Switch anytime via `/language`.
- 💬 **Session Management** — Create, switch, and resume sessions. Context is preserved across conversations.
- 🗜️ **Smart Context Compaction** — Automatically compresses long conversations to stay within token limits while preserving key context.
- 🩺 **LSP Diagnostics** — Integrated Language Server Protocol support (e.g., rust-analyzer) for real-time error and warning detection.
- 🔒 **Approval Modes** — Control what CarryCode can do: `read-only`, `agent` (read + write + execute), or `agent-full` (unrestricted).
- 🔄 **One-Click Update** — Run `/update` to check and install the latest version in-place.

## 📦 Installation

### One-Line Install (Recommended)

```bash
# MacOS / Linux
curl -fsSL https://carrycode.ai/install.sh | sudo sh

# Windows Powershell
irm https://carrycode.ai/install.ps1 | iex
```

Supports **macOS** (ARM64 / x64) and **Linux** (x64 / ARM64 / musl). The script auto-detects your platform, downloads the correct binary, verifies the checksum, and installs to `/usr/local/bin`.

### VSCode

CarryCode has released the VSCode extention in VSCode Market, you can search keyword 'carrycode' in sidebar extension markdet.

[CarryCode VSCode](https://marketplace.visualstudio.com/items?itemName=carrycodeai.carrycode-vscode)

### Verify Installation

```bash
carry --help
```

## 🚀 Quick Start

### Interactive Mode

Launch the full terminal UI:

```bash
carry
```

On first launch, a **setup wizard** will guide you through:
1. Choose your language (English / 中文)
2. Pick a theme (Light / Dark)
3. Select an LLM provider and enter your API key

### Single-Shot Mode

Run a one-off prompt and exit — great for scripting and CI:

```bash
carry --once "Explain what this function does"
carry --once "Add error handling to server.js" --timeout-ms 60000
```

## 🔨 Building from Source

### Prerequisites

- **Rust** (latest stable) - [Install via rustup](https://rustup.rs/)
- **Node.js** (v18+)
- **Bun** - [Install via bun.sh](https://bun.sh)
- **Build essentials** (for your OS):
  - Linux: `build-essential`, `pkg-config`, `libssl-dev`
  - macOS: Xcode Command Line Tools
  - Windows: Visual Studio Build Tools

### Build Commands

```bash
# Install dependencies
bun install

# Full build (Rust + TypeScript)
bun run build

# Or build separately:
bun run build:rust   # Compile Rust to Node native module
bun run build:ts     # Compile TypeScript

# Development mode (watch TypeScript)
bun run dev
```

### Build Output

After building, the executable will be at:
- `./target/index.js` - Main CLI entry point
- `./target/*.node` - Native Rust module

Run with:
```bash
node target/index.js
# or
./target/index.js
```

### Cleaning Build Artifacts

```bash
bun run clean        # Clean all build artifacts
bun run clean:rust   # Clean only Rust build artifacts
```

## ⌨️ Slash Commands

Type `/` in the input area to access the command menu:

| Command | Description |
|---------|-------------|
| `/model` | Switch LLM model, add or edit providers |
| `/mcp` | Manage MCP servers (add / edit / connect) |
| `/skill` | Load skills to guide agent behavior |
| `/rule` | Select project rules / guides |
| `/theme` | Switch code highlight and diff themes |
| `/language` | Switch interface language |
| `/approval` | Set approval mode (read-only / agent / agent-full) |
| `/session` | Create new or switch between sessions |
| `/compact` | Compress current session context |
| `/update` | Check and install updates |
| `/exit` | Exit the application |

## 🤖 Supported Providers

CarryCode works with a wide range of LLM providers out of the box:

| Provider | Models (examples) | Protocol |
|----------|-------------------|----------|
| **OpenAI** | GPT-4o, GPT-5.2 | OpenAI |
| **Anthropic** | Claude Opus 4.5, Claude Sonnet | Anthropic |
| **Google** | Gemini 3 Pro | Gemini |
| **DeepSeek** | DeepSeek R1 | OpenAI Compatible |
| **Moonshot / Kimi** | Kimi K2 | OpenAI Compatible / Anthropic |
| **ZhipuAI** | GLM-4.7 | OpenAI Compatible |
| **MiniMax** | MiniMax M2.1 | Anthropic |
| **Alibaba Cloud** | Qwen3 Max | OpenAI Compatible |
| **xAI** | Grok 4 | OpenAI Compatible |
| **SiliconFlow** | DeepSeek V3.2 | OpenAI Compatible |
| **Ollama** | Any local model | Ollama |
| **vLLM** | Any local model | vLLM |
| **OpenAI Compatible** | Any endpoint | OpenAI Compatible |

> 💡 Use `/model add` to configure a new provider interactively, or `/model edit` to modify existing ones.

## ⚙️ Configuration

### API Keys

Set your API key as an environment variable:

```bash
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
export GEMINI_API_KEY="AIza..."
```

Or use the interactive setup wizard on first launch, or `/model add` at any time.

### Config Files

| File | Location | Purpose |
|------|----------|---------|
| User config | `~/.carry/carrycode.json` | Provider credentials and preferences |
| Runtime config | `~/.carry/carrycode-runtime.json` | Language, default model, theme |
| Project rules | `./AGENTS.md` | Project-specific instructions for CarryCode |

> Override config paths with `CARRYCODE_CONFIG_DIR` or `CARRYCODE_CONFIG_FILE` environment variables.


## 📄 License

See [LICENSE](./LICENSE) for details.

### Usage Terms

- **Personal Research and Learning**: The source code of this project can be used for personal research and learning purposes.
- **Modification and Commercial Use**: If you wish to modify or use this project for commercial purposes, you are **prohibited from modifying** the project's Logo, Banner, or any identifying marks.
- **Logo/Banner Modification Requests**: If you have a need to modify the Logo, Banner, or identifying marks, please contact us at **us@carrycode.ai**.

---

<p align="center">
  Built with ❤️ by the <a href="https://carrycode.ai">CarryCode</a> team.
</p>
