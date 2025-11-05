# RustCraft

> An experimental voxel sandbox game built with Rust and WGPU, featuring GPU-driven fluid simulation and climate-aware procedural terrain generation.

## Features

- **Climate-Driven Terrain**: Procedural world generation with 11 distinct biomes (plains, desert, forest, mountain, swamp, tundra, jungle, mesa, savanna, taiga, and meadow) featuring unique height scales, sky palettes, and vegetation patterns.
- **Advanced World Generation**: Rivers, multi-layered cave networks, and continental influence systems create diverse and realistic landscapes.
- **GPU-Accelerated Fluid Simulation**: Compute shader-driven water simulation running on background workers with real-time diffusion between chunk columns.
- **Optimized Rendering**: Chunked world storage (16×16×256) with on-demand meshing, instanced rendering via WGPU, and efficient texture atlas system.
- **Player Interaction**: Fully-featured movement system with gravity, collision detection, sprint mechanics with dynamic FOV, noclip mode, and a nine-slot hotbar inventory.
- **Rich UI System**: Overlay interface with hotbar selection, pause and inventory menus, plus optional debug overlays for performance monitoring.
- **Pure Rust**: Built entirely in Rust 2021 edition using `winit` for windowing/input and `cgmath` for camera mathematics.

## Getting Started

### Prerequisites

