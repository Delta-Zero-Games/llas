# LLAS (Low Latency Audio System)

A high-performance, low-latency audio chat system built with Rust, Tauri, and Svelte.

## Features

- Ultra-low latency audio communication (~40ms end-to-end)
- Cross-platform support (Windows, macOS, Linux)
- Efficient UDP-based networking with custom protocol
- Opus codec for optimal audio quality
- Built-in room management system
- Simple and intuitive user interface

## Technology Stack

- **Backend**: Rust
  - Audio Processing: cpal
  - Network Transport: Custom UDP implementation
  - Audio Codec: Opus
- **Frontend**: Tauri + Svelte
- **Infrastructure**: TURN/STUN server (coturn)

## Development Setup

1. Install dependencies:
   ```bash
   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Install Node.js dependencies
   npm install
   ```

2. Run in development mode:
   ```bash
   npm run tauri dev
   ```

3. Build for production:
   ```bash
   npm run tauri build
   ```

## Architecture

The system is designed with a focus on minimal latency:

```
Capture → Encode → Network → Decode → Playback
[10ms]    [2ms]   [15ms]   [2ms]    [10ms]
```

Target latency: ~40ms end-to-end

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.