                                        _
   ____ ___  ___  ____________  _______(_)_  _______      ____
  / __ `__ \/ _ \/ ___/ ___/ / / / ___/ / / / / ___/_____/ __ \
 / / / / / /  __/ /  / /__/ /_/ / /  / / /_/ (__  )_____/ /_/ /
/_/ /_/ /_/\___/_/   \___/\__,_/_/  /_/\__,_/____/     / .___/
                                                      /_/

`mercurius-p` is a configurable [Model Context Protocol](https://modelcontextprotocol.io/) (MCP) stdio server for exposing local commands and scripts as tools. Define tools in JSON, point an MCP client at the binary, and give agents a controlled way to run the commands you choose.

The project is written in Rust and is intended for local-first workflows: system inspection, project utilities, git helpers, desktop automation, build commands, and other command-line tasks that are useful to make available through MCP.

## Features

- Expose local commands as MCP tools from a JSON configuration file.
- Support typed tool parameters: `string`, `integer`, `boolean`, `path`, and `enum`.
- Substitute parameters into command arguments and working directories with `{placeholder}` syntax.
- Capture `stdout`, `stderr`, exit code, timeout state, and execution duration.
- Validate configuration before starting the server.
- Enforce absolute-path validation for `path` parameters and configured working directories.
- Publish the active tool configuration as an MCP resource at `mercurius-p://tools/config`.

## Status

`mercurius-p` currently implements an MCP stdio transport with:

- `initialize`
- `tools/list`
- `tools/call`
- `resources/list`
- `resources/read`

The repository includes an example `tools.json` with Linux-oriented system, storage, desktop, git, development, and Rust tools. Treat it as a starting point and edit it for your machine before connecting an agent.

## Installation

### From source

Requirements:

- Rust toolchain with Cargo

Build the binary:

```sh
cargo build --release
```

The compiled binary will be available at:

```sh
target/release/mercurius-p
```

You can also install it into your Cargo bin directory:

```sh
cargo install --path .
```

## Quick Start

Validate the included example configuration:

```sh
cargo run -- validate --config tools.json
```

List the configured tools:

```sh
cargo run -- list-tools --config tools.json
```

Start the MCP server over stdio:

```sh
cargo run -- mcp --config tools.json
```

When using an installed or release binary:

```sh
mercurius-p mcp --config /absolute/path/to/tools.json
```

## MCP Client Configuration

Use `mercurius-p` as a stdio MCP server. The exact configuration format depends on your client, but it will generally look like this:

```json
{
  "mcpServers": {
    "mercurius-p": {
      "command": "/absolute/path/to/mercurius-p",
      "args": [
        "mcp",
        "--config",
        "/absolute/path/to/tools.json"
      ]
    }
  }
}
```

If you are running from a checkout instead of an installed binary, point the client at `cargo`:

```json
{
  "mcpServers": {
    "mercurius-p": {
      "command": "cargo",
      "args": [
        "run",
        "--manifest-path",
        "/absolute/path/to/mercurius-p/Cargo.toml",
        "--",
        "mcp",
        "--config",
        "/absolute/path/to/tools.json"
      ]
    }
  }
}
```

For regular use, prefer a compiled binary so startup is fast and independent of the build environment.

## Tool Configuration

Tools are configured as a JSON array. Each item describes one MCP tool and the local process it runs.

```json
[
  {
    "name": "echo-message",
    "description": "Echo a message",
    "command": "echo",
    "arguments": ["{message}"],
    "parameters": [
      {
        "name": "message",
        "type": "string",
        "required": true
      }
    ],
    "timeout_ms": 5000,
    "read_only": true,
    "category": "examples"
  }
]
```

### Tool Fields

| Field | Required | Description |
| --- | --- | --- |
| `name` | Yes | MCP tool name. Must be unique within the configuration. |
| `description` | Yes | Human-readable description shown to MCP clients. |
| `command` | Yes | Executable to start, such as `git`, `rg`, `sh`, or an absolute script path. |
| `arguments` | Yes | Command arguments. Use `{parameter_name}` placeholders to insert validated parameters. |
| `parameters` | Yes | Input parameters accepted by the tool. Use an empty array for tools without input. |
| `timeout_ms` | No | Process timeout in milliseconds. Defaults to `5000`. |
| `read_only` | No | Metadata describing whether the tool is intended to be read-only. Defaults to `false`. |
| `category` | No | Optional metadata for grouping tools in your own configuration. |
| `working_directory` | No | Directory to run the command in. May include placeholders such as `{path}`. |

### Parameter Fields

| Field | Required | Description |
| --- | --- | --- |
| `name` | Yes | Parameter name used by MCP clients and placeholders. |
| `type` | Yes | One of `string`, `integer`, `boolean`, `path`, or `enum`. |
| `required` | No | Whether the caller must provide the parameter. Defaults to `false`. |
| `default` | No | Default value used when the caller omits the parameter. |
| `allowed_values` | For `enum` | Accepted string values for enum parameters. |

Example enum parameter:

```json
{
  "name": "format",
  "type": "enum",
  "required": false,
  "default": "short",
  "allowed_values": ["short", "full"]
}
```

### Working Directory Example

```json
{
  "name": "git-status",
  "description": "Show short git status for a repository",
  "command": "git",
  "arguments": ["status", "--short"],
  "parameters": [
    {
      "name": "path",
      "type": "path",
      "required": true
    }
  ],
  "working_directory": "{path}",
  "timeout_ms": 5000,
  "read_only": true,
  "category": "git"
}
```

## Security Notes

`mercurius-p` runs local processes with the same operating-system permissions as the user who starts the server. Only configure commands you are comfortable exposing to your MCP client.

Current safeguards include:

- Configuration validation before the server starts.
- Typed parameter validation before execution.
- Rejection of unknown input parameters.
- Rejection of unknown placeholders in command arguments and working directories.
- Timeout-based process termination.
- Absolute-path requirements for `path` parameters and `working_directory` values.
- Default blocked path roots for path validation: `/etc`, `/boot`, `/usr`, `/bin`, `/sbin`, and `/root`.

Important limitations:

- `read_only` is metadata in the current implementation; it does not prevent a command from modifying the system.
- Commands are executed directly as configured. If you configure shell commands such as `sh -c`, review them carefully.
- The included `tools.json` contains desktop and system utilities that may not exist on every platform.

## CLI Reference

```sh
mercurius-p --help
```

Commands:

```sh
mercurius-p mcp --config tools.json
mercurius-p validate --config tools.json
mercurius-p list-tools --config tools.json
```

If `--config` is omitted, `mercurius-p` uses `tools.json` in the current working directory.

## Development

Run the test suite:

```sh
cargo test
```

Check compilation:

```sh
cargo check
```

Run Clippy:

```sh
cargo clippy
```

Format the code:

```sh
cargo fmt
```

## Project Layout

```text
src/
  cli.rs             CLI definitions
  config/           Tool configuration loading and validation
  executor.rs       Process execution with stdout/stderr capture and timeouts
  mcp/              JSON-RPC and MCP stdio handling
  model/            Tool, parameter, and execution result types
  registry.rs       Tool lookup registry
  service.rs        Tool validation and execution service
tools.json          Example tool configuration
assets/banner.txt   CLI validation banner
```

## Contributing

Contributions are welcome. For changes that affect behavior, include or update tests where practical. Before opening a pull request, run:

```sh
cargo fmt
cargo test
cargo clippy
```

## License

`mercurius-p` is licensed under the MIT License. See [LICENSE](LICENSE) for details.
