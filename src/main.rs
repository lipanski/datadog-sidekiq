extern crate redis;
extern crate curl;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate time;
#[macro_use] extern crate log;
extern crate env_logger;

use std::env;
use std::io::Read;
use std::fmt::Display;
use std::thread::sleep;
use std::time::Duration;
use std::collections::HashSet;
use std::error::Error;
use redis::{Client as RedisClient, Commands, cmd, Connection, RedisError};
use curl::easy::{Easy, List as HeaderList};

const DD_SERIES_URL: &str = "https://app.datadoghq.com/api/v1/series";
const DEFAULT_INTERVAL: u64 = 60;

#[derive(Serialize)]
struct Series {
    series: Vec<Metric>,
}

impl Series {
    fn new() -> Self {
        Series { series: Vec::new() }
    }

    fn push(&mut self, metric: Metric) {
        self.series.push(metric);
    }
}

#[derive(Serialize)]
struct Metric {
    metric: String,
    points: Vec<(i64, u32)>,
    #[serde(rename="type")]
    metric_type: String,
    tags: Vec<String>,
}

impl Metric {
    fn new<M: Into<String>, MT: Into<String>>(metric: M, value: u32, metric_type: MT, tags: Vec<String>) -> Self {
        let metric = metric.into();
        let points = vec![(time::get_time().sec, value)];
        let metric_type = metric_type.into();

        Metric { metric, points, metric_type, tags }
    }

    fn gauge<M: Into<String>>(metric: M, value: u32, tags: Vec<String>) -> Self {
        Self::new(metric, value, "gauge", tags)
    }
}

struct Namespace {
    default: Option<String>,
}

impl Namespace {
    fn new(default: Option<String>) -> Self {
        Namespace { default }
    }

    fn wrap<V: Into<String> + Display>(&self, value: V) -> String {
        match self.default {
            Some(ref default) => format!("{}:{}", default, value),
            None => value.into(),
        }
    }
}

fn main() {
    env_logger::init().unwrap();

    let interval = env::var("INTERVAL")
        .map(|value| value.parse::<u64>().expect("INTERVAL is not a valid number."))
        .unwrap_or(DEFAULT_INTERVAL);

    let redis_url = env::var("REDIS_URL").expect("REDIS_URL is missing.");
    let redis_ns = Namespace::new(env::var("REDIS_NAMESPACE").ok());
    let redis = RedisClient::open(&*redis_url).expect("Could not connect to the provided REDIS_URL.");
    let mut conn = establish_redis_connection(&redis).expect("Could not establish a connection to Redis.");
    let mut reconnect = false;

    let dd_api_key = env::var("DD_API_KEY").expect("DD_API_KEY is missing.");
    let url = format!("{}?api_key={}", DD_SERIES_URL, dd_api_key);

    let tags: Vec<String> = match env::var("TAGS") {
        Ok(raw_tags) => raw_tags.split(",").map(|value| value.to_string()).collect(),
        _ => vec![],
    };

    let mut previous_total_processed: Option<u32> = None;

    debug!("Welcome to datadog-sidekiq");
    loop {
        // Redis was down. Try to re-establish a connection.
        if reconnect {
            info!("Trying to connect to Redis again.");

            match establish_redis_connection(&redis) {
                Ok(new_connection) => {
                    conn = new_connection;
                    reconnect = false;
                },
                Err(redis_err) => {
                    error!("An error occured while trying to re-establish the Redis connection: {}", redis_err);
                    continue;
                }
            }
        }

        let mut series = Series::new();

        match get_total_enqueued(&conn, &redis_ns) {
            Ok(Some(total_enqueued)) => {
                series.push(Metric::gauge("sidekiq.enqueued", total_enqueued, tags.clone()));
            },
            Ok(None) => {},
            Err(redis_err) => {
                error!("A Redis error occured: {}", redis_err);
                reconnect = true;
                continue;
            }
        }

        match get_total_processed(&conn, &redis_ns) {
            Ok(total_processed) => {
                if previous_total_processed.is_some() && total_processed.is_some() {
                    let current_processed = total_processed.unwrap() - previous_total_processed.unwrap();
                    series.push(Metric::gauge("sidekiq.processed", current_processed, tags.clone()));
                }

                previous_total_processed = total_processed;
            },
            Err(redis_err) => {
                error!("A Redis error occured: {}", redis_err);
                reconnect = true;
                continue;
            },
        }

        match deliver_series(&url, series) {
            Ok(_) => {},
            Err(err) => { error!("An error occured while sending the series to DataDog: {}", err); }
        }

        sleep(Duration::new(interval, 0));
    }
}

fn establish_redis_connection(client: &RedisClient) -> Result<Connection, RedisError> {
    client.get_connection()
}

fn get_total_enqueued(conn: &Connection, redis_ns: &Namespace) -> Result<Option<u32>, RedisError> {
    let queues: HashSet<String> = conn.smembers(redis_ns.wrap("queues"))?;

    let mut pipe = redis::pipe();
    for queue in queues {
        let queue_name = ["queue", &queue].join(":");
        pipe.add_command(cmd("LLEN").arg(redis_ns.wrap(queue_name)));
    }

    let total_per_queue: Vec<u32> = pipe.query(conn)?;

    Ok(Some(total_per_queue.iter().sum()))
}

fn get_total_processed(conn: &Connection, redis_ns: &Namespace) -> Result<Option<u32>, RedisError> {
    conn.get(redis_ns.wrap("stat:processed"))
}

fn deliver_series(url: &str, series: Series) -> Result<(), String> {
    let mut request = Easy::new();
    request.post(true).map_err(|err| err.description().to_string())?;
    request.url(url).map_err(|err| err.description().to_string())?;

    let mut headers = HeaderList::new();
    headers.append("content-type: application/json").map_err(|err| err.description().to_string())?;
    request.http_headers(headers).map_err(|err| err.description().to_string())?;

    let body = serde_json::to_string(&series).map_err(|err| err.to_string())?;
    info!("\nPOST {}\n{}", url, body);

    request.post_field_size(body.len() as u64).map_err(|err| err.description().to_string())?;

    let mut response = vec![];
    {
        let mut transfer = request.transfer();

        transfer.read_function(|into| {
            Ok(body.as_bytes().read(into).unwrap_or(0))
        }).map_err(|err| err.description().to_string())?;

        transfer.write_function(|data| {
            response.extend_from_slice(data);
            Ok(data.len())
        }).map_err(|err| err.description().to_string())?;

        transfer.perform().map_err(|err| err.description().to_string())?;
    }

    let status_code = request.response_code().map_err(|err| err.description().to_string())?;
    if status_code == 202 {
        Ok(())
    } else {
        Err(format!("The DataDog server replied with an error: {}", String::from_utf8_lossy(&response)))
    }
}
