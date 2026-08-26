use super::super::output_proof::{
    validate_output_proof_rollback_after_apply, validate_prepared_output_proof_candidate,
};
use std::time::Duration;

#[test]
fn post_apply_output_rollback_proof_fails_closed() {
    let public = sophia_config::ExternalWmInterface::SophiaWmV1;
    let legacy = sophia_config::ExternalWmInterface::ApiV7;
    let bounded = Some(Duration::from_secs(1));
    for requirements in [
        (false, true, public, bounded, true, false),
        (true, false, public, bounded, true, false),
        (true, true, legacy, bounded, true, false),
        (true, true, public, None, true, false),
        (true, true, public, bounded, false, false),
        (true, true, public, bounded, true, true),
    ] {
        let (native, normal, interface, runtime, armed, other_proof) = requirements;
        assert!(
            validate_output_proof_rollback_after_apply(
                true,
                native,
                normal,
                interface,
                runtime,
                armed,
                other_proof,
            )
            .is_err()
        );
    }
    assert!(
        validate_output_proof_rollback_after_apply(true, true, true, public, bounded, true, false,)
            .is_ok()
    );
    assert!(
        validate_output_proof_rollback_after_apply(false, false, false, legacy, None, false, true,)
            .is_ok()
    );
    assert!(validate_prepared_output_proof_candidate(true, false).is_err());
    assert!(validate_prepared_output_proof_candidate(true, true).is_ok());
    assert!(validate_prepared_output_proof_candidate(false, false).is_ok());
}