- **Rust Toolchain**: Version 1.74 or newer (install from [rustup.rs](https://rustup.rs/))
- **GPU Support**: Modern graphics drivers supporting Vulkan, Metal, DirectX 12, or other WebGPU-compatible APIs
- **Platform-Specific Requirements**:
  - **Linux**: Install `vulkan-sdk` or appropriate Mesa drivers for your hardware
  - **Windows**: Ensure DirectX 12 or Vulkan drivers are up to date
  - **macOS**: Metal support is built-in on modern systems

### Build & Run

```bash
# Clone the repository
git clone https://github.com/CesarPetrescu/rustcraft.git
cd rustcraft

# Run in release mode (recommended for optimal performance)
cargo run --release

# Run in debug mode (for development)
cargo run
```

**Note**: The first launch compiles shaders and builds initial chunk meshes, resulting in a longer startup time. Release builds are highly recommended for maintaining real-time chunk update performance.

## Controls

| Action | Input |
|--------|-------|
| Start/Grab mouse | Click inside the window |
| Pause/Release mouse | `Esc` |
| Move forward/back | `W` / `S` |
| Strafe left/right | `A` / `D` |
| Jump / Ascend (noclip) | `Space` |
| Sprint | `Left Shift` |
| Toggle noclip fly mode | `F` |
| Toggle debug overlay | `F3` |
| Open/Close inventory | `E` |
| Look around | Mouse movement |
| Break block | Left mouse button |
| Place block | Right mouse button |
| Select hotbar slot | Number keys `1`-`9` |
| Cycle hotbar | Mouse wheel |

**Tip**: When noclip mode is enabled, you can fly freely in any direction. Hold sprint to increase flight speed.

## Architecture Overview

### Core Systems

- **Terrain Generation** (`world.rs`, `chunk.rs`)
  - Multi-noise climate lattice system for biome selection
  - Each biome has unique block palettes, sky colors, and height parameters
  - Chunks are 16×16×256 voxels with per-block fluid levels
  - Intelligent rebuild queues for efficient mesh updates

- **Rendering Pipeline** (`renderer.rs`, `mesh.rs`, `texture.rs`)
  - Instanced rendering using vertex buffers for optimal GPU utilization
  - Texture atlas system for efficient material switching
  - Separate pipelines for world geometry and 2D UI overlay
  - WGSL shader-based rendering for cross-platform compatibility

- **Fluid Simulation** (`fluid_system.rs`, `fluid_gpu.rs`, `fluid_compute.wgsl`)
  - Asynchronous compute shader execution on worker threads
  - GPU-accelerated water diffusion between chunk columns
  - Real-time fluid level updates integrated back into world state

- **Player Interaction** (`main.rs`, `camera.rs`, `inventory.rs`)
  - Physics-based camera movement with collision detection
  - Sprint mechanics with smooth FOV transitions
  - Raycast-based block interaction system
  - Hotbar and inventory management UI

## Project Structure

```
rustcraft/
├── src/
│   ├── main.rs              # Application entry point and event loop
│   ├── world.rs             # World generation and biome systems
│   ├── chunk.rs             # Chunk storage and management
│   ├── block.rs             # Block registry and metadata
│   ├── renderer.rs          # WGPU rendering backend
│   ├── mesh.rs              # Chunk meshing algorithms
│   ├── texture.rs           # Texture atlas management
│   ├── camera.rs            # Camera projection and controls
│   ├── fluid_system.rs      # Fluid simulation coordinator
│   ├── fluid_gpu.rs         # GPU compute shader bindings
│   ├── fluid_compute.wgsl   # Water diffusion compute shader
│   ├── shader.wgsl          # Main vertex/fragment shaders
│   ├── sky.wgsl             # Sky rendering shader
│   ├── ui_shader.wgsl       # UI overlay shader
│   ├── inventory.rs         # Inventory and hotbar systems
│   ├── raycast.rs           # Block selection raycasting
│   └── profiler.rs          # Performance profiling tools
├── docs/
│   └── electrical.md        # Documentation for electrical systems
├── Cargo.toml               # Project dependencies
├── STATE.md                 # Development roadmap and status
└── README.md                # This file
```

## Development Roadmap

This project is under active development. Current priorities include:

### Planned Features

- **Enhanced Block System**: Extended metadata for light emission, hardness variations, and texture variants
- **Biome Enrichment**: Unique structures, vegetation props, and atmospheric effects per biome
- **Crafting System**: Tool durability, crafting recipes, and resource gathering mechanics
- **Entity Framework**: Mob AI, spawning systems, and combat mechanics
- **World Persistence**: Save/load functionality with multiple world slots
- **Performance Optimizations**: Multi-threaded chunk generation, LOD systems, and async streaming

### Known Issues

- Water equalization occasionally creates raised ridges at chunk boundaries
- Rivers/lakes may leave floating water sheets when intersecting caves
- Fluid diffusion needs additional smoothing passes for waterfalls

See [`STATE.md`](STATE.md) for detailed milestone tracking and technical planning.

## Troubleshooting

**Black screen on startup**
- Verify your GPU drivers support the latest WGPU backends
- Try running with `--release` flag for better performance
- Check that your system meets the GPU requirements

**Long initial load time**
- First launch compiles shaders and generates initial chunks
- Subsequent launches will be faster due to shader caching
- Consider using release mode for improved performance

**Performance issues**
- Enable release mode: `cargo run --release`
- Toggle debug overlay with `F3` to monitor performance metrics
- Reduce render distance if experiencing frame drops

**Build errors**
- Ensure Rust toolchain is version 1.74 or newer: `rustc --version`
- Update dependencies: `cargo update`
- Clean build artifacts: `cargo clean && cargo build --release`

## Contributing

Contributions are welcome! Please feel free to submit pull requests or open issues for bugs and feature requests.

### Development Setup

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Make your changes and test thoroughly
4. Commit your changes: `git commit -m 'Add amazing feature'`
5. Push to the branch: `git push origin feature/amazing-feature`
6. Open a pull request

## License

This project's licensing has not yet been declared. Please contact the repository owner for licensing information before using this code in your own projects.

## Acknowledgments

- Built with [WGPU](https://wgpu.rs/) for cross-platform graphics
- Uses [winit](https://github.com/rust-windowing/winit) for windowing and input
- Terrain generation powered by [noise-rs](https://github.com/Razaekel/noise-rs)

---

**Note**: This is an experimental project focused on learning and exploring GPU-accelerated voxel rendering and procedural generation techniques. Performance characteristics and features are subject to change as development continues.
