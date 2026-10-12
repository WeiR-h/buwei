//! A local file export needs its own exact snapshot and destination preview.
use super::*;
use sha2::{Digest, Sha256};
pub(super) struct ExportPreview {
    pub nonce: String,
    pub account: String,
    pub room: String,
    pub revision: u64,
    pub expires: u64,
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    pub original: Option<String>,
    pub summary: String,
}
fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn destination(path: &str) -> Result<(PathBuf, Option<String>)> {
    let p = PathBuf::from(path);
    if !p.is_absolute()
        || p.extension()
            .and_then(|s| s.to_str())
            .is_none_or(|s| !s.eq_ignore_ascii_case("csv"))
        || p.components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err("请选择完整的 CSV 文件保存路径".into());
    }
    let parent = p
        .parent()
        .ok_or("保存目录不存在")?
        .canonicalize()
        .map_err(|_| "保存目录不存在")?;
    if parent
        .components()
        .any(|s| s.as_os_str() == ".secrets" || s.as_os_str() == ".git")
    {
        return Err("请另选导出目录".into());
    }
    let p = parent.join(p.file_name().ok_or("导出文件名缺失")?);
    if std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_symlink() || !m.is_file()) {
        return Err("导出目标不是普通文件".into());
    }
    let original = if p.exists() {
        Some(digest(
            &std::fs::read(&p).map_err(|_| "原导出文件不可读取")?,
        ))
    } else {
        None
    };
    Ok((p, original))
}
impl Controller {
    pub(super) fn preview_roster(&mut self, path: String) -> Result<String> {
        self.g("read")?;
        if self.activity()?.owner != self.actor() {
            return Err("只有本场组织者可以导出报名名单".into());
        }
        #[cfg(feature = "full-host")]
        if rinx_bridge::official_mode() {
            self.sync_activity()?;
        }
        let a = self.activity()?;
        let (path, original) = destination(&path)?;
        let clock = now();
        let bytes = buwei_host_core::roster::csv(&a, &self.actor(), clock, clock)?;
        let summary = format!(
            "导出账号：{}\n活动：{}\n共 {} 条报名记录；按同行人数统计，已确认 {} 人，保留 {} 人。\n核验北京时间：{}\n保存到：{}\n包含报名序号、显示名、Matrix 身份、同行人数、可用时间和状态。{}",
            self.actor(),
            a.title,
            a.people.len(),
            a.confirmed(),
            a.held(),
            buwei_host_core::calendar::display(clock),
            path.display(),
            if original.is_some() {
                "将替换此路径已有 CSV。"
            } else {
                "将新建 CSV。"
            }
        );
        self.roster_preview = Some(ExportPreview {
            nonce: new_id(),
            account: self.actor(),
            room: a.room,
            revision: a.revision,
            expires: clock + 120,
            path,
            bytes,
            original,
            summary,
        });
        Ok("名单已核验，请确认内容与保存路径。".into())
    }
    pub(super) fn confirm_roster(&mut self, nonce: &str) -> Result<String> {
        self.g("read")?;
        let p = self
            .roster_preview
            .as_ref()
            .ok_or("请先选择路径并预览导出")?;
        let a = self.activity()?;
        if p.nonce != nonce
            || p.account != self.actor()
            || a.owner != self.actor()
            || p.room != a.room
            || p.revision != a.revision
            || now() >= p.expires
        {
            return Err("名单、账号或预览已变化，请重新导出".into());
        }
        let (path, original) = destination(p.path.to_str().ok_or("保存路径不合法")?)?;
        if path != p.path || original != p.original {
            return Err("保存目标已变化，请重新预览".into());
        }
        let temporary = path.with_extension(format!("{}.tmp", new_id()));
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| "导出目录不可写")?;
        f.write_all(&p.bytes)
            .and_then(|_| f.sync_all())
            .map_err(|_| "导出尚未写入完成")?;
        drop(f);
        std::fs::rename(&temporary, &path).map_err(|_| "文件不可保存，请保留临时导出文件")?;
        if digest(&std::fs::read(&path).map_err(|_| "保存后不可核验")?) != digest(&p.bytes) {
            return Err("导出校验未通过，请保留文件".into());
        }
        let result = format!("名单已保存并校验：{}", path.display());
        self.roster_preview = None;
        Ok(result)
    }
}
#[cfg(windows)]
pub(crate) fn choose_path() -> Option<String> {
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetSaveFileNameW, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    let mut file = vec![0u16; 32768];
    let initial: Vec<u16> = "补位报名名单.csv".encode_utf16().collect();
    file[..initial.len()].copy_from_slice(&initial);
    let filter: Vec<u16> = "CSV 报名名单\0*.csv\0\0".encode_utf16().collect();
    let title: Vec<u16> = "选择报名名单的保存位置\0".encode_utf16().collect();
    let extension: Vec<u16> = "csv\0".encode_utf16().collect();
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.lpstrFile = file.as_mut_ptr();
    dialog.nMaxFile = file.len() as u32;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrDefExt = extension.as_ptr();
    dialog.Flags = OFN_NOCHANGEDIR | OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT;
    if unsafe { GetSaveFileNameW(&mut dialog) } == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(
        &file[..file.iter().position(|c| *c == 0).unwrap_or(file.len())],
    ))
}
#[cfg(not(windows))]
pub(crate) fn choose_path() -> Option<String> {
    None
}
