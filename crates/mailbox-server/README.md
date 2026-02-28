# Mailbox Server

A simple HTTP server for storing and retrieving messages organized by topics. Built with Axum and redb.

## Features

- Store dollops organized by topic → log → sequence number
- Bidirectional sync: server returns missing dollops AND reports what it needs from the client
- Separate storage of large blobs associated with dollops (e.g. images, videos, files)
- Watermark tracking for efficient sync (tracks highest contiguous sequence per log)
- Persistent storage using redb (embedded database)
- Automatic cleanup of dollops and blobs older than 7 days (via UUID v7 timestamps)
- CORS enabled for cross-origin requests

## Installation

```bash
cargo build --release
```

## Usage

### Starting the Server

**Default configuration:**
```bash
cargo run --bin mailbox_server
```

This starts the server on `0.0.0.0:3000` with database file `mailbox.redb`.

**Custom configuration:**
```bash
cargo run --bin mailbox_server -- --db-path /path/to/database.redb --addr 127.0.0.1:8080
```

**Command-line options:**
- `-d, --db-path <DB_PATH>`: Path to the redb database file (default: `mailbox.redb`)
- `-a, --addr <ADDR>`: Address to bind the server to (default: `0.0.0.0:3000`)
- `-h, --help`: Print help information

### API Endpoints

#### Health Check
```bash
GET /health
```

Response:
```json
{
  "status": "ok"
}
```

#### Store Dollops
```bash
POST /dollops/store
Content-Type: application/json

{
  "dollops": {
    "topic-id-1": {
      "log-id-a": {
        "0": "SGVsbG8=",           // base64-encoded blob at sequence 0
        "1": "V29ybGQ="            // base64-encoded blob at sequence 1
      }
    }
  }
}
```

Dollops are organized by topic → log → sequence number. The server tracks watermarks (highest contiguous sequence from 0) for each topic:log pair.

Response: `201 Created`

#### Retrieve Dollops (Bidirectional Sync)
```bash
POST /dollops/get
Content-Type: application/json

{
  "topics": {
    "topic-id-1": {
      "log-id-a": 5,    // Client has sequences 0-5 for this log
      "log-id-b": 2     // Client has sequences 0-2 for this log
    }
  }
}
```

The request specifies, for each topic and log, the highest sequence number the client already has. The server responds with:

1. **dollops**: Dollops the client is missing (sequences > client's max)
2. **missing**: Sequences the server needs from the client (for bidirectional sync)

Response:
```json
{
  "dollops_by_topic": {
    "topic-id-1": {
      "dollops": {
        "log-id-a": {
          "6": "SGVsbG8sIFdvcmxkIQ==",
          "7": "QW5vdGhlciBtZXNzYWdl"
        }
      },
      "missing": {
        "log-id-b": [3, 4, 5]
      }
    }
  }
}
```

In this example:
- Server returns dollops for `log-id-a` at sequences 6 and 7 (client had up to 5)
- Server reports it's missing sequences 3, 4, 5 for `log-id-b` (client should send these)

### Example Usage with curl

**Store dollops:**
```bash
curl -X POST http://localhost:3000/dollops/store \
  -H "Content-Type: application/json" \
  -d '{
    "dollops": {
      "test-topic": {
        "log-1": {
          "0": "SGVsbG8=",
          "1": "V29ybGQ="
        }
      }
    }
  }'
```

**Retrieve dollops (with sync):**
```bash
curl -X POST http://localhost:3000/dollops/get \
  -H "Content-Type: application/json" \
  -d '{
    "topics": {
      "test-topic": {
        "log-1": 0
      }
    }
  }'
```

## Development

### Running Tests

**Integration tests:**
```bash
cargo test --test integration_test
```

**Stress tests:**
```bash
cargo test --test stress_test -- --nocapture
```

The stress tests include:
- Concurrent writes and reads
- Mixed read/write operations
- Large message handling
- Many topics scalability
- Health endpoint load testing

### Environment Variables

- `RUST_LOG`: Set logging level (e.g., `RUST_LOG=debug`)

Example:
```bash
RUST_LOG=mailbox_server=debug,tower_http=debug cargo run --bin mailbox_server
```

## Architecture

- **Storage**: redb (embedded key-value database)
- **Web framework**: Axum
- **Blob format**: Binary data encoded as base64 in JSON
- **Key format**: `{topic_id}:{log_id}:{sequence_number}:{uuid_v7}`
- **Watermarks**: Track highest contiguous sequence (0..=N) per topic:log for efficient sync
- **Concurrency**: Async/await with Tokio runtime, spawn_blocking for database operations

## License

See the root project license.
