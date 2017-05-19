# datadog-sidekiq

A Rust app to track sidekiq enqueued & processed jobs in DataDog.

## Usage

Grab the latest binary from the release section or build it from source.

Configure the monitor via environment variables:

```bash
# [Required] The DataDog API key
DD_API_KEY=xxx

# [Required] The full Redis URL (including the port and database number)
REDIS_URL=redis://localhost:6379/0

# [Optional] The Redis namespace to use
REDIS_NAMESPACE=some:namespace

# [Optional] The polling interval in seconds (defaults to 60 seconds)
INTERVAL=30

# [Optional] A comma-separated list of tags
TAGS=application:xxx,environment:yyy,hello:world

# [Optional] The log level (allowed values: error|info)
RUST_LOG=error
```

Run the binary:

```bash
datadog-sidekiq
```

Alternatively you can also run it in the background:

```bash
datadog-sidekiq &
```

## Metrics

Two metrics will be made available in DataDog:

- `sidekiq.enqueued`: the amount of enqueued jobs at one point in time
- `sidekiq.processed`: the amount of processed jobs between two polling intervals

Consider using the `TAGS` environment variable to configure your data sources.

## Build form source

Install Rust (latest stable should be fine).

Clone the project and run:

```bash
cargo build --release
```
