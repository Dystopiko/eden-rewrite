use eden_config::{Config, LiveConfig};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::time::timeout;

#[tokio::test]
async fn should_alert_for_changes() {
    let handle = prepare_empty_cfg();

    let mut config = (*handle.get()).clone();
    let last_config = config.clone();
    config.background_jobs.workers = 4; // make this small and should update

    let mut recv = handle.subscribe();
    handle.update(config);

    timeout(Duration::from_millis(500), recv.changed())
        .await
        .expect("did not receive the new config in time")
        .unwrap();

    assert_ne!(**recv.borrow(), last_config);
}

#[tokio::test]
async fn should_not_notify_on_identical_update() {
    let handle = prepare_empty_cfg();
    let config = (*handle.get()).clone();

    let mut recv = handle.subscribe();

    // Update with identical config
    handle.update(config);

    // Should timeout because no change occurred
    let result = timeout(Duration::from_millis(100), recv.changed()).await;
    assert!(
        result.is_err(),
        "should not receive notification for identical config"
    );
}

#[tokio::test]
async fn should_notify_multiple_subscribers() {
    let handle = prepare_empty_cfg();

    let mut recv1 = handle.subscribe();
    let mut recv2 = handle.subscribe();
    let mut recv3 = handle.subscribe();

    let mut config = (*handle.get()).clone();
    config.background_jobs.workers = 8;
    handle.update(config);

    // All receivers should get the notification
    timeout(Duration::from_millis(500), recv1.changed())
        .await
        .expect("recv1 did not receive update")
        .unwrap();

    timeout(Duration::from_millis(500), recv2.changed())
        .await
        .expect("recv2 did not receive update")
        .unwrap();

    timeout(Duration::from_millis(500), recv3.changed())
        .await
        .expect("recv3 did not receive update")
        .unwrap();

    assert_eq!(recv1.borrow().background_jobs.workers, 8);
    assert_eq!(recv2.borrow().background_jobs.workers, 8);
    assert_eq!(recv3.borrow().background_jobs.workers, 8);
}

#[test]
fn clone_shares_state() {
    let handle1 = prepare_empty_cfg();
    let handle2 = handle1.clone();

    let mut config = (*handle1.get()).clone();
    config.background_jobs.workers = 20;
    handle1.update(config);

    // Cloned handle should see the same update
    assert_eq!(handle2.get().background_jobs.workers, 20);
}

#[test]
fn get_is_consistent() {
    let handle = prepare_empty_cfg();

    let config1 = handle.get();
    let config2 = handle.get();

    // Multiple gets should return Arc to same config
    assert!(Arc::ptr_eq(&config1, &config2));
}

fn prepare_empty_cfg() -> LiveConfig {
    let (config, _) = Config::maybe_toml_file("", Path::new("eden.toml"))
        .expect("should parse eden.toml template file");

    LiveConfig::new(config)
}
