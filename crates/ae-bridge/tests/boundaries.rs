use effectcraft_ae_bridge::{discovery::inspect_pe, warnings};

#[test]
fn malformed_headers_and_hostile_offsets_are_errors() {
    for len in 0..200 {
        assert!(inspect_pe(&vec![0xff; len]).is_err());
    }
    let mut bytes = vec![0; 96];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[60..64].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(inspect_pe(&bytes).is_err());
    bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
    bytes[64..68].copy_from_slice(b"PE\0\0");
    bytes[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
    bytes[84..86].copy_from_slice(&240u16.to_le_bytes());
    bytes[86..88].copy_from_slice(&0x2000u16.to_le_bytes());
    bytes[88..90].copy_from_slice(&0x20bu16.to_le_bytes());
    assert!(inspect_pe(&bytes).is_ok());
    bytes[68..70].copy_from_slice(&0x14cu16.to_le_bytes());
    assert!(inspect_pe(&bytes).is_err(), "32-bit image must not appear x64");
}

#[test]
fn reports_cannot_silently_lose_diagnostics() {
    assert!(warnings("{}").is_err());
    assert!(warnings(r#"{"report":{"warnings":[{"message":"lost context"}]}}"#).is_err());
    assert_eq!(warnings(r#"{"report":{"warnings":[{"context":"Main/Glow","message":"not mapped"}]}}"#).unwrap(), vec!["Main/Glow: not mapped"]);
}

#[cfg(unix)]
mod native {
    use effectcraft_ae_bridge::{
        ProjectConverter,
        discovery::{Compatibility, PluginConfig, scan},
        process::ExternalConverter,
    };
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!("effectcraft-bridge-test-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn worker(&self, text: &str) -> ExternalConverter {
            let path = self.0.join("worker with spaces");
            fs::write(&path, format!("#!/bin/sh\n{text}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            ExternalConverter { executable: path, timeout: Duration::from_secs(2) }
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn worker_uses_a_snapshot_and_cleans_up_success_failure_timeout() {
        let temp = Temp::new();
        let worker = temp.worker("printf '{}' > \"$2\"\nprintf '%s' '{\"report\":{\"warnings\":[]}}' > \"$2.import.json\"\nprintf changed > \"$1\"");
        let source = b"original source";
        let result = worker.convert(source).unwrap();
        assert_eq!(result.project_json, "{}");
        assert_eq!(source, b"original source");
        let worker = temp.worker("exit 7");
        assert!(worker.convert(source).unwrap_err().contains("failed"));
        let mut worker = temp.worker("exec sleep 30");
        worker.timeout = Duration::from_millis(40);
        let start = std::time::Instant::now();
        assert!(worker.convert(source).unwrap_err().contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn discovery_never_executes_candidates_or_follows_symlink_loops() {
        let temp = Temp::new();
        let marker = temp.0.join("executed");
        let fake = temp.0.join("malicious.AEX");
        fs::write(&fake, format!("#!/bin/sh\ntouch '{}'", marker.display())).unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
        symlink(&temp.0, temp.0.join("loop")).unwrap();
        let result = scan(&PluginConfig { folders: vec![temp.0.to_string_lossy().into()], ..Default::default() });
        assert_eq!(result.plugins.len(), 1);
        assert_eq!(result.plugins[0].compatibility, Compatibility::Unsupported);
        assert!(!result.plugins[0].loaded);
        assert!(!marker.exists());
    }
}
