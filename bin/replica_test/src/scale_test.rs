use crate::{main, utils, Argv as MainArgv};
use clap::{App, ArgMatches};
use fred::{
  bytes_utils::Str,
  pool::RedisPool,
  prelude::*,
  types::{ReplicaConfig, Server},
};
use indicatif::ProgressBar;
use rand::{self, distributions::Alphanumeric, Rng};
use std::{
  sync::{
    atomic::{AtomicBool, AtomicUsize},
    Arc,
  },
  time::Instant,
};
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct Argv {
  pub count:       u64,
  pub concurrency: usize,
  pub quiet:       bool,
}

pub fn parse_argv(matches: &ArgMatches<'_>) -> Arc<Argv> {
  let count = matches
    .value_of("count")
    .map(|v| {
      v.parse::<u64>().unwrap_or_else(|_| {
        panic!("Invalid command count: {}.", v);
      })
    })
    .unwrap_or(100_000);
  let concurrency = matches
    .value_of("concurrency")
    .map(|v| {
      v.parse::<usize>().unwrap_or_else(|_| {
        panic!("Invalid concurrency: {}.", v);
      })
    })
    .unwrap_or(10);
  let quiet = matches.is_present("quiet");

  Arc::new(Argv {
    count,
    concurrency,
    quiet,
  })
}

pub fn random_string(len: usize) -> String {
  rand::thread_rng()
    .sample_iter(&Alphanumeric)
    .take(len)
    .map(char::from)
    .collect()
}

async fn use_replica(client: &RedisClient, key: &Str) -> Result<i64, RedisError> {
  client.replicas().get::<i64, _>(key).await
}

async fn use_primary(client: &RedisClient, key: &Str) -> Result<i64, RedisError> {
  client.incr::<i64, _>(key).await
}

fn spawn_client_task(
  bar: &Option<ProgressBar>,
  main_argv: &Arc<MainArgv>,
  argv: &Arc<Argv>,
  count: &Arc<AtomicUsize>,
  client: RedisClient,
) -> JoinHandle<Result<(), RedisError>> {
  let bar = bar.clone();
  let main_argv = main_argv.clone();
  let argv = argv.clone();
  let count = count.clone();

  tokio::spawn(async move {
    let key = Str::from(random_string(12));
    let mut value = 0;

    loop {
      let count = utils::incr_atomic(&count);
      if count as u64 >= argv.count {
        break;
      }

      let result = if value % 2 == 0 {
        use_primary(&client, &key).await?
      } else {
        use_replica(&client, &key).await?
      };
      debug!("result ({}) {}: {}", count, key, result);
      if let Some(bar) = bar.as_ref() {
        bar.inc(1);
      }
      value += 1;
    }

    Ok(())
  })
}

pub async fn run(main_argv: &Arc<MainArgv>, matches: &ArgMatches<'_>) -> Result<(), RedisError> {
  let argv = parse_argv(matches);
  let config = RedisConfig {
    server: ServerConfig::Clustered {
      hosts: vec![Server::new(&main_argv.host, main_argv.port)],
    },
    replica: ReplicaConfig {
      lazy_connections: !main_argv.no_lazy,
      ..Default::default()
    },
    username: main_argv.username.clone(),
    password: main_argv.password.clone(),
    ..Default::default()
  };
  let perf = PerformanceConfig {
    auto_pipeline: !main_argv.no_pipeline,
    max_command_attempts: 3,
    ..Default::default()
  };
  let policy = ReconnectPolicy::new_linear(0, 2_000, 50);
  let count = Arc::new(AtomicUsize::new(0));
  let bar = if argv.quiet {
    None
  } else {
    Some(ProgressBar::new(argv.count as u64))
  };

  let pool = RedisPool::new(config, Some(perf), Some(policy), main_argv.pool).expect("Failed to create pool");
  let _ = pool.connect();
  let _ = pool.wait_for_connect().await?;

  let mut tasks = Vec::with_capacity(argv.concurrency);
  let started = Instant::now();

  for _ in 0 .. argv.concurrency {
    let client = pool.next().clone();
    tasks.push(spawn_client_task(&bar, main_argv, &argv, &count, client));
  }
  if let Err(e) = futures::future::try_join_all(tasks).await {
    println!("Finished with error: {:?}", e);
    std::process::exit(1);
  }
  let duration = Instant::now().duration_since(started);
  let duration_sec = duration.as_secs() as f64
    + (duration.subsec_millis() as f64 / 1_000.0)
    + (duration.subsec_micros() as f64 / 1_000_000.0);
  if let Some(bar) = bar {
    bar.finish();
  }

  if argv.quiet {
    println!("{}", (argv.count as f64 / duration_sec) as u32);
  } else {
    println!(
      "Performed {} operations in: {:?}. Throughput: {} req/sec",
      argv.count,
      duration,
      (argv.count as f64 / duration_sec) as u32
    );
  }
  let _ = pool.flushall_cluster().await?;

  Ok(())
}
