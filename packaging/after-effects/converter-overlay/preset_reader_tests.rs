//! Original generated containers exercise format structure, never reference artwork/presets.
use aep_reader::effectcraft_preset;

fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = tag.to_vec();
    out.extend((body.len() as u32).to_be_bytes());
    out.extend(body);
    if body.len() % 2 != 0 {
        out.push(0);
    }
    out
}
fn form(tag: &[u8; 4], parts: &[Vec<u8>]) -> Vec<u8> {
    chunk(b"LIST", &[tag.as_slice(), &parts.concat()].concat())
}
fn path(name: &str) -> Vec<u8> {
    form(b"tdsp", &[form(b"tdsi", &[chunk(b"tdmn", name.as_bytes())])])
}
fn clock(rate: u32) -> Vec<u8> {
    let mut d = vec![0; 56];
    d[..4].copy_from_slice(&1_u32.to_be_bytes());
    d[4..8].copy_from_slice(&1_u32.to_be_bytes());
    d[12..16].copy_from_slice(&rate.to_be_bytes());
    chunk(b"beso", &d)
}
fn settings(rate: u32, value: f64, tick: Option<u32>) -> Vec<u8> {
    let mut desc = vec![0; 124];
    desc[3] = 1;
    desc[12..16].copy_from_slice(&rate.to_be_bytes());
    let payload = if let Some(tick) = tick {
        let mut header = vec![0; 52];
        header[8..12].copy_from_slice(&1_u32.to_be_bytes());
        header[16..20].copy_from_slice(&48_u32.to_be_bytes());
        let mut key = vec![0; 48];
        key[..4].copy_from_slice(&tick.to_be_bytes());
        key[4..6].copy_from_slice(&[1, 1]);
        key[8..16].copy_from_slice(&value.to_be_bytes());
        form(b"list", &[chunk(b"lhd3", &header), chunk(b"ldat", &key)])
    } else {
        chunk(b"cdat", &value.to_be_bytes())
    };
    form(b"tdbs", &[chunk(b"tdb4", &desc), payload])
}
fn preset(parts: &[Vec<u8>]) -> Vec<u8> {
    chunk(b"RIFX", &[b"FaFX".as_slice(), &form(b"besc", parts)].concat())
}

#[test]
fn batched_paths_and_interleaved_paths_bind_each_saved_payload_once() {
    for parts in [
        vec![path("first"), path("second"), path("ADBE End of path sentinel"), settings(12000, 7.0, None), settings(12000, 9.0, None)],
        vec![path("first"), settings(12000, 7.0, None), path("second"), settings(12000, 9.0, None)],
    ] {
        let (nodes, _, _, _, bindings) = effectcraft_preset(&preset(&parts), 8000).unwrap();
        assert_eq!(nodes.iter().map(|n| n.match_name.as_str()).collect::<Vec<_>>(), ["first", "second"]);
        assert_eq!(bindings.len(), 2);
        for (node, expected) in nodes.iter().zip([7.0, 9.0]) {
            assert_eq!(node.property.as_ref().unwrap().static_value.as_ref().unwrap().numbers().unwrap()[0].value, Some(expected));
        }
    }
}

#[test]
fn property_clock_controls_keys_and_container_clock_is_preserved_separately() {
    let bytes = preset(&[clock(30000), path("animated"), path("other"), settings(24000, 2.0, Some(6000)), settings(12000, 5.0, Some(6000))]);
    let (nodes, _, container, saved, _) = effectcraft_preset(&bytes, 8000).unwrap();
    assert!(saved);
    assert_eq!(container, 30000);
    for (node, rate, time) in [(&nodes[0], 24000, 0.25), (&nodes[1], 12000, 0.5)] {
        let p = node.property.as_ref().unwrap();
        assert_eq!(p.source_ticks_per_second, Some(rate));
        assert_eq!(p.keyframes[0].time_ticks, 6000);
        assert_eq!(p.keyframes[0].time_seconds, time);
    }
    assert!(effectcraft_preset(&preset(&[clock(30000), clock(24000), path("x"), settings(24000, 1.0, None)]), 8000).is_err());
    assert!(effectcraft_preset(&preset(&[path("x"), settings(u32::MAX, 1.0, None)]), 8000).is_err());
}

#[test]
fn unpaired_paths_and_trailing_bytes_are_reported_and_truncation_is_rejected() {
    let mut bytes = preset(&[path("saved"), path("pending"), settings(0, 1.0, None)]);
    bytes.extend(b"original trailing record");
    let (_, diagnostics, rate, saved, bindings) = effectcraft_preset(&bytes, 8000).unwrap();
    assert!(!saved);
    assert_eq!(rate, 8000);
    assert_eq!(bindings[0].path, ["saved"]);
    assert!(diagnostics.iter().any(|d| d.code == "preset-unpaired-path"));
    assert!(diagnostics.iter().any(|d| d.code == "preset-trailing-data"));
    assert!(effectcraft_preset(&bytes[..20], 8000).is_err());
}
