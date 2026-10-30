pub(super) const DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
// Include schema/revision fields around the maximum document.
pub(super) const NOTE_BODY_BYTES: usize = DOCUMENT_BYTES + 64 * 1024;
// Yjs includes identifiers and editing history beyond the current JSON snapshot.
pub(super) const BRIDGE_BODY_BYTES: usize = 14 * 1024 * 1024;
pub(super) const YJS_STATE_BYTES: usize = 8 * 1024 * 1024;
pub(super) const YJS_BASE64_BYTES: usize = YJS_STATE_BYTES.div_ceil(3) * 4;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maximum_yjs_state_and_document_fit_inside_the_bridge_envelope() {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(vec![0; YJS_STATE_BYTES]);
        assert_eq!(encoded.len(), YJS_BASE64_BYTES);
        assert!(encoded.len() + DOCUMENT_BYTES + 4096 < BRIDGE_BODY_BYTES);
    }
}
