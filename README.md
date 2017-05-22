# datadog-sidekiq

A Rust app to track Sidekiq enqueued & processed jobs in DataDog.

## Metrics

Two metrics will be made available in DataDog:

- `sidekiq.enqueued`: the amount of enqueued jobs at one point in time
- `sidekiq.processed`: the amount of processed jobs between two consequent polling intervals

Consider using the `TAGS` environment variable to specify your data sources.

## Usage

Grab the latest binary from the [release section](https://github.com/blacklane/datadog-sidekiq/releases) or build it from source.

Configure the monitor via environment variables:

```bash
# [Required] The DataDog API key
DD_API_KEY=xxx

# [Required] The full Redis URL (including the port and database number)
REDIS_URL=redis://localhost:6379/0

# [Optional] The Redis namespace to use
REDIS_NAMESPACE=some:namespace

# [Optional] The polling interval in seconds (defaults to 60 seconds)
INTERVAL=60

# [Optional] A comma-separated list of tags
TAGS=application:xxx,environment:yyy,hello:world

# [Optional] The log level: either "error" or "info" (defaults to no logging)
RUST_LOG=error
```

Run the binary:

```bash
datadog-sidekiq
```

Or run it in the background:

```bash
datadog-sidekiq &
```

If that's not enough, consider using a supervisor (Systemd, runit, Monit, immortal etc.) so you can make sure that your monitor will be available even if the system is restarted.

## Build from source

Install [Rust](https://www.rust-lang.org) (latest stable version should be fine).

Clone the project, enter the project directory and run:

```bash
cargo build --release
```
