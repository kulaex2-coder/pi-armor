use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct ExtensionInfo {
    pub id: String,
    pub title: String,
}

/// 后期外挂：有第二个板块时再做侧栏。设置在顶栏，不在这里。
pub fn list() -> Vec<ExtensionInfo> {
    vec![ExtensionInfo {
        id: "web".into(),
        title: "Web".into(),
    }]
}
