use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub(in crate::modules) ids: Vec<i64>,
}
#[derive(Clone, Copy)]
pub(in crate::modules) enum Kind {
    Place,
    Course,
    Session,
}
