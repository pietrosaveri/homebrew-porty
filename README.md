# Porty

```
██████╗  ██████╗ ██████╗ ████████╗██╗   ██╗
██╔══██╗██╔═══██╗██╔══██╗╚══██╔══╝╚██╗ ██╔╝
██████╔╝██║   ██║██████╔╝   ██║    ╚████╔╝
██╔═══╝ ██║   ██║██╔══██╗   ██║     ╚██╔╝
██║     ╚██████╔╝██║  ██║   ██║      ██║
╚═╝      ╚═════╝ ╚═╝  ╚═╝   ╚═╝      ╚═╝
```

**A fast, intelligent local port inspector for macOS**

Porty helps you quickly identify what's running on your machine's ports, with intelligent categorization of development servers, databases, containers, and system services.

## Features

- **Smart Categorization**: Automatically classifies ports as Dev Servers, Databases, Containers, System services, or Unknown
- **Process Detection**: Shows the exact process and PID using each port
- **Detailed Port Inspection**: Comprehensive information including command line, working directory, process tree, resource usage, network details, and environment variables
- **Flexible Filtering**: View all ports, only development servers, or specific ports
- **Port Management**: Check availability and safely kill processes using specific ports
- **Colored Output**: Optional color-coded categories for better readability
- **Performance Optimized**: Parallel execution for fast detailed port inspection

## Installation

### Homebrew

```bash
brew tap pietrosaveri/porty
brew install porty
```

### From Source

```bash
cargo install --path .
```

## Usage

### Basic Commands

#### Default View (Dev Servers + Unknown)

Display development servers and unclassified ports:

```bash
porty
```

#### View All Ports

Display all listening TCP ports:

```bash
porty all
```

#### Development Servers Only

Show only identified development servers:

```bash
porty dev
```

#### Production View (Dev + Containers)

Show development servers and containers (useful for production-like environments):

```bash
porty prod
```

#### Check a Specific Port

Get comprehensive details about what's running on a particular port:

```bash
porty port 3000
```

This command provides extensive information including:
- Full command line with arguments
- Working directory and executable path
- Process tree (parent and child processes)
- Resource usage (memory, CPU, threads, file descriptors)
- Network details (listening addresses, active connections, other ports)
- Environment variables
- Docker container information (when applicable)

#### Check Port Availability

Verify if a port is free or in use:

```bash
porty free 8080
```

If in use, displays the process and provides hints on how to free the port.

#### Kill Process on Port

Terminate the process using a specific port:

```bash
# Dry run (shows what would be killed)
porty kill 3000

# Force kill (actually terminates the process)
porty kill 3000 --force
```

**Note**: The kill command requires the `--force` flag to actually terminate processes. Without it, it performs a dry run showing what would be killed.

### Global Options

#### Verbose Mode

Include full executable paths in the output:

```bash
porty --verbose
porty -v all
porty port 3000 -v
```

#### Colored Output

Enable color-coded categories:

- **Green**: Development servers
- **Cyan**: Databases
- **Blue**: Containers
- **Yellow**: System services
- **Red**: Unknown processes

```bash
porty --colors
porty -c dev
```

Combine both options:

```bash
porty -v -c
porty all --verbose --colors
```

## Command Reference

### Commands

| Command | Description | Example |
|---------|-------------|---------|
| _(default)_ | Show dev servers and unknown ports | `porty` |
| `all` | Show all listening ports | `porty all` |
| `dev` | Show only development servers | `porty dev` |
| `prod` | Show dev servers and containers | `porty prod` |
| `port <PORT>` | Inspect a specific port | `porty port 3000` |
| `free <PORT>` | Check if a port is available | `porty free 8080` |
| `kill <PORT>` | Terminate process on port | `porty kill 3000 --force` |

### Global Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--verbose` | `-v` | Show executable paths |
| `--colors` | `-c` | Enable colored output |
| `--help` | `-h` | Display help information |
| `--version` | `-V` | Show version number |

### Kill Command Options

| Flag | Short | Description |
|------|-------|-------------|
| `--force` | `-f` | Actually kill the process (required) |

## Port Categories

Porty intelligently categorizes ports based on process names and common port numbers:

- **Dev Server**: Node, Vite, Next.js, Python, Ruby, Rails, Django, Flask, Phoenix, Webpack, npm, yarn, and common dev ports (3000, 5173, 8080, 8000, 4200, etc.)
- **Database**: PostgreSQL, MySQL, Redis, MongoDB, MariaDB, CouchDB
- **Container**: Docker, containerd, Colima, Podman
- **System**: macOS system services (launchd, mDNSResponder, CUPS, ControlCenter, AirPlay)
- **Unknown**: Unrecognized processes or ports

## Examples

### Find what's using port 3000

```bash
$ porty port 3000
```

Displays comprehensive process information:

```
╭─────────────────────────────────────────────────────────────────────╮
│ Port 3000 - Process Details                                        │
╰─────────────────────────────────────────────────────────────────────╯

PROCESS INFORMATION
  Name:       node
  PID:        1234
  Category:   Dev Server
  Command:    node --inspect dist/server.js --port 3000
  Directory:  /Users/you/projects/api-server
  Exec Path:  /Users/you/.nvm/versions/node/v20.0.0/bin/node
  User:       you (501)
  Uptime:     2:15:30 (started Thu Jan 23 14:23:15 2026)

PROCESS TREE
  Parents:    Terminal (1000) → zsh (1100) → npm (1200) → node (1234)
  Children:   None

RESOURCES
  Memory:     245.3 MB (RSS), 1.2 GB (Virtual)
  CPU:        2.3%
  Threads:    8
  File Descriptors: 23 open

NETWORK
  Binding:    0.0.0.0:3000 (IPv4) + [::]:3000 (IPv6)
  Protocol:   TCP (LISTEN)
  Connections: 3 active
  Other Ports: Also listening on 9229

ENVIRONMENT
  NODE_ENV=development
  PORT=3000
  DATABASE_URL=postgresql://localhost:5432/myapp_dev
```

Use `--colors` for color-coded sections and categories.

### Check if port 8080 is free

```bash
$ porty free 8080
Port 8080 is in use:
  python3 (PID 5678)
  Hint: kill 5678 or use 'porty kill 8080'
```

### Kill a process safely

```bash
$ porty kill 3000
Process(es) on port 3000:
  node (PID 1234)

Dry run mode. Use --force to actually kill the process(es).
Example: porty kill 3000 --force

$ porty kill 3000 --force
Process(es) on port 3000:
  node (PID 1234)

Killing process(es)...
Killing node (PID 1234)...
Process killed
```

### View all ports with details and colors

```bash
$ porty all --verbose --colors
╭──────┬───────────┬────────────┬──────┬─────────────────────────╮
│ PORT │ PROCESS   │ CATEGORY   │ PID  │ EXEC PATH               │
├──────┼───────────┼────────────┼──────┼─────────────────────────┤
│ 3000 │ node      │ Dev Server │ 1234 │ /usr/local/bin/node     │
│ 5432 │ postgres  │ Database   │ 5678 │ /usr/local/bin/postgres │
│ 6379 │ redis     │ Database   │ 9012 │ /usr/local/bin/redis    │
╰──────┴───────────┴────────────┴──────┴─────────────────────────╯
```

## Platform Support

Porty is designed for **macOS** and uses native system APIs (`lsof` and `libproc`) for accurate port and process detection.

## Requirements

- macOS
- `lsof` command (pre-installed on macOS)

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

---

Built with Rust 🦀
