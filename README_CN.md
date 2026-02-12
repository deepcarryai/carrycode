<a href="https://carrycode.ai">
  <p align="center" style="max-width: 50%;">
    <img src="https://cdn.carrycode.ai/imgs/carrycode-ai-banner-light.png">
  </p>
</a>

<p align="center">
  <strong>Your AI Coding Agent, Right in the Terminal.</strong>
</p>
<p align="center">
  <a href="./README.md">English</a> ·
  <a href="https://carrycode.ai">官网</a> ·
  <a href="https://marketplace.visualstudio.com/items?itemName=carrycodeai.carrycode-vscode">VSCode扩展</a>
</p>

---

**CarryCode** 是一款终端原生的 AI 编程智能体，通过自然对话帮你编写、重构、调试和理解代码。它接入了 17+ 个大模型服务商，支持 MCP 协议扩展，并提供精美的终端 UI，包含主题切换、语法高亮和代码 Diff 预览。

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

## ✨ 核心亮点

- 🤖 **双模式 Agent** — **Build 模式**自主生成和编辑代码；**Plan 模式**只读分析和规划方案。
- 🔌 **17+ 模型服务商** — OpenAI、Claude、Gemini、DeepSeek、Kimi、GLM、MiniMax、通义千问、Grok、Ollama、vLLM，以及任何 OpenAI 兼容接口。
- 🚀 **240+ LLM Models** - GPT-5.2, Claude-Opus-4.6, Claude-Sonnet-4.5, Gemini-3-Pro, Gemini-3-Flash, Kimi-2.5, MiniMax-M2, GLM-4.7, DeepSeek-V3.2, Qwen3 等等在编程领域的最新SOTA模型.
- 🧩 **MCP 协议** — 通过 Model Context Protocol 服务器扩展智能体能力。使用 `/mcp` 添加、编辑和管理 MCP 服务器。
- 🎯 **Skills 技能系统** — 加载预定义或自定义技能，引导 CarryCode 更好地完成特定任务。使用 `/skill` 管理。
- 📋 **AGENTS.md** — 在项目根目录放置 `AGENTS.md` 文件，为 CarryCode 提供项目专属的指令和规范。
- 🎨 **精美终端 UI** — 丰富的 TUI 界面，支持渐变 Banner、Markdown 渲染、语法高亮代码块和内联 Diff 预览。
- 🌗 **主题切换** — 通过 `/theme` 在亮色和暗色主题之间切换代码高亮和 Diff 预览样式。
- 🌍 **多语言** — 支持 English 和简体中文界面，随时通过 `/language` 切换。
- 💬 **会话管理** — 创建、切换和恢复会话，上下文在对话间持久保存。
- 🗜️ **智能上下文压缩** — 自动压缩长对话以适应 Token 限制，同时保留关键上下文。
- 🩺 **LSP 代码诊断** — 集成语言服务器协议（如 rust-analyzer），实时检测代码错误和警告。
- 🔒 **审批模式** — 控制 CarryCode 的权限：`read-only`（只读）、`agent`（读写+执行）或 `agent-full`（无限制）。
- 🔄 **一键更新** — 运行 `/update` 即可检查并安装最新版本。

## 📦 安装

### 一键安装（推荐）

```bash
# MacOS / Linux
curl -fsSL https://carrycode.ai/install.sh | sudo sh

# Windows Powershell
irm https://carrycode.ai/install.ps1 | iex
```

支持 **macOS**（ARM64 / x64）和 **Linux**（x64 / ARM64 / musl）。脚本会自动检测平台、下载对应二进制文件、校验哈希并安装到 `/usr/local/bin`。

### VSCode插件

CarryCode官方插件已经上线VSCode扩展商店,可以在侧边栏扩展商店的搜索框中,搜关键字"carrycode"并安装扩展;

[CarryCode VSCode插件地址](https://marketplace.visualstudio.com/items?itemName=carrycodeai.carrycode-vscode)

### 验证安装

```bash
carry --help
```

## 🚀 快速开始

### 交互模式

启动完整的终端 UI：

```bash
carry
```

首次启动时，**设置向导**会引导你完成：
1. 选择语言（English / 中文）
2. 选择主题（亮色 / 暗色）
3. 选择大模型服务商并输入 API Key

### 单次模式

执行一次性提示词后退出 — 适合脚本和 CI 场景：

```bash
carry --once "解释这个函数的作用"
carry --once "给 server.js 添加错误处理" --timeout-ms 60000
```

## ⌨️ 斜杠命令

在输入区域输入 `/` 即可打开命令菜单：

| 命令 | 说明 |
|------|------|
| `/model` | 切换大模型、添加或编辑服务商 |
| `/mcp` | 管理 MCP 服务器（添加 / 编辑 / 连接） |
| `/skill` | 加载技能以引导智能体行为 |
| `/rule` | 选择项目规则 / 指南 |
| `/theme` | 切换代码高亮和 Diff 主题 |
| `/language` | 切换界面语言 |
| `/approval` | 设置审批模式（read-only / agent / agent-full） |
| `/session` | 新建或切换会话 |
| `/compact` | 压缩当前会话上下文 |
| `/update` | 检查并安装更新 |
| `/exit` | 退出应用 |

## 🤖 支持的模型服务商

CarryCode 开箱即用地支持以下大模型服务商：

| 服务商 | 模型示例 | 协议 |
|--------|----------|------|
| **OpenAI** | GPT-4o, GPT-5.2 | OpenAI |
| **Anthropic** | Claude Opus 4.5, Claude Sonnet | Anthropic |
| **Google** | Gemini 3 Pro | Gemini |
| **DeepSeek** | DeepSeek R1 | OpenAI Compatible |
| **Moonshot / Kimi** | Kimi K2 | OpenAI Compatible / Anthropic |
| **智谱 AI** | GLM-4.7 | OpenAI Compatible |
| **MiniMax** | MiniMax M2.1 | Anthropic |
| **阿里云** | 通义千问 Qwen3 Max | OpenAI Compatible |
| **xAI** | Grok 4 | OpenAI Compatible |
| **SiliconFlow** | DeepSeek V3.2 | OpenAI Compatible |
| **Ollama** | 任意本地模型 | Ollama |
| **vLLM** | 任意本地模型 | vLLM |
| **OpenAI Compatible** | 任意兼容接口 | OpenAI Compatible |

> 💡 使用 `/model add` 交互式配置新服务商，或 `/model edit` 修改已有配置。

## ⚙️ 配置

### API Key 设置

可以在首次启动时通过设置向导配置，或随时使用 `/model add` 添加。

### 配置文件

| 文件 | 位置 | 用途 |
|------|------|------|
| 用户配置 | `~/.carry/carrycode.json` | 服务商凭证和偏好设置 |
| 运行时配置 | `~/.carry/carrycode-runtime.json` | 语言、默认模型、主题 |
| 项目规则 | `./AGENTS.md` | 项目专属的 CarryCode 指令 |

> 可通过 `CARRYCODE_CONFIG_DIR` 或 `CARRYCODE_CONFIG_FILE` 环境变量覆盖配置路径。


## 📄 许可证

详见 [LICENSE](./LICENSE)。

---

<p align="center">
  由 <a href="https://carrycode.ai">CarryCode</a> 团队用 ❤️ 打造。
</p>
