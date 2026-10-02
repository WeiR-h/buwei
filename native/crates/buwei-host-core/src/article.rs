//! A second native-host scene sharing action-receipts with activity invites.
use crate::{Result,account_valid,room_valid,event_valid};
use action_receipts::{Action,Grant,Operation,Status,Evidence};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use rusqlite::{Connection,OptionalExtension,params};
use std::path::Path;

#[derive(Clone,Copy,Debug,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Publication {Draft,Dispatching,Unknown,Published}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Article {
    pub author:String,pub room:String,pub title:String,pub markdown:String,
    pub revision:u64,pub state:Publication,pub operation:Option<String>,
    pub digest:Option<String>,pub server_event:Option<String>,
}
impl Article {
    pub fn new(author:String,room:String,title:String,markdown:String)->Result<Self> {
        let draft=Self{author,room,title,markdown,revision:1,state:Publication::Draft,operation:None,digest:None,server_event:None};draft.validate()?;Ok(draft)
    }
    pub fn validate(&self)->Result<()> {
        if !account_valid(&self.author)||!room_valid(&self.room)||self.revision==0||self.revision>i64::MAX as u64||self.title.trim().is_empty()||self.title.chars().count()>80||self.markdown.trim().is_empty()||self.markdown.len()>24000 {return Err("文章身份、对象或内容不合法".into());}
        if self.state==Publication::Draft {
            if self.operation.is_some()||self.digest.is_some()||self.server_event.is_some(){return Err("草稿含冲突的发布记录".into());}
        } else {
            if self.operation.as_deref().is_none_or(|s|!crate::hex_valid(s,32))||self.digest.as_deref().is_none_or(|s|!crate::hex_valid(s,64)){return Err("文章缺少原操作编号或摘要".into());}
            if self.state==Publication::Published && self.server_event.as_deref().is_none_or(|s|!event_valid(s)){return Err("发布结果缺少服务端证据".into());}
        }
        Ok(())
    }
    pub fn action(&self)->Action {Action{permission:"publish".into(),target:self.room.clone(),summary:format!("发布文章《{}》；作者 {}；完整正文 {} 字节",self.title,self.author,self.markdown.len()),payload:serde_json::json!({"title":self.title,"markdown":self.markdown})}}
    pub fn edit(&mut self,actor:&str,title:String,markdown:String)->Result<()> {
        if actor!=self.author||self.state!=Publication::Draft{return Err("当前身份不能修改待核实或已发布的文章".into());}
        let mut next=self.clone();next.title=title;next.markdown=markdown;next.revision+=1;next.validate()?;*self=next;Ok(())
    }
    pub fn claim(&mut self,op:&Operation)->Result<()> {
        if op.app!="buwei-article"||op.account!=self.author||op.action!=self.action() {return Err("文章预览身份或完整内容不匹配".into());}
        let digest=hex::encode(Sha256::digest(serde_json::to_vec(&op.action).map_err(|_|"文章摘要不可计算")?));
        if op.digest!=digest{return Err("文章摘要不匹配".into());}
        if self.operation.as_deref()==Some(op.id.as_str()) && self.digest.as_deref()==Some(op.digest.as_str()){return Ok(());}
        if self.state!=Publication::Draft||op.status!=Status::Dispatching||self.revision!=op.revision{return Err("文章版本或执行状态已变化".into());}
        self.operation=Some(op.id.clone());self.digest=Some(op.digest.clone());self.state=Publication::Dispatching;self.revision+=1;self.validate()
    }
    pub fn mark_unknown(&mut self,id:&str)->Result<()> {
        if self.operation.as_deref()!=Some(id){return Err("文章操作编号不匹配".into());}
        if self.state==Publication::Dispatching {self.state=Publication::Unknown;self.revision+=1;}Ok(())
    }
    pub fn record(&mut self,e:&Evidence)->Result<()> {
        if self.operation.as_deref()!=Some(e.operation_id.as_str())||self.author!=e.account||self.room!=e.target||self.digest.as_deref()!=Some(e.digest.as_str())||!event_valid(&e.external_id){return Err("文章服务端回执不匹配".into());}
        if let Some(old)=&self.server_event {return if old==&e.external_id {Ok(())}else{Err("同一文章操作存在冲突事件".into())};}
        self.server_event=Some(e.external_id.clone());self.state=Publication::Published;self.revision+=1;self.validate()
    }
}
pub struct ArticleStore {db:Connection}
impl ArticleStore {
    pub fn open(path:impl AsRef<Path>)->Result<Self> {
        let db=Connection::open(path).map_err(|e|e.to_string())?;
        db.busy_timeout(std::time::Duration::from_secs(3)).map_err(|e|e.to_string())?;
        let version:u32=db.query_row("PRAGMA user_version",[],|r|r.get(0)).map_err(|e|e.to_string())?;
        if version>1{return Err("文章记录版本不受支持，原文件保留".into());}
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS article(singleton INTEGER PRIMARY KEY CHECK(singleton=1),revision INTEGER NOT NULL,body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS published_articles(operation TEXT PRIMARY KEY,body TEXT NOT NULL); PRAGMA user_version=1;").map_err(|e|e.to_string())?;Ok(Self{db})
    }
    pub fn load(&self)->Result<Option<Article>> {
        let row:Option<(i64,String)>=self.db.query_row("SELECT revision,body FROM article WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
        row.map(|(revision,body)|{let a:Article=serde_json::from_str(&body).map_err(|_|"文章记录损坏，原文件保留")?;a.validate()?;if revision<1||a.revision!=revision as u64{return Err("文章版本记录不一致".into());}Ok(a)}).transpose()
    }
    pub fn create(&mut self,g:&Grant,a:&Article,now:u64)->Result<()> {
        g.check("create",now).map_err(|_|"文章创建授权已失效")?;a.validate()?;
        if g.app()!="buwei-article"||g.account()!=a.author||a.state!=Publication::Draft{return Err("文章创建身份不匹配".into());}
        self.db.execute("INSERT INTO article VALUES(1,?1,?2)",params![a.revision as i64,serde_json::to_string(a).map_err(|e|e.to_string())?]).map_err(|_|"文章已存在")?;Ok(())
    }
    pub fn start_next(&mut self,g:&Grant,expected:u64,mut draft:Article,now:u64)->Result<Article>{
        g.check("create",now).map_err(|_|"新文章授权已失效")?;draft.validate()?;
        if g.app()!="buwei-article"||g.account()!=draft.author||draft.state!=Publication::Draft{return Err("新草稿身份不匹配".into());}
        let tx=self.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
        let (revision,body):(i64,String)=tx.query_row("SELECT revision,body FROM article WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
        let previous:Article=serde_json::from_str(&body).map_err(|_|"原文章记录不可读取")?;previous.validate()?;
        if revision<1||revision as u64!=expected||previous.revision!=expected||previous.author!=draft.author||previous.room!=draft.room||previous.state!=Publication::Published{return Err("先核实原文章发布结果；未发布或不明状态不能被新草稿覆盖".into());}
        draft.revision=expected.checked_add(1).ok_or("文章版本达到上限")?;draft.validate()?;
        tx.execute("INSERT INTO published_articles(operation,body) VALUES(?1,?2)",params![previous.operation.as_ref().ok_or("原文章编号缺失")?,body]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE article SET revision=?1,body=?2 WHERE singleton=1",params![draft.revision as i64,serde_json::to_string(&draft).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;tx.commit().map_err(|e|e.to_string())?;Ok(draft)
    }
    pub fn published_history(&self)->Result<Vec<Article>>{
        let mut query=self.db.prepare("SELECT body FROM published_articles ORDER BY rowid DESC LIMIT 40").map_err(|e|e.to_string())?;
        let rows=query.query_map([],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?;
        rows.map(|r|{let a:Article=serde_json::from_str(&r.map_err(|e|e.to_string())?).map_err(|_|"文章历史损坏")?;a.validate()?;Ok(a)}).collect()
    }
    pub fn update(&mut self,expected:u64,change:impl FnOnce(&mut Article)->Result<()>)->Result<Article> {
        let tx=self.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
        let (revision,body):(i64,String)=tx.query_row("SELECT revision,body FROM article WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
        if revision<1||revision as u64!=expected{return Err("文章版本已变化".into());}
        let mut a:Article=serde_json::from_str(&body).map_err(|_|"文章记录损坏，原文件保留")?;a.validate()?;if a.revision!=expected{return Err("文章版本记录不一致".into());}
        change(&mut a)?;a.validate()?;tx.execute("UPDATE article SET revision=?1,body=?2 WHERE singleton=1",params![a.revision as i64,serde_json::to_string(&a).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;tx.commit().map_err(|e|e.to_string())?;Ok(a)
    }
}
