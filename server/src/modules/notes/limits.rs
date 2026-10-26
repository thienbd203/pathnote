pub(super) const BRIDGE_BODY_BYTES: usize = 3 * 1024 * 1024;
pub(super) const YJS_STATE_BYTES: usize = 2_100_000;
pub(super) const YJS_BASE64_BYTES: usize = YJS_STATE_BYTES.div_ceil(3) * 4;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maximum_yjs_state_and_document_fit_inside_the_bridge_envelope() {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(vec![0; YJS_STATE_BYTES]);
        assert_eq!(encoded.len(), YJS_BASE64_BYTES);
        assert!(encoded.len() + 262144 + 4096 < BRIDGE_BODY_BYTES);
    }
}
