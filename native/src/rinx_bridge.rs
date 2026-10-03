//! The current client comes only from Rinx. No app-supplied actor or token.
use super::*;
use sha2::{Digest, Sha256};
pub fn official_mode() -> bool {
    std::env::args().any(|a| a == "--official-rinx")
}
fn allowed(c: &Client) -> bool {
    let url = c.homeserver();
    url.scheme() == "https"
        && url.host_str() == Some("matrix.rinx.chat")
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && matches!(url.port(), None | Some(443))
        && matches!(url.path(), "" | "/")
}
fn session_stamp(c: &Client) -> Result<[u8; 32]> {
    let meta = c.session_meta().ok_or("Rinx 尚未完成登录")?;
    let mut secret = c.access_token().ok_or("Rinx 登录会话不可用")?.into_bytes();
    let mut h = Sha256::new();
    h.update(meta.user_id.as_str());
    h.update([0]);
    h.update(meta.device_id.as_str());
    h.update([0]);
    h.update(&secret);
    secret.fill(0);
    Ok(h.finalize().into())
}
pub fn ensure_current(c: &Client) -> Result<()> {
    let active = rinx::sliding_sync::get_client().ok_or("请先在 Rinx 完成浏览器登录")?;
    if !allowed(&active) || !allowed(c) {
        return Err("仅允许官方 HTTPS 聊天服务器；未使用本机测试身份".into());
    }
    if session_stamp(&active)? != session_stamp(c)? {
        return Err("Rinx 当前会话已变化，旧确认失效".into());
    }
    Ok(())
}
pub fn verify_current(rt: &Runtime, c: &Client) -> Result<()> {
    ensure_current(c)?;
    let response = rt
        .block_on(async { tokio::time::timeout(Duration::from_secs(8), c.whoami()).await })
        .map_err(|_| "正式服务器身份核验超时；尚未执行动作")?
        .map_err(|_| "正式服务器身份核验失败；尚未执行动作")?;
    if c.user_id() != Some(&response.user_id) {
        return Err("Rinx 与服务端返回的账号不一致".into());
    }
    if response.device_id.as_deref() != c.device_id() {
        return Err("Rinx 与服务端返回的设备不一致".into());
    }
    ensure_current(c)
}
pub fn verified_current(rt: &Runtime) -> Result<Client> {
    let c = rinx::sliding_sync::get_client()
        .ok_or("请先在 Rinx 填写 matrix.rinx.chat 并完成本人浏览器登录")?;
    verify_current(rt, &c)?;
    Ok(c)
}
pub fn record_status(
    root: &Path,
    actor: Option<&str>,
    verified: bool,
    authorized: bool,
) -> Result<()> {
    let dir = root
        .join("data")
        .join(format!("v{}", env!("CARGO_PKG_VERSION")));
    std::fs::create_dir_all(&dir).map_err(|_| "状态目录不可用")?;
    let v = json!({"snapshot_at_unix":now(),"process_id":std::process::id(),"kind":"local status snapshot, not an authorization token","version":env!("CARGO_PKG_VERSION"),"homeserver":"https://matrix.rinx.chat","identity_source":"rinx_host_current_client","account":actor,"server_identity_verified":verified,"action_authorized":authorized,"automatic_new_invitations":false,"automatic_articles":false,"automatic_state_sync_authorized":authorized});
    std::fs::write(
        dir.join("rinx-binding-status.json"),
        serde_json::to_vec_pretty(&v).map_err(|_| "状态格式不可用")?,
    )
    .map_err(|_| "绑定状态不可保存".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn app_supplied_sdk_session_cannot_replace_a_rinx_login() {
        let rt = Runtime::new().unwrap();
        let c = rt.block_on(async {
            let c = Client::builder()
                .homeserver_url("https://matrix.rinx.chat")
                .build()
                .await
                .unwrap();
            let session: MatrixSession = serde_json::from_value(json!({
                "user_id":"@fixture:matrix.rinx.chat","device_id":"FAKE_TEST_DEVICE",
                "access_token":"nonlive-unit-test-placeholder","refresh_token":null
            }))
            .unwrap();
            c.restore_session(session).await.unwrap();
            c
        });
        assert!(c.user_id().is_some());
        assert!(rinx::sliding_sync::get_client().is_none());
        assert!(ensure_current(&c).is_err());
    }
}
