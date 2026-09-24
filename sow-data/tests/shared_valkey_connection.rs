//! Regression test: `PlayerDb` must multiplex every operation over **one**
//! Valkey connection instead of opening a fresh socket per call.
//!
//! It needs a reachable Valkey/Redis. Point `SOW_REDIS_URL` at one to run it,
//! otherwise the test self-skips so plain `cargo test` stays green:
//!
//! ```text
//! valkey-server --port 6399 --save '' --appendonly no --daemonize yes
//! SOW_REDIS_URL=redis://127.0.0.1:6399 \
//!   cargo test -p sow-data --features server --test shared_valkey_connection
//! ```

#![cfg(feature = "server")]

use sow_data::PlayerDb;

/// Each `INFO` probe is itself a connection, so a reading taken after the
/// workload counts one probe connection on top of `PlayerDb`'s.
const PROBE_CONNECTIONS: u64 = 1;

async fn total_connections_received(url: &str) -> u64 {
    let client = redis::Client::open(url).expect("open probe client");
    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .expect("probe connection");
    let info: String = redis::cmd("INFO")
        .arg("stats")
        .query_async(&mut con)
        .await
        .expect("INFO stats");
    info.lines()
        .find_map(|line| line.strip_prefix("total_connections_received:"))
        .and_then(|value| value.trim().parse::<u64>().ok())
        .expect("total_connections_received present in INFO stats")
}

#[tokio::test]
async fn player_db_reuses_one_connection_for_many_operations() {
    let Ok(url) = std::env::var("SOW_REDIS_URL") else {
        eprintln!("SOW_REDIS_URL is not set; skipping the shared-connection regression check");
        return;
    };

    /// Comfortably more calls than the old code needed to show the problem:
    /// before the fix this opened one connection per call.
    const OPERATIONS: usize = 25;

    let before = total_connections_received(&url).await;

    // No redb mirror required: these calls only touch Valkey.
    let db = PlayerDb::new(&url, None, None);
    db.warm_connection()
        .await
        .expect("establish the shared connection");

    for index in 0..OPERATIONS {
        db.is_bot_account_checked(&format!("connection-regression-probe-{index:03}"))
            .await
            .expect("sismember over the shared connection");
    }

    let after = total_connections_received(&url).await;
    let opened_for_operations = after.saturating_sub(before).saturating_sub(PROBE_CONNECTIONS);

    assert_eq!(
        opened_for_operations, 1,
        "{OPERATIONS} operations should multiplex over 1 connection, but \
         {opened_for_operations} were opened: PlayerDb::get_connection is not reusing the \
         shared MultiplexedConnection"
    );
}
