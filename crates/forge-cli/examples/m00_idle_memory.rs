use std::path::PathBuf;
use std::time::Duration;

use forge_kernel::{NativeBootConfig, boot_native};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let data_directory = args
        .next()
        .map(PathBuf::from)
        .expect("pass a new state directory");
    let idle_seconds = args
        .next()
        .and_then(|value| value.to_string_lossy().parse::<u64>().ok())
        .unwrap_or(30);
    assert!(args.next().is_none(), "unexpected extra arguments");

    let boot = boot_native(NativeBootConfig::new(data_directory)).expect("native idle probe boot");
    println!(
        "FORGE_IDLE_READY pid={} schema={} boot_fingerprint={}",
        std::process::id(),
        boot.report.database_schema_version,
        boot.report.boot_fingerprint
    );
    std::thread::sleep(Duration::from_secs(idle_seconds));
    drop(boot);
}
