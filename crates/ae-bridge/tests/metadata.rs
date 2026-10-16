use effectcraft_ae_bridge::pe;

fn word(data: &mut [u8], at: usize, v: u32) {
    data[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// Original tiny PE container; no executable code or third-party fixture bytes.
fn image() -> Vec<u8> {
    let mut data = vec![0u8; 4096];
    data[..2].copy_from_slice(b"MZ");
    word(&mut data, 60, 128);
    data[128..132].copy_from_slice(b"PE\0\0");
    data[132..134].copy_from_slice(&0x8664u16.to_le_bytes());
    data[134..136].copy_from_slice(&1u16.to_le_bytes());
    data[148..150].copy_from_slice(&240u16.to_le_bytes());
    data[150..152].copy_from_slice(&0x2000u16.to_le_bytes());
    data[152..154].copy_from_slice(&0x20bu16.to_le_bytes());
    word(&mut data, 152 + 60, 512);
    word(&mut data, 152 + 108, 16);
    word(&mut data, 392 + 12, 0x1000);
    word(&mut data, 392 + 16, 3584);
    word(&mut data, 392 + 20, 512);
    data
}
fn pipl(little: bool, match_name: &str) -> Vec<u8> {
    let integer = |v: u32| if little { v.to_le_bytes() } else { v.to_be_bytes() };
    let mut data = vec![];
    data.extend(integer(0));
    data.extend(integer(4));
    for (key, payload) in [
        (b"kind", integer(u32::from_be_bytes(*b"eFKT")).to_vec()),
        (b"name", [vec![13], b"Original Test".to_vec()].concat()),
        (b"eMNA", [vec![match_name.len() as u8], match_name.as_bytes().to_vec()].concat()),
        (b"eVER", integer(1234).to_vec()),
    ] {
        data.extend(integer(u32::from_be_bytes(*b"8BIM")));
        data.extend(integer(u32::from_be_bytes(*key)));
        data.extend(integer(0));
        data.extend(integer(payload.len() as u32));
        data.extend(payload);
        while data.len() % 4 != 0 {
            data.push(0);
        }
    }
    data
}
fn with_resource(payload: &[u8]) -> Vec<u8> {
    let mut data = image();
    word(&mut data, 152 + 112 + 2 * 8, 0x1000);
    word(&mut data, 152 + 112 + 2 * 8 + 4, 1024);
    data[512 + 12..512 + 14].copy_from_slice(&1u16.to_le_bytes());
    word(&mut data, 528, 0x80000000 | 100);
    word(&mut data, 532, 0x80000000 | 24);
    data[512 + 24 + 14..512 + 24 + 16].copy_from_slice(&1u16.to_le_bytes());
    word(&mut data, 512 + 40, 16000);
    word(&mut data, 512 + 44, 0x80000000 | 48);
    data[512 + 48 + 14..512 + 48 + 16].copy_from_slice(&1u16.to_le_bytes());
    word(&mut data, 512 + 64, 1033);
    word(&mut data, 512 + 68, 72);
    word(&mut data, 512 + 72, 0x1000 + 128);
    word(&mut data, 512 + 76, payload.len() as u32);
    data[612..614].copy_from_slice(&4u16.to_le_bytes());
    for (i, c) in "PiPL".encode_utf16().enumerate() {
        data[614 + i * 2..616 + i * 2].copy_from_slice(&c.to_le_bytes());
    }
    data[640..640 + payload.len()].copy_from_slice(payload);
    data
}

#[test]
fn metadata_identity_requires_a_complete_resource_never_a_filename_or_stray_string() {
    let mut data = image();
    data[600..607].copy_from_slice(b"S_Shake");
    let metadata = pe::inspect(&data).unwrap();
    assert!(metadata.effects.is_empty());
    for little in [false, true] {
        let data = with_resource(&pipl(little, "org.effectcraft.test.metadata"));
        let metadata = pe::inspect(&data).unwrap();
        assert!(metadata.diagnostics.is_empty(), "{:?}", metadata.diagnostics);
        assert_eq!(metadata.effects.len(), 1);
        assert_eq!(metadata.effects[0].match_name, "org.effectcraft.test.metadata");
        assert_eq!(metadata.effects[0].name, "Original Test");
        assert_eq!(metadata.effects[0].kind, "eFKT");
        assert_eq!(metadata.effects[0].version_raw, Some(1234));
    }
}

#[test]
fn imports_exports_and_fixed_file_version_are_read_without_loading() {
    let mut data = image();
    word(&mut data, 152 + 112 + 8, 0x1000);
    word(&mut data, 152 + 112 + 12, 40);
    word(&mut data, 512 + 12, 0x1100);
    data[768..779].copy_from_slice(b"SYSTEM.dll\0");
    word(&mut data, 152 + 112, 0x1200);
    word(&mut data, 152 + 116, 40);
    word(&mut data, 1024 + 24, 1);
    word(&mut data, 1024 + 32, 0x1300);
    word(&mut data, 1280, 0x1400);
    let export = b"PluginDataEntryFunction2\0";
    data[1536..1536 + export.len()].copy_from_slice(export);
    let m = pe::inspect(&data).unwrap();
    assert_eq!(m.imports, vec!["SYSTEM.dll"]);
    assert_eq!(m.exports, vec!["PluginDataEntryFunction2"]);
    let mut value = vec![0u8; 92];
    let key: Vec<_> = "VS_VERSION_INFO\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
    value[6..6 + key.len()].copy_from_slice(&key);
    word(&mut value, 40, 0xfeef04bd);
    word(&mut value, 48, (2026 << 16) | 5);
    word(&mut value, 52, (1 << 16) | 42);
    let mut data = with_resource(&value);
    word(&mut data, 528, 16);
    assert_eq!(pe::inspect(&data).unwrap().file_version.as_deref(), Some("2026.5.1.42"));
}

#[test]
fn hostile_metadata_offsets_cycles_and_truncations_are_bounded() {
    let original = with_resource(&pipl(false, "org.effectcraft.test.metadata"));
    for len in 0..original.len() {
        let _ = pe::inspect(&original[..len]);
    }
    let mut data = original.clone();
    word(&mut data, 532, 0x80000000);
    assert!(!pe::inspect(&data).unwrap().diagnostics.is_empty());
    let mut data = original;
    word(&mut data, 152 + 112 + 2 * 8, 0xfffffff0);
    assert!(pe::inspect(&data).unwrap().effects.is_empty());
}

#[cfg(unix)]
#[test]
fn diagnostic_keeps_last_native_stage_when_worker_exits() {
    use std::{fs, os::unix::fs::PermissionsExt};
    let root = std::env::temp_dir().join(format!("effectcraft-diagnostic-test-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let worker = root.join("worker with spaces");
    let plugin = root.join("not-a-real-plugin.aex");
    fs::write(&plugin, image()).unwrap();
    fs::write(&worker,"#!/bin/sh\nprintf '%s' '{\"protocol\":1,\"loaded\":true,\"initialized\":false,\"stage\":\"globalSetup\",\"error\":\"unsupported suite\"}' > \"$2\"\nprintf 'missing suite evidence' >&2\nexit 7\n").unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    let result = effectcraft_ae_bridge::diagnostic::execute_test(&worker, &plugin, 0.0, &[]).unwrap();
    assert_eq!(result["exitCode"], 7);
    assert_eq!(result["checkpoint"]["stage"], "globalSetup");
    assert_eq!(result["checkpoint"]["loaded"], true);
    assert_eq!(result["frame"]["verified"], false);
    assert_eq!(result["presetApplied"], false);
    assert!(result["log"].as_str().unwrap().contains("missing suite evidence"));
    fs::remove_dir_all(root).unwrap();
}
