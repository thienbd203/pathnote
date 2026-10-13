use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkAttrs {
    pub href: String,
    pub target: Option<String>,
    pub rel: Option<String>,
    pub class: Option<String>,
    pub title: Option<String>,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StyleAttrs {
    pub color: Option<String>,
    pub background_color: Option<String>,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Mark {
    Bold {},
    Italic {},
    Strike {},
    Code {},
    Underline {},
    Superscript {},
    Subscript {},
    Link {
        attrs: LinkAttrs,
    },
    TextStyle {
        #[serde(default)]
        attrs: StyleAttrs,
    },
}
