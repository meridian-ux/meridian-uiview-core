//! Guards that this crate is a drop-in for the `theme_proto`
//! `rust_prost_library`, and that the descriptor set it generates from is real.
//!
//! The failure this exists to catch is one-sided and silent. Bazel and cargo
//! produce these types from two different paths; if they diverge, whichever half
//! CI does not run keeps compiling, and the break surfaces on someone's laptop
//! or in the other build system entirely. So the assertions here are written
//! against what the *consumer* needs — `meridian-tui/rust/tui/src/theme.rs`,
//! which does `pub use theme_proto::meridian::theme::v1::*` and then names
//! `Theme`, `Palette`, and specific colour roles — rather than against whatever
//! this crate happens to emit.

use prost::Message as _;
use prost_types::FileDescriptorSet;

const FDS: &[u8] = include_bytes!("../proto/gen/meridian_theme_v1.binpb");

#[test]
fn the_module_path_the_consumer_writes_resolves() {
    // If this file compiles, `theme_proto::meridian::theme::v1::*` is a real
    // path exporting real types — which is the whole contract. Written as an
    // explicit `use` rather than a glob so a rename breaks HERE, naming the
    // symbol, instead of at a downstream call site.
    use theme_proto::meridian::theme::v1::{FontSource, Metrics, Palette, Theme, Typography};

    let t = Theme {
        light: Some(Palette {
            bg: "#FFFFFF".into(),
            accent: "#F2C46A".into(),
            ..Default::default()
        }),
        dark: None,
        ..Default::default()
    };
    assert_eq!(t.light.as_ref().unwrap().accent, "#F2C46A");
    // `dark` is optional and the TUI binding falls back to `light` when it is
    // unset; that fallback is only expressible because the field is a message.
    assert!(t.dark.is_none());

    // Typography / Metrics / FontSource carry no terminal mapping, so the TUI
    // does not re-export them — but they must still EXIST, because the web
    // binding reads them off the same Theme and a Theme missing them would be a
    // different message.
    let _ = Typography::default();
    let _ = Metrics::default();
    let _ = FontSource::default();
}

#[test]
fn every_palette_role_the_tui_binds_is_present() {
    // meridian-tui maps each of these to a ratatui Color. A role dropped
    // upstream would compile fine here and fail in the renderer, so name them.
    let p = theme_proto::meridian::theme::v1::Palette {
        bg: "#000000".into(),
        surface: "#111111".into(),
        fg: "#FFFFFF".into(),
        muted: "#888888".into(),
        border: "#222222".into(),
        accent: "#F2C46A".into(),
        accent_strong: "#E0A93F".into(),
        on_accent: "#000000".into(),
        danger: "#FF0000".into(),
        success: "#00FF00".into(),
        warning: "#FFAA00".into(),
        info: "#0088FF".into(),
        code_bg: "#0A0A0A".into(),
        code_fg: "#EEEEEE".into(),
    };
    // Round-trips through the wire, so this is a check on the generated
    // encode/decode too, not just on field names.
    let bytes = {
        let mut b = Vec::new();
        p.encode(&mut b).unwrap();
        b
    };
    let back = theme_proto::meridian::theme::v1::Palette::decode(bytes.as_slice()).unwrap();
    assert_eq!(back, p);
}

#[test]
fn the_descriptor_set_is_self_contained_and_is_theme() {
    let set = FileDescriptorSet::decode(FDS).expect("committed descriptor set does not decode");

    let names: Vec<&str> = set.file.iter().filter_map(|f| f.name.as_deref()).collect();
    assert!(
        names.contains(&"proto/theme.proto"),
        "proto/theme.proto missing — regenerate with tools/regen_descriptors.sh"
    );

    // theme.proto imports nothing today. Asserting that rather than assuming it
    // means an import added upstream fails here, where the message can say
    // "regenerate with --include_imports", instead of as an unresolved-type
    // error out of prost-build.
    let present: Vec<&str> = names.clone();
    for f in &set.file {
        for dep in &f.dependency {
            assert!(
                present.contains(&dep.as_str()),
                "{} imports {dep}, which is not in the set — regenerate with --include_imports",
                f.name.as_deref().unwrap_or("?")
            );
        }
    }

    let pkgs: Vec<&str> = set.file.iter().filter_map(|f| f.package.as_deref()).collect();
    assert!(
        pkgs.contains(&"meridian.theme.v1"),
        "the set does not declare package meridian.theme.v1 — wrong proto?"
    );
}
