# Local Developer Dictation App: Product Specification

## 1. Product Overview

This document outlines the product strategy, technical architecture, and phased rollout for a privacy-first, macOS-exclusive dictation and assistant application tailored specifically for software developers. Unlike general-purpose dictation tools (e.g., Apple Dictation or standard SuperWhisper), this application runs entirely locally on Apple Silicon, ensuring zero telemetry and absolute data privacy. It leverages localized LLMs and Speech-to-Text (STT) models to understand developer vocabulary, syntax, and system context natively.

## 2. Core Features (The Backlog)

- **App-Aware Formatting**: The application detects the currently active window via macOS accessibility APIs. If the active application is a terminal (e.g., iTerm2, Kitty), it automatically formats dictation as CLI commands (e.g., "git commit update readme" becomes `git commit -m "update readme"`). In Slack, it defaults to conversational casing and punctuation.
- **The Local Config Agent**: Utilizing local file system access, the app can read and modify developer configurations natively. Using a specific wake word, the user can command the agent to update `~/.zshrc`, Neovim `init.lua`, or Claude desktop MCP settings securely without manual file navigation.
- **Pre-Loaded Developer Vocabulary**: The local Whisper model is dynamically seeded with a developer-centric prompt before transcription begins. This ensures high-accuracy recognition of domain-specific jargon like `kubectl`, `JSON`, `macOS`, `Hammerspoon`, `camelCase`, and framework names.
- **Syntax & Casing Commands**: Real-time voice macros allow developers to instruct the local LLM on string formatting. For example, dictating "Snake case error response handler" will output `error_response_handler` directly to the IDE.
- **Boilerplate Generation**: Intercepting raw transcripts, the LLM can generate functional code blocks on the fly. A command like "Dictate boilerplate React functional component named User Profile" will stream the completed React code directly to the active cursor.

## 3. Architecture & Solutioning

To optimize for rapid iteration, leveraging AI coding agents, and the developer's existing React/TypeScript expertise, the application will be built using the Tauri v2 framework.

| Component | Technology Choice | Rationale |
|---|---|---|
| Frontend / UI | React, TypeScript, Tailwind CSS | Leverages existing developer expertise for settings panes, agent chat UI, and floating widget design. |
| Backend Application Logic | Tauri v2 (Rust) | Provides low-level system access, lightweight binaries, and excellent AI-agent code generation support compared to Swift/CGO. |
| Audio Capture & STT | cpal (Rust), whisper-rs (C++ bindings) | Captures raw microphone streams locally and processes them through quantized Whisper GGUF models on Apple Silicon. |
| System Hooks (Paste/Focus) | rdev or enigo (Rust crates) | Simulates global keyboard shortcuts (e.g., Cmd+V) and listens for global push-to-talk hotkeys. |
| LLM Refinement Layer | Ollama API / Llama.cpp | Processes the raw Whisper transcript to correct developer jargon and apply casing/formatting rules before output. |

## 4. Day One (MVP) Build Plan

The Day One MVP focuses on a functional "Push-to-Talk" dictation loop that outputs corrected text to the active cursor.

### Phase 1: Project Scaffolding
- Initialize Tauri v2 with React and Vite.
- Configure `tauri.conf.json` to operate as a stealth macOS menu bar application (hidden dock icon, transparent floating window).
- Build a minimalist React UI with a recording toggle state and settings layout.

### Phase 2: Audio & System Hooks (Rust Backend)
- Implement microphone capture via `cpal` and save audio to a temporary `.wav` buffer.
- Register a global hotkey (e.g., Cmd+Shift+Space) using `rdev` to trigger the recording lifecycle.

### Phase 3: The STT Engine
- Integrate the `whisper-rs` crate to load a local Whisper base model.
- Create a Tauri IPC command to pass the `.wav` buffer to the model and retrieve the transcript string.
- Copy the transcribed string to the macOS clipboard and simulate Cmd+V to paste it at the active cursor position.

## 5. Future State: The "Rubber Duck" Agent & Continuous Voice

Moving beyond simple push-to-talk transcription, the long-term vision transforms the app into a full-duplex conversational developer assistant.

### 5.1 Continuous Voice Interaction (GPT-Live Paradigm)

Inspired by advanced continuous voice architectures (like OpenAI's third-generation GPT-Live system), the future state will shift from discrete turn-taking to a streaming, full-duplex model. Key architectural upgrades will include:

- **Removing the Turn Detector**: Instead of waiting for a button release or silence, the application will continuously stream audio to the local STT engine. GPT-Live allows the model to listen and speak at the same time, making conversation feel immediate.
- **Asynchronous Delegation**: Following the GPT-Live pattern, the core media loop will remain uninterrupted on a fast path, while complex reasoning (e.g., "Analyze my current React component for memory leaks") will be delegated asynchronously to a heavier local LLM (like a 32B parameter model) or frontier models. This ensures audio flows quickly without being blocked by deep thinking or tool use.
- **Stateful Context Management**: The application will maintain a rolling context window of the user's voice inputs and active IDE state. Similar to how GPT-Live handles long-running sessions, this context will be managed to allow seamless handoffs without interrupting the media flow.

### 5.2 Developer-Specific Modalities

- **Rubber Ducking Mode**: The local LLM will be system-prompted to utilize Socratic questioning. Instead of writing code for the user, the agent will analyze the active screen state and ask guiding questions to help the developer debug their logic autonomously.
- **Local TTS Integration**: To complete the conversational loop, the app will integrate local, low-latency Text-to-Speech engines (such as Kokoro or Piper) to provide immediate vocal feedback to the developer without relying on cloud APIs.

## 6. Backlog & Roadmap

See `ROADMAP.md` for what's next, in priority order, and `BACKLOG.md` for
the full detail behind every item (known issues, research findings,
effort estimates).
