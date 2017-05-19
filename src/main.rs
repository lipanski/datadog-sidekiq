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
use redis::{Client as RedisClient, Commands, cmd, Connection};
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
        .map(|value| value.parse::<u64>().unwrap())
        .unwrap_or(DEFAULT_INTERVAL);

    let redis_url = env::var("REDIS_URL").unwrap();
    let redis_ns = Namespace::new(env::var("REDIS_NAMESPACE").ok());
    let redis = RedisClient::open(&*redis_url).unwrap();
    let conn = redis.get_connection().unwrap();

    let dd_api_key = env::var("DD_API_KEY").unwrap();

    let tags: Vec<String> = match env::var("TAGS") {
        Ok(raw_tags) => raw_tags.split(",").map(|value| value.to_string()).collect(),
        _ => vec![],
    };

    let mut previous_total_processed: Option<u32> = None;

    loop {
        let mut series = Series::new();

        let total_enqueued = get_total_enqueued(&conn, &redis_ns);
        series.push(Metric::gauge("sidekiq.enqueued", total_enqueued, tags.clone()));

        let total_processed = get_total_processed(&conn, &redis_ns);
        if previous_total_processed.is_some() && total_processed.is_some() {
            let current_processed = total_processed.unwrap() - previous_total_processed.unwrap();
            series.push(Metric::gauge("sidekiq.processed", current_processed, tags.clone()));
        }

        previous_total_processed = total_processed;

        let url = format!("{}?api_key={}", DD_SERIES_URL, dd_api_key);

        let mut easy = Easy::new();
        easy.post(true).unwrap();
        easy.url(&url).unwrap();

        let mut headers = HeaderList::new();
        headers.append("content-type: application/json").unwrap();
        easy.http_headers(headers).unwrap();

        let body = serde_json::to_string(&series).unwrap();
        info!("\nPOST {}\n{}", url, body);

        easy.post_field_size(body.len() as u64).unwrap();

        let mut response = vec![];
        {
            let mut transfer = easy.transfer();

            transfer.read_function(|into| {
                Ok(body.as_bytes().read(into).unwrap_or(0))
            }).unwrap();

            transfer.write_function(|data| {
                response.extend_from_slice(data);
                Ok(response.len())
            }).unwrap();

            transfer.perform().unwrap();
        }

        let status_code = easy.response_code().unwrap();
        if status_code != 202 {
            error!("{}", String::from_utf8_lossy(response.as_slice()));
        }

        sleep(Duration::new(interval, 0));
    }
}

fn get_total_enqueued(conn: &Connection, redis_ns: &Namespace) -> u32 {
    let queues: HashSet<String> = conn.smembers(redis_ns.wrap("queues")).unwrap();

    let mut pipe = redis::pipe();
    for queue in queues {
        let queue_name = ["queue", &queue].join(":");
        pipe.add_command(cmd("LLEN").arg(redis_ns.wrap(queue_name)));
    }

    let total_per_queue: Vec<u32> = pipe.query(conn).unwrap_or(Vec::new());

    total_per_queue.iter().sum()
}

fn get_total_processed(conn: &Connection, redis_ns: &Namespace) -> Option<u32> {
    conn.get(redis_ns.wrap("stat:processed")).ok()
}
