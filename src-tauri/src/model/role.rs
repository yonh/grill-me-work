/// Preset role definitions: (id, display_name, icon, description)
pub const PRESET_ROLES: &[(&str, &str, &str, &str)] = &[
    ("pm", "需求方/产品经理", "users", "厘清要什么：功能、权限、数据、业务流程"),
    ("dev", "开发者/技术负责人", "code", "怎么实现：技术栈、架构、API、数据模型"),
    ("designer", "设计师", "palette", "用户怎么用：交互、视觉、流程、可访问性"),
    ("manager", "项目经理", "clipboard", "范围与优先级：里程碑、风险、资源、边界"),
];

pub fn is_preset_role(role: &str) -> bool {
    PRESET_ROLES.iter().any(|(id, _, _, _)| *id == role)
}

pub fn role_display_name(role: &str) -> String {
    if let Some((_, name, _, _)) = PRESET_ROLES.iter().find(|(id, _, _, _)| *id == role) {
        return name.to_string();
    }
    role.to_string()
}
