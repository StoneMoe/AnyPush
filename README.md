![](img/logo.png)

# AnyPush

A flexible message router that accepts messages from various sources, processing and routes them to different destinations based on configurable rules.

## Features

- **Multiple Inbound Handlers**: SMTP, HTTP, TCP
- **Multiple Outbound Handlers**: HTTP, TCP
- **Flexible Routing**: Expression-based conditions
- **Pipeline Processing**: Chain actions like log, transform, route
- **YAML Configuration**: With anchor/alias support for reusability

## Quick Start

```bash
cd anypush-rs
cargo run -- --config config.test.yaml
```

Test with curl:
```bash
curl -X POST http://127.0.0.1:8080 \
  -H "Content-Type: application/json" \
  -d '{"from":"test@example.com","subject":"Hello","text":"Test message"}'
```

Or send via SMTP:
```bash
telnet localhost 2525
HELO test
MAIL FROM:<sender@example.com>
RCPT TO:<receiver@example.com>
DATA
Subject: Test
Hello World
.
QUIT
```

## Configuration Example

```yaml
inbound:
  - type: http
    name: http-api
    port: 8080
  - type: smtp
    name: smtp-receiver
    port: 2525

outbound:
  - type: http
    name: webhook
    url: https://example.com/api

routes:
  - when:
      from: smtp-receiver
    goto:
      - action: log
      - action: route
        to: webhook
```

## Development Status

This project is under active development. The Rust rewrite (`anypush-rs/`) provides improved performance and flexibility.