//! Fixed, actionable messages. Provider details are never echoed to the UI.
pub fn user_message(code: &str, detail: Option<&str>) -> &'static str {
    let detail = detail.unwrap_or_default();
    match code {
        "capability" => "模型授权已失效，请续期后再试",
        "rate" => "请求过于频繁，请稍后再试",
        "budget" => "已达到本机模型调用限额，请在设置中核对预算",
        "no_provider" if detail.contains("模型计数迁移不可核实") => {
            "历史模型计数未通过核验，请保留旧资料并检查预算记录"
        }
        "no_provider" if detail.contains("当前 Windows 用户无法读取私密配置") => {
            "当前 Windows 用户无法读取密钥，请用本机配置工具重新填写"
        }
        "no_provider" => "未能读取本机模型配置，请在账号与设置中填写或更新密钥",
        "invalid_output" | "too_large" => "模型草稿未通过格式校验，请重新准备或手动填写",
        "bad_request" => "模型请求未通过校验，请检查输入后重新准备",
        "provider" if detail.contains("共享模型预算已达上限") => {
            "共享模型预算已达上限，请在设置中核对预算"
        }
        "provider"
            if detail.contains("模型服务 HTTP 401") || detail.contains("模型服务 HTTP 403") =>
        {
            "模型服务拒绝了访问，请核对当前密钥与模型权限"
        }
        "provider" if detail.contains("Kimi Code 服务 HTTP 401") => {
            "Kimi Code 密钥或 K3 权限未通过验证，请核对会员配置"
        }
        "provider" if detail.contains("Kimi Code 服务 HTTP 402") => {
            "Kimi Code 会员权益暂时未通过验证，请核对订阅状态"
        }
        "provider" if detail.contains("Kimi Code 服务 HTTP 403") => {
            "Kimi Code 配额或客户端访问受限，请在官方控制台核对"
        }
        "provider" if detail.contains("Kimi Code 服务 HTTP 429") => {
            "Kimi Code 暂时限制请求，请稍后再试"
        }
        "provider" if detail.contains("模型服务 HTTP 429") => {
            "模型服务暂时限制请求，请稍后再试并核对账户额度"
        }
        "provider"
            if detail.contains("模型服务暂时不可用") || detail.contains("模型响应未完整读取") =>
        {
            "模型服务连接或响应中断，请检查网络后再试"
        }
        "provider" if detail.contains("模型型号或业务结果不匹配") => {
            "服务未返回所配置型号的有效结果，请核对模型权限和服务状态"
        }
        "provider" if detail.contains("预算") || detail.contains("用量") => {
            "模型预算记录未通过核验，请检查本机预算资料"
        }
        _ => "模型服务暂时未完成请求，请稍后再试",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_failures_have_specific_recovery_steps() {
        for (code, detail, expected) in [
            (
                "no_provider",
                "当前 Windows 用户无法读取私密配置",
                "重新填写",
            ),
            ("no_provider", "请先完成本机私密配置", "填写或更新"),
            ("provider", "failed: 模型服务 HTTP 401", "密钥与模型权限"),
            ("provider", "failed: 模型服务 HTTP 403", "密钥与模型权限"),
            ("provider", "failed: 模型服务 HTTP 429", "限制请求"),
            ("provider", "Kimi Code 服务 HTTP 401", "K3 权限"),
            ("provider", "Kimi Code 服务 HTTP 402", "会员权益"),
            ("provider", "Kimi Code 服务 HTTP 403", "官方控制台"),
            ("provider", "Kimi Code 服务 HTTP 429", "稍后再试"),
            ("provider", "failed: 模型服务暂时不可用", "检查网络"),
            (
                "provider",
                "共享模型预算已达上限，手动功能仍可使用",
                "共享模型预算已达上限",
            ),
            ("provider", "模型型号或业务结果不匹配", "模型权限和服务状态"),
            ("invalid_output", "schema details", "格式校验"),
            ("capability", "", "续期"),
        ] {
            assert!(user_message(code, Some(detail)).contains(expected));
        }
    }
    #[test]
    fn untrusted_provider_details_never_reach_the_message() {
        let secret = "sk-private-test-value-123456789";
        let payload = format!(
            "Authorization: Bearer {secret}; C:/private/profile; @person:server; 模型服务 HTTP 401"
        );
        for code in [
            "provider",
            "no_provider",
            "bad_request",
            "invalid_output",
            "unknown",
        ] {
            let message = user_message(code, Some(&payload));
            assert!(!message.contains(secret));
            assert!(!message.contains("@person"));
            assert!(!message.contains("private/profile"));
            assert!(!message.contains("Authorization"));
        }
    }
}
