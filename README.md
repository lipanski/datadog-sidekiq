# datadog-sidekiq

A Rust app to track Sidekiq enqueued & processed jobs in DataDog.

## Metrics

Two metrics will be made available in DataDog:

- `sidekiq.enqueued`: the amount of enqueued jobs at one point in time
- `sidekiq.processed`: the amount of processed jobs between two consequent polling intervals

Consider using the `TAGS` environment variable to specify your data sources.

## Usage

Grab the latest binary from the [release section](https://github.com/lipanski/datadog-sidekiq/releases) or build it from source.

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

## Docker

The app is also provided as a Docker image: https://hub.docker.com/r/lipanski/datadog-sidekiq

## Build from source

Install [Rust](https://www.rust-lang.org) (latest stable version should be fine).

Clone the project, enter the project directory and run:

```bash
cargo build --release
```

## Alternative

Another way to track Sidekiq metrics in DataDog is by using the [DataDog Redis integration](https://docs.datadoghq.com/integrations/redis/). There are some drawbacks though and I haven't tried it out myself.

Fetching the *amount of processed jobs* is as easy as querying `[namespace]:stat:processed`, but fetching the *amount of enqueued jobs* is a bit more complicated. First you need to query the names of all your queues - `SMEMBERS [namespace]:queues` - and for *each one* of these names you'll need to run `LLEN [namespace]:queue:[name]` and sum up the results. Ideally you should pipeline these calls, just like Sidekiq does and just like *datadog-sidekiq* does.

If you're interested in only one queue or just a couple, using the DataDog Redis integration might do the job. If you want a more dynamic solution, give a try to *datadog-sidekiq* :)
