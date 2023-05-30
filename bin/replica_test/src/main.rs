#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;

use clap::{App, ArgMatches};
use fred::{pool::RedisPool, prelude::*};
use std::sync::Arc;
use tokio::runtime::Builder;

mod failover;
mod scale_test;
mod utils;

#[derive(Debug)]
pub struct Argv {
  pub host:        String,
  pub port:        u16,
  pub no_pipeline: bool,
  pub pool:        usize,
  pub no_lazy:     bool,
  pub username:    Option<String>,
  pub password:    Option<String>,
}

fn parse_argv(matches: &ArgMatches) -> Arc<Argv> {
  let no_lazy = matches.is_present("no-lazy");

  let host = matches
    .value_of("host")
    .map(|v| v.to_owned())
    .unwrap_or("127.0.0.1".into());
  let username = matches.value_of("username").map(|v| v.to_owned());
  let password = matches.value_of("password").map(|v| v.to_owned());
  let port = matches
    .value_of("port")
    .map(|v| v.parse::<u16>().expect("Invalid port"))
    .unwrap_or(6379);
  let pool = matches
    .value_of("pool")
    .map(|v| v.parse::<usize>().expect("Invalid pool"))
    .unwrap_or(1);
  let no_pipeline = matches.subcommand_matches("no-pipeline").is_some();

  Arc::new(Argv {
    host,
    port,
    no_pipeline,
    pool,
    no_lazy,
    username,
    password,
  })
}

fn main() {
  pretty_env_logger::init();
  let cli_yml = load_yaml!("../cli.yml");
  let app = App::from_yaml(cli_yml);
  let matches = app.get_matches();
  let argv = parse_argv(&matches);
  debug!("Parsed argv: {:?}", argv);

  let runtime = Builder::new_multi_thread()
    .enable_all()
    .build()
    .expect("Failed to build tokio runtime.");

  runtime.block_on(async move {
    if let Some(matches) = matches.subcommand_matches("scale-test") {
      scale_test::run(&argv, &matches).await.expect("Failed scale test")
    } else if let Some(matches) = matches.subcommand_matches("failover") {
      unimplemented!()
    } else {
      panic!("Invalid subcommand.");
    }
  });
}
