use serde::Serialize;
use sha2::{Digest, Sha256};
/// Hash the existing JSON serialization; callers own normalization and identity semantics.
pub(crate) fn json_hash<T: Serialize + ?Sized>(value: &T) -> Result<String, serde_json::Error> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hashes_remain_compatible_with_existing_utf8_json_identities() {
        assert_eq!(
            json_hash(&serde_json::json!([null, null])).unwrap(),
            "95cb9b4f84ceff132cc7a875d8c192bf4997016a939ee64141c1fd628c0e8738"
        );
        assert_eq!(
            json_hash(&serde_json::json!(["course", 7, null, "Học tập"])).unwrap(),
            "e466026d44fe77ed0aab6755f7245175d6249211c9bfaebe1b9c3997500ab2a1"
        );
        assert_eq!(
            json_hash(&serde_json::json!(["hello", "en-US", "noun", "xin chào"])).unwrap(),
            "d2d4e8bb730bbe5ece39f3f4f47059c894f2af76764e08030a46625f1d61f921"
        );
    }
}
