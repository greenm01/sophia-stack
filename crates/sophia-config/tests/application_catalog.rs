use sophia_config::*;
#[test]
fn catalogs_require_explicit_supported_policy_and_bounded_absolute_sources() {
    let valid = "schema 2\nsession { application-catalog \"installed\" launch-policy=\"trusted-host\" { source \"/usr/share/applications\"; application \"terminal\"; terminal \"terminal\" { arg \"--\"; }; }; }\n";
    let parsed = parse_core_config(valid.as_bytes(), ConfigGeneration::INITIAL).unwrap();
    assert_eq!(
        parsed.session.application_catalogs[0].terminal_arguments,
        vec!["--"]
    );
    for bad in [
        valid.replace(" launch-policy=\"trusted-host\"", ""),
        valid.replace("trusted-host", "isolated"),
        valid.replace("/usr/share/applications", "relative"),
        valid.replace("/usr/share/applications", "/usr/../tmp"),
    ] {
        assert!(parse_core_config(bad.as_bytes(), ConfigGeneration::INITIAL).is_err());
    }
}
