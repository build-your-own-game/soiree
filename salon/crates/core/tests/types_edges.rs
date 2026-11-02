//! The small types and the journal envelope, value by value, each found by a
//! mutant that survived the simulated network.

use salon_core::journal::{Diagnostic, PanicReport, fingerprint};
use salon_core::{DeviceId, Kind, Ms, Name, Token, TvKey};

#[test]
fn time_saturates_instead_of_wrapping() {
    assert_eq!(Ms(10).since(Ms(4)), 6);
    assert_eq!(Ms(4).since(Ms(10)), 0);
    assert_eq!(Ms(10).plus(5), Ms(15));
    assert_eq!(Ms(u64::MAX).plus(1), Ms(u64::MAX));
    assert!((Ms(1_500).as_f64() - 1_500.0).abs() < f64::EPSILON);
}

#[test]
fn secrets_stay_out_of_logs_and_identities_read_well() {
    assert_eq!(format!("{:?}", Token([7; 16])), "Token(…)");
    assert_eq!(format!("{:?}", TvKey([7; 16])), "TvKey(…)");
    assert_eq!(DeviceId(12).to_string(), "#12");
}

#[test]
fn only_what_connects_is_remote() {
    for (kind, remote) in [
        (Kind::Tv, true),
        (Kind::Phone, true),
        (Kind::App, true),
        (Kind::Gamepad, false),
        (Kind::Keyboard, false),
    ] {
        assert_eq!(kind.is_remote(), remote);
    }
}

#[test]
fn a_name_is_cleaned_cut_and_never_empty() {
    assert_eq!(Name::new("  Ada\n "), Name::new("Ada"));
    assert_eq!(Name::new(" \t\u{7f} "), None);
    assert_eq!(Name::new(""), None);
    let exact = "é".repeat(Name::MAX_CHARS);
    assert_eq!(Name::new(&exact).unwrap().as_str(), exact);
    let long = "é".repeat(Name::MAX_CHARS + 1);
    assert_eq!(Name::new(&long).unwrap().as_str(), exact);
    // Cut in front of a space: the space goes too.
    let spaced = format!("{} b", "a".repeat(Name::MAX_CHARS - 1));
    assert_eq!(
        Name::new(&spaced).unwrap().as_str(),
        "a".repeat(Name::MAX_CHARS - 1)
    );
    assert_eq!(Name::new("Ada").unwrap().to_string(), "Ada");
}

#[test]
fn a_diagnostic_says_its_page_and_its_message() {
    let panic = Diagnostic::Panic(PanicReport {
        message: "boom".to_owned(),
        page: "tv".to_owned(),
        at: Ms(1),
        attempt: 0,
    });
    assert_eq!((panic.page(), panic.message()), ("tv", "boom"));
    let marker = Diagnostic::Marker {
        at: Ms(1),
        page: "phone".to_owned(),
        note: "here".to_owned(),
    };
    assert_eq!((marker.page(), marker.message()), ("phone", "here"));
}

/// SHA-256 of the single byte 0, which is how postcard writes `0_u8`.
#[test]
fn a_fingerprint_is_the_sha_256_of_the_postcard_bytes() {
    assert_eq!(
        fingerprint(&0_u8).unwrap(),
        "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d"
    );
}
