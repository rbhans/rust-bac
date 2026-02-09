use rustbac_core::apdu::ApduType;
use rustbac_core::encoding::reader::Reader;
use rustbac_core::npdu::Npdu;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should be resolvable")
}

fn parse_hex_fixture(path: &Path) -> Vec<u8> {
    let content = fs::read_to_string(path).expect("fixture must be readable");
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        for token in trimmed.split_whitespace() {
            let byte = u8::from_str_radix(token, 16)
                .unwrap_or_else(|_| panic!("invalid hex token '{token}' in {}", path.display()));
            out.push(byte);
        }
    }
    out
}

#[test]
fn golden_corpus_fixtures_decode_npdu_and_apdu_header() {
    let fixture_dir = workspace_root().join("fixtures/golden");
    let mut fixture_files = fs::read_dir(&fixture_dir)
        .expect("fixtures directory should exist")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "hex"))
        .collect::<Vec<_>>();
    fixture_files.sort();
    assert!(
        !fixture_files.is_empty(),
        "expected at least one corpus fixture in {}",
        fixture_dir.display()
    );

    for fixture in fixture_files {
        let bytes = parse_hex_fixture(&fixture);
        assert!(
            !bytes.is_empty(),
            "fixture {} must contain at least one byte",
            fixture.display()
        );

        let mut r = Reader::new(&bytes);
        let _npdu = Npdu::decode(&mut r).unwrap_or_else(|e| {
            panic!(
                "fixture {} failed NPDU decode with error {e:?}",
                fixture.display()
            )
        });

        if r.remaining() > 0 {
            let apdu = r
                .read_exact(r.remaining())
                .unwrap_or_else(|_| panic!("fixture {} APDU read failed", fixture.display()));
            let apdu_type_nibble = apdu[0] >> 4;
            assert!(
                ApduType::from_u8(apdu_type_nibble).is_some(),
                "fixture {} has unknown APDU type nibble 0x{:x}",
                fixture.display(),
                apdu_type_nibble
            );
        }
    }
}
