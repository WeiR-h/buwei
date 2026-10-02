use action_receipts::*;
use buwei_host_core::article::*;
use std::sync::{Mutex,atomic::{AtomicUsize,Ordering}};
fn draft()->Article{Article::new("@author:local".into(),"!opaque".into(),"活动小记".into(),"# 活动\n欢迎候补参与。".into()).unwrap()}
fn grant()->(Authority,Grant){let a=Authority::default();a.set_account(Some("@author:local"));let g=a.grant("buwei-article",&["create","publish"],10,300).unwrap();(a,g)}
struct Channel{article:Mutex<Article>,calls:AtomicUsize}
impl Adapter for Channel {
    fn validate(&self,action:&Action,revision:u64)->Result<()>{let a=self.article.lock().unwrap();if a.action()!=*action||a.revision!=revision||a.state!=Publication::Draft{Err(Error::Conflict)}else{Ok(())}}
    fn dispatch(&self,op:&Operation)->Dispatch{if self.article.lock().unwrap().claim(op).is_err(){return Dispatch::Rejected("changed".into());}self.calls.fetch_add(1,Ordering::SeqCst);self.article.lock().unwrap().mark_unknown(&op.id).unwrap();Dispatch::Uncertain("fixture lost ack".into())}
    fn lookup(&self,op:&Operation)->Result<Option<Evidence>>{let e=Evidence{operation_id:op.id.clone(),external_id:"$fixture-article".into(),account:op.account.clone(),target:op.action.target.clone(),digest:op.digest.clone()};self.article.lock().unwrap().record(&e).map_err(|_|Error::Integrity)?;Ok(Some(e))}
}
fn path()->std::path::PathBuf{std::env::temp_dir().join(format!("buwei-article-{}.db",new_id()))}
#[test]fn editing_article_invalidates_exact_preview(){let(_,g)=grant();let mut j=Journal::open(path()).unwrap();let c=Channel{article:Mutex::new(draft()),calls:AtomicUsize::new(0)};let op=j.prepare(&g,c.article.lock().unwrap().action(),1,10,100).unwrap();c.article.lock().unwrap().edit(g.account(),"已修改".into(),"新正文".into()).unwrap();assert!(j.confirm(&g,&op.id,&c,11).is_err());assert_eq!(c.calls.load(Ordering::SeqCst),0);}
#[test]fn lost_ack_holds_article_and_recovery_never_republishes(){let(_,g)=grant();let file=path();let mut j=Journal::open(&file).unwrap();let c=Channel{article:Mutex::new(draft()),calls:AtomicUsize::new(0)};let op=j.prepare(&g,c.article.lock().unwrap().action(),1,10,100).unwrap();j.confirm(&g,&op.id,&c,11).unwrap();let unknown=j.execute(&g,&op.id,&c,12).unwrap();assert_eq!(unknown.status,Status::Unknown);assert!(c.article.lock().unwrap().edit(g.account(),"不同标题".into(),"不同内容".into()).is_err());assert!(j.execute(&g,&op.id,&c,13).is_err());drop(j);let(_,fresh)=grant();let mut reopened=Journal::open(&file).unwrap();let restored=reopened.reconcile(&fresh,&op.id,&c,14).unwrap();assert_eq!(restored.status,Status::Confirmed);assert_eq!(restored.id,op.id);assert_eq!(c.article.lock().unwrap().state,Publication::Published);assert_eq!(c.calls.load(Ordering::SeqCst),1);}
#[test]fn another_author_cannot_edit_or_create(){let mut a=draft();assert!(a.edit("@other:local","改".into(),"改".into()).is_err());let authority=Authority::default();authority.set_account(Some("@other:local"));let g=authority.grant("buwei-article",&["create"],10,100).unwrap();assert!(ArticleStore::open(path()).unwrap().create(&g,&a,11).is_err());}
#[test]fn article_sqlite_rejects_stale_writer_and_survives_reopen(){let(_,g)=grant();let file=path();let mut first=ArticleStore::open(&file).unwrap();first.create(&g,&draft(),10).unwrap();let mut second=ArticleStore::open(&file).unwrap();first.update(1,|a|a.edit(g.account(),"新标题".into(),"正文".into())).unwrap();assert!(second.update(1,|a|a.edit(g.account(),"覆盖".into(),"覆盖".into())).is_err());drop(first);assert_eq!(ArticleStore::open(&file).unwrap().load().unwrap().unwrap().title,"新标题");}
#[test]fn wrong_application_grant_cannot_create_article(){let authority=Authority::default();authority.set_account(Some("@author:local"));let g=authority.grant("buwei",&["create"],10,100).unwrap();assert!(ArticleStore::open(path()).unwrap().create(&g,&draft(),10).is_err());g.revoke();assert!(g.check("create",10).is_err());}
#[test]fn article_bounds_reject_empty_or_oversized_content(){assert!(Article::new("@author:local".into(),"!opaque".into(),"".into(),"正文".into()).is_err());assert!(Article::new("@author:local".into(),"!opaque".into(),"标题".into(),"x".repeat(24001)).is_err());}
#[test]fn next_article_archives_verified_content_and_receipt_atomically(){
    let(_,g)=grant();let file=path();let mut store=ArticleStore::open(&file).unwrap();store.create(&g,&draft(),10).unwrap();
    let mut j=Journal::open(path()).unwrap();let mut op=j.prepare(&g,draft().action(),1,10,100).unwrap();op.status=Status::Dispatching;
    let sent=store.update(1,|a|a.claim(&op)).unwrap();let evidence=Evidence{operation_id:op.id.clone(),external_id:"$published".into(),account:op.account.clone(),target:op.action.target.clone(),digest:op.digest.clone()};
    let published=store.update(sent.revision,|a|a.record(&evidence)).unwrap();let mut next=draft();next.title="下一篇".into();
    let saved=store.start_next(&g,published.revision,next.clone(),20).unwrap();assert_eq!(saved.revision,published.revision+1);assert_eq!(saved.state,Publication::Draft);
    assert!(store.start_next(&g,published.revision,next,21).is_err());drop(store);
    let reopened=ArticleStore::open(file).unwrap();let history=reopened.published_history().unwrap();assert_eq!(history.len(),1);assert_eq!(history[0].title,published.title);assert_eq!(history[0].server_event,Some(evidence.external_id));assert_eq!(history[0].operation,Some(op.id));assert_eq!(reopened.load().unwrap().unwrap().title,"下一篇");
}
#[test]fn new_article_cannot_overwrite_uncertain_publication_or_change_author(){
    let(_,g)=grant();let mut store=ArticleStore::open(path()).unwrap();store.create(&g,&draft(),10).unwrap();
    let mut j=Journal::open(path()).unwrap();let mut op=j.prepare(&g,draft().action(),1,10,100).unwrap();op.status=Status::Dispatching;
    let sent=store.update(1,|a|a.claim(&op)).unwrap();let unknown=store.update(sent.revision,|a|a.mark_unknown(&op.id)).unwrap();
    assert!(store.start_next(&g,unknown.revision,draft(),20).is_err());assert_eq!(store.load().unwrap().unwrap().operation,Some(op.id));
    let mut other=draft();other.author="@other:local".into();assert!(store.start_next(&g,unknown.revision,other,20).is_err());assert!(store.published_history().unwrap().is_empty());
}
