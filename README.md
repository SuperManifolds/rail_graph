# RailGraph

A web-based railway timetabling and conflict detection application for planning and visualizing train schedules on complex railway networks.

## Features

### Time Graph Visualization
- **Time-Distance Graphs**: Visualize train movements over time with station positions on the vertical axis
- **Journey Lines**: Each train journey displayed as a line showing travel through the network
- **Interactive Canvas**: Zoom, pan, and navigate through the timetable
- **Block Occupancy Visualization**: Hover over train lines to see track block occupancy
- **Current Time Marker**: Visual indicator of the current time on the graph

### Infrastructure Editor
- **Station Management**: Add, edit, and position railway stations
- **Track Configuration**: Define tracks between stations with direction (single/bidirectional)
- **Platform Assignment**: Configure platforms at each station for train services
- **Visual Network Editor**: Drag-and-drop interface for building railway topology

### Line and Schedule Management
- **Automatic Scheduling**: Define frequency-based services (e.g., every 30 minutes)
- **Manual Departures**: Specify individual train departures with custom timings
- **Forward and Return Routes**: Separate route configuration for each direction
- **Line Properties**: Customize colors, thickness, and visual appearance

### Conflict Detection
- **Head-on Conflicts**: Detect trains meeting on the same track from opposite directions
- **Overtaking Detection**: Identify trains catching up on the same track
- **Block Violations**: Detect multiple trains in the same single-track section
- **Platform Violations**: Identify platform conflicts at stations (with 1-minute buffer)
- **Station Crossings**: Track successful passing maneuvers at stations
- **Interactive Conflict List**: Click conflicts to navigate to their location on the graph

### Data Persistence
- **IndexedDB Storage**: Automatic project saving in browser storage
- **Legend Preferences**: Display settings persist across sessions
- **Multiple Project Support**: (Coming soon) Save and load multiple projects
- **CSV Import**: Import timetable data from CSV files
- **Binary Backup**: Download and restore project backups

### Display Options
- **Toggle Indicators**: Show/hide conflicts, station crossings, and block occupancy
- **Day Selection**: (Coming soon) View schedules for specific days of the week
- **View Filters**: (Coming soon) Focus on network subsections or specific lines

## Prerequisites

- **Rust** 1.76.0 or later
- **Trunk** (for building and serving)
- **wasm32-unknown-unknown** target

## Installation

### Install Rust and Trunk

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install trunk
cargo install --locked trunk

# Add WebAssembly target
rustup target add wasm32-unknown-unknown
```

### Clone the Repository

```bash
git clone https://github.com/supermanifolds/rail_graph.git
cd rail_graph
```

## Usage

### Running Locally

```bash
# Development mode (with hot reload)
trunk serve

# Release mode (optimized)
trunk serve --release
```

The application will be available at `http://localhost:8080`

### CSV Import

Prepare a CSV file with the following format:

```csv
,Line1,Line2,Line3
Station1,0:00:00,0:15:00,
Station2,0:05:00,0:20:00,0:10:00
Station3,0:10:00,,0:15:00
```

- First column: Station names
- Header row: Line IDs
- Cells: Departure times (HH:MM:SS format)
- Empty cells: Train doesn't stop at that station

Import via the import button in the application sidebar.

## Docker Deployment

### Using Docker Compose

```bash
# Build and run
docker-compose up -d

# View logs
docker-compose logs -f

# Stop
docker-compose down
```

### Using Docker Directly

```bash
# Build image
docker build -t rail-graph .

# Run container
docker run -d -p 8080:8080 --name rail-graph rail-graph

# Access at http://localhost:8080
```

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name
```

### Running Benchmarks

```bash
# Run conflict detection benchmarks
cargo bench
```

### Linting

```bash
# Run clippy
cargo clippy

# Auto-fix where possible
cargo clippy --fix
```

## Code Quality

The project uses clippy with the following lints:
- Complexity warnings
- Performance warnings
- Style warnings
- Suspicious code warnings
- Unwrap usage warnings

## CI/CD

GitHub Actions workflows automatically:
- Run `cargo check` on all targets
- Run `cargo clippy` with warnings as errors
- Build and test the Docker container

## Browser Compatibility

Tested and supported on:
- Chrome/Edge (latest)
- Firefox (latest)
- Safari (latest)

Requires IndexedDB support for data persistence.

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

Please ensure:
- Code passes `cargo clippy` without warnings
- Tests pass with `cargo test`
- Follow the coding guidelines in `CLAUDE.md`

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [Leptos](https://leptos.dev/) - Reactive web framework for Rust
