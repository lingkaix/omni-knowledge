//! A process that never starts the account must fail before opening a source.

use std::path::Path;

use knowledge_processing::admission::AdmitError;
use knowledge_processing::extract;

#[test]
fn missing_account_is_not_started_before_open() {
    let error = extract::extract(Path::new("/no/such/omni-m3-source"), 128).unwrap_err();
    assert_eq!(
        error.downcast_ref::<AdmitError>(),
        Some(&AdmitError::NotStarted)
    );
}
