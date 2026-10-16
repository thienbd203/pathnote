use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub(super) ids: Vec<i64>,
}
#[derive(Clone, Copy)]
pub(super) enum Kind {
    Place,
    Course,
    Session,
}
