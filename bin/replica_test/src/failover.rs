use crate::Argv as MainArgv;
use clap::ArgMatches;
use fred::{prelude::*, types::Server};
use std::{sync::Arc, time::Duration};
use tokio::{task::JoinHandle, time::sleep};

struct Argv {
  failover_interval: Duration,
  failover_duration: Duration,
  // TODO add options to do only primary nodes, replicas, both, etc
}

fn parse(matches: &ArgMatches<'_>) -> Arc<Argv> {
  let failover_interval = matches
    .value_of("failover-interval")
    .map(|v| {
      v.parse::<u64>().unwrap_or_else(|_| {
        panic!("Invalid failover interval: {}.", v);
      })
    })
    .unwrap_or(3_000);
  let failover_duration = matches
    .value_of("failover-duration")
    .map(|v| {
      v.parse::<u64>().unwrap_or_else(|_| {
        panic!("Invalid failover duration: {}.", v);
      })
    })
    .unwrap_or(5_000);

  Arc::new(Argv {
    failover_duration,
    failover_interval,
  })
}

async fn pause() -> Result<(), RedisError> {
  unimplemented!()
}

async fn unpause() -> Result<(), RedisError> {
  unimplemented!()
}

async fn read_primaries(client: &RedisClient) -> Result<Vec<Server>, RedisError> {
  let _ = client.sync_cluster().await?;
  client
    .cached_cluster_state()
    .map(|state| state.unique_primary_nodes())
    .ok_or(RedisError::new(
      RedisErrorKind::Cluster,
      "Failed to read cluster state.",
    ))
}

fn spawn_unpause_task(duration: &Duration) -> JoinHandle<Result<(), RedisError>> {
  tokio::spawn(async move {
    sleep(duration).await;

    Ok(())
  })
}

async fn pause_random_primary() -> Result<(), RedisError> {
  unimplemented!()
}

pub async fn run(main_argv: &Arc<MainArgv>, matches: &ArgMatches<'_>) -> Result<(), RedisError> {
  let argv = parse(matches);
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

  let client = RedisClient::new(config, Some(perf), Some(policy));
  let _ = pool.connect();
  let _ = pool.wait_for_connect().await?;

  let unpause_duration = Duration::from_millis(argv.failover_duration);
  loop {
    // read primary nodes, pick one randomly?, pause it, spawn task to unpause it
    pause_random_primary().await;
    spawn_unpause_task(&unpause_duration);

    sleep(Duration::from_millis(argv.failover_interval)).await;
  }

  Ok(())
}
