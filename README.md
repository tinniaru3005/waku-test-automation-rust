# Waku Test Automation Framework (Rust Implementation)

A comprehensive test automation framework for Waku nodes written in Rust, implementing the Test Automation Engineer coding challenge requirements.

## Overview

This framework provides automated testing capabilities for Waku nodes, including:
- **Test Suite 1**: Basic Node Operation - Single node testing with message publication and API verification
- **Test Suite 2**: Inter-Node Communication - Multi-node testing with peer discovery and message relay

## Features

- 🐳 **Docker Integration**: Automated container lifecycle management
- 🔗 **Network Management**: Custom Docker network creation and node connectivity
- 📡 **RESTful API Testing**: Comprehensive API endpoint validation
- 🔄 **Async Operations**: Full async/await support with Tokio
- 📝 **Structured Logging**: Detailed tracing and logging
- 🧪 **Comprehensive Tests**: Both unit and integration testing
- 🛡️ **Error Handling**: Robust error handling with context

## Prerequisites

- **Rust**: 1.70+ (with Cargo)
- **Docker**: Running Docker daemon
- **Docker Image**: `wakuorg/nwaku:v0.24.0` (automatically pulled)

## Dependencies

Key dependencies include:
- `tokio` - Async runtime
- `reqwest` - HTTP client for API calls
- `bollard` - Docker API client
- `serde/serde_json` - Serialization/deserialization
- `anyhow` - Error handling
- `tracing` - Logging and instrumentation
- `base64` - Message encoding/decoding

## Installation & Setup

1. **Clone the repository:**
```bash
git clone git@github.com:tinniaru3005/waku-test-automation-rust.git
cd waku-test-automation-rust
```

2. **Install Rust dependencies:**
```bash
cargo build
```

3. **Ensure Docker is running:**
```bash
docker --version
# Should show Docker version information
```

4. **Pull required Docker image (optional - auto-pulled during tests):**
```bash
docker pull wakuorg/nwaku:v0.24.0
```

## Running Tests

### Run All Tests
```bash
cargo test
```

### Run Specific Test Suites

**Test Suite 1 - Basic Node Operation:**
```bash
cargo test test_suite_1_basic_node_operation
```

**Test Suite 2 - Inter-Node Communication:**
```bash
cargo test test_suite_2_inter_node_communication
```

### Run with Detailed Output
```bash
cargo test -- --nocapture
```

### Run with Logging
```bash
RUST_LOG=info cargo test -- --nocapture
```

## Test Details

### Test Suite 1: Basic Node Operation

This test verifies:
- ✅ Docker container startup for Waku node
- ✅ Node accessibility via REST API (`/debug/v1/info`)
- ✅ ENR URI extraction and validation
- ✅ Topic subscription (`/relay/v1/auto/subscriptions`)
- ✅ Message publication (`/relay/v1/auto/messages`)
- ✅ Message retrieval and validation
- ✅ Proper cleanup of resources

### Test Suite 2: Inter-Node Communication

This test verifies:
- ✅ Multiple node deployment with different port configurations
- ✅ Docker network creation and node connectivity
- ✅ Bootstrap node configuration for peer discovery
- ✅ Automatic peer connection establishment
- ✅ Cross-node message relay functionality
- ✅ Message propagation validation
- ✅ Network and container cleanup

## Project Structure

```
waku-test-automation/
├── Cargo.toml              # Project dependencies and metadata
├── README.md              # This documentation
├── src/
│   ├── lib.rs             # Main framework implementation
│   ├── main.rs            # CLI runner (optional)
│── tests/
│   ├── integration_tests.rs  
```

## Framework Architecture

### Core Components

1. **WakuTestFramework**: Main orchestrator class
   - Docker client management
   - HTTP client for API calls
   - Network lifecycle management

2. **WakuNode**: Node representation
   - Container metadata
   - Port configurations
   - ENR URI storage

3. **WakuNodeConfig**: Configuration builder
   - Flexible node setup
   - Bootstrap node support
   - Custom port mapping

### Key Methods

- `start_waku_node()` - Deploy and configure Waku container
- `setup_network()` - Create Docker bridge network
- `subscribe_to_topic()` - Subscribe node to relay topics
- `publish_message()` - Send messages through relay
- `get_messages()` - Retrieve messages from node
- `wait_for_peer_connection()` - Wait for peer discovery
- `cleanup_*()` - Resource cleanup methods

## Screenshots

<img width="1172" height="884" alt="Screenshot 2025-08-29 at 3 49 16 PM" src="https://github.com/user-attachments/assets/3c6bcb18-b989-4bb6-bae7-eadc7219288a" />
