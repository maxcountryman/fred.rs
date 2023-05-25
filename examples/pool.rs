use fred::{pool::RedisPool, prelude::*};
use futures::FutureExt;

#[tokio::main]
async fn main() -> Result<(), RedisError> {
  let config = RedisConfig::default();
  let pool = RedisPool::new(config, None, None, 3)?;
  let _ = pool.connect();
  let _ = pool.wait_for_connect().await?;

  for client in pool.clients() {
    println!("{} connected to {:?}", client.id(), client.active_connections().await?);
  }

  // use the pool like any other RedisClient to round-robin commands via `Deref`
  let _ = pool.get("foo").await?;
  let _ = pool.set("foo", "bar", None, None, false).await?;
  let _ = pool.get("foo").await?;

  // some functions, such as the on_* family of functions, must be called on each client individually
  for client in pool.clients() {
    let mut error_rx = client.on_error();
    tokio::spawn(async {
      while let Ok(error) = error_rx.recv().await {
        println!("Error: {}", error);
      }
    });
  }

  let _ = pool.quit_pool().await;
  Ok(())
}
