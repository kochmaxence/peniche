# peniche-core

`peniche-core` is a central dependency of the Peniche CLI tool, which is designed for efficient management of Rust workspaces. This crate encapsulates core functionalities and shared logic that power the various components of the Peniche toolset.

## Overview

The `peniche-core` crate includes a range of dependencies that are crucial for its operation within larger contexts of workspace management, including but not limited to:
- Managing crate dependencies within a workspace.
- Providing utility functions for file and path manipulation.
- Handling cross-platform command execution asynchronously.
- Facilitating robust error handling and colorful logging to enhance developer experience.

## Dependencies

`peniche-core` makes extensive use of several Rust community libraries such as `serde` for serialization, `cargo` for integration with Cargo's internals, `tokio` for asynchronous programming, and many more. These dependencies ensure that `peniche-core` can handle a variety of tasks that are essential for modern Rust development environments.

## Development Status

Peniche, including the `peniche-core` crate, is currently in early development. Features and APIs are subject to change as we refine the tool and expand its capabilities. Feedback and contributions are welcome as we work towards a more stable release.

## Usage

This crate is primarily intended to be used internally by the Peniche CLI tools. It is not designed for standalone use.

For more information on using or contributing to Peniche, please refer to the main project repository.
