#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "agora-signal-wrapper-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn sigterm_uses_the_same_stdin_shutdown_contract_and_preserves_child_receipt() {
    let root = TempRoot::new();
    let helper = root.0.join("owner-helper.sh");
    let ready = root.0.join("ready");
    fs::write(
        &helper,
        "#!/bin/sh\nset -eu\ntouch \"$1\"\ndd bs=1 count=1 >/dev/null 2>&1\nprintf '{\"schema\":1,\"cleanup\":\"clean\"}\\n'\n",
    )
    .unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
    let wrapper = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("run-with-signals.sh");
    let child = Command::new(wrapper)
        .arg(&ready)
        .env("AGORA_PRIVATE_LOCAL_DAEMON", &helper)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    for _ in 0..100 {
        if ready.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(ready.exists(), "signal wrapper child did not become ready");
    assert!(
        Command::new("kill")
            .arg("-TERM")
            .arg(child.id().to_string())
            .status()
            .unwrap()
            .success()
    );
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "{\"schema\":1,\"cleanup\":\"clean\"}\n"
    );
}
