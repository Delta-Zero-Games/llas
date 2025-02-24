# LLAS Development Guide

## Build Commands
- Development: `npm run tauri dev` (runs both UI and Rust backend)
- UI Only: `cd ui && npm run dev`
- Build: `npm run tauri build`
- UI Typecheck: `cd ui && npm run check`

## Rust Development
- Rust code follows standard formatting conventions
- Error handling with descriptive error messages and proper propagation
- Thread safety with Arc/Mutex wrappers for shared state
- Event-driven architecture with Tauri events

## Frontend (Svelte)
- TypeScript with strict typing
- Component-based architecture
- Tailwind CSS for styling
- State management via Svelte stores

## Code Style
- Descriptive variable/function names
- Thorough error handling with user-friendly messages
- Clean, modular code with appropriate commenting
- Consistent formatting (4-space indentation in Rust)

## Architecture
- Tauri (Rust) backend for audio processing and networking
- Svelte frontend for UI
- UDP-based network with Opus audio codec
- Redis for persistent room state