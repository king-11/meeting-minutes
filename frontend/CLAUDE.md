# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Development Commands

### Quick Start
```bash
# Development (macOS)
./clean_run.sh [info|debug|trace]  # Default: info

# Production build (macOS)
./clean_build.sh [info|debug|trace]

# Development (Windows)
clean_run_windows.bat

# Production build (Windows)
clean_build_windows.bat
```

### NPM Scripts
```bash
pnpm dev          # Start Next.js dev server on port 3118
pnpm build        # Build Next.js production bundle
pnpm tauri dev    # Run Tauri desktop app in development
pnpm tauri build  # Build Tauri desktop app for production
pnpm lint         # Run ESLint
```

### Whisper Server
```bash
cd whisper-server-package
./run-server.sh   # Starts on http://localhost:8178
```

## Architecture Overview

### Technology Stack
- **Desktop Framework**: Tauri 2.x (Rust backend + React frontend)
- **Frontend**: Next.js 14.2 with App Router, React 18, TypeScript 5.7
- **Styling**: Tailwind CSS + shadcn/ui components (Radix UI based)
- **Audio Processing**: Local Whisper.cpp server for transcription
- **Rich Text**: Remirror and TipTap editors

### Key Architectural Patterns

1. **Tauri Desktop Integration**
   - Frontend communicates with Rust backend via `invoke` API
   - Audio capture handled by Rust backend (`src-tauri/src/audio_capture.rs`)
   - File system operations through Tauri plugins

2. **Real-time Transcription Pipeline**
   - Audio capture → Rust backend → Whisper server (port 8178) → Frontend display
   - Buffered transcript processing with sequence ID ordering
   - Out-of-order segment handling with 10-second buffer window

3. **State Management**
   - Context providers: `SidebarProvider`, `AnalyticsProvider`
   - Refs for async operations to avoid stale closures
   - Local component state with hooks

4. **Component Structure**
   ```
   src/app/           # Next.js pages (App Router)
   src/components/    
     ├── ui/          # shadcn/ui base components
     ├── molecules/   # Form components
     └── [features]/  # Feature-specific components
   ```

## Critical Implementation Details

### Audio Processing
- Sample rate: 16000 Hz, 32-bit float PCM format
- Minimum chunk: 1000ms (16000 samples)
- Recommended chunk: 500ms for real-time streaming
- Two audio sources: microphone and system audio captured separately

### Transcript Ordering System
- Each transcript segment has a `sequence_id`
- Buffering strategy to handle out-of-order segments
- 10-second window for reordering before display
- Chunk drop warnings when segments are lost

### AI Integration
- Supports multiple providers: Ollama (local), Claude, Groq
- Dynamic model selection based on provider
- Custom system prompts for meeting summaries
- Provider configurations stored in Tauri store

### Desktop Permissions (Tauri)
- File system: Full read/write to app data and downloads
- CSP allows: localhost connections (ports 11434, 5167, 8178)
- Microphone and system audio capture permissions required

## Common Development Tasks

### Adding a New AI Provider
1. Update `src/types/ai.ts` with provider interface
2. Add provider config to Settings page (`src/app/settings/page.tsx`)
3. Implement API call in meeting page (`src/app/page.tsx`)

### Modifying Audio Capture
1. Rust backend: `src-tauri/src/audio_capture.rs`
2. Frontend handler: `src/app/page.tsx` (search for `listen` calls)
3. Whisper config: `whisper-server-package/run-server.sh`

### Working with Transcripts
- Transcript state: `src/app/page.tsx` - `transcriptSegments` state
- Buffering logic: Look for `bufferedSegments` and `processBufferedSegments`
- Display component: `src/components/TranscriptDisplay.tsx`

## Testing Approach
No formal test framework is configured. Manual testing through:
- Development mode: `./clean_run.sh debug` for verbose logging
- Rust logs: Set `RUST_LOG=debug` or `trace`
- Browser DevTools for frontend debugging

## Important Files
- `src/app/page.tsx` - Main meeting interface and core logic
- `src-tauri/src/audio_capture.rs` - Audio capture implementation
- `src-tauri/tauri.conf.json` - Desktop app configuration
- `API.md` - Whisper server API documentation