#![allow(clippy::unwrap_used, clippy::expect_used)]

use modkit_security::{SECCTX_BIN_VERSION, SecurityContext, decode_bin, encode_bin};
use uuid::Uuid;

#[test]
#[allow(clippy::unreadable_literal)] // UUID hex patterns are intentionally repeating
fn round_trips_security_ctx_binary_payload() {
    let subject_id = Uuid::from_u128(0xdeadbeefdeadbeefdeadbeefdeadbeef);
    let subject_tenant_id = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);

    let ctx = SecurityContext::builder()
        .subject_id(subject_id)
        .subject_tenant_id(subject_tenant_id)
        .token_scopes(vec!["admin".to_owned(), "read:events".to_owned()])
        .build()
        .unwrap();

    let encoded = encode_bin(&ctx).expect("security context encodes");
    let decoded = decode_bin(&encoded).expect("security context decodes");

    // Validate core fields round-trip
    assert_eq!(decoded.subject_id(), ctx.subject_id());
    assert_eq!(decoded.subject_tenant_id(), ctx.subject_tenant_id());
    assert_eq!(decoded.token_scopes(), ctx.token_scopes());
    // bearer_token is #[serde(skip)] so not included in binary encoding
    assert!(decoded.bearer_token().is_none());
}

#[test]
#[allow(clippy::unreadable_literal)] // UUID hex patterns are intentionally repeating
fn decode_rejects_unknown_version() {
    let ctx = SecurityContext::anonymous();

    let mut encoded = encode_bin(&ctx).expect("encodes context");
    encoded[0] = SECCTX_BIN_VERSION.wrapping_add(1);

    let err = decode_bin(&encoded).expect_err("version mismatch should error");
    let message = err.to_string();
    assert!(
        message.contains("unsupported secctx version"),
        "expected version error, got: {message}"
    );
}
