use action_receipts::{Action, Adapter, Authority, Dispatch, Error, Evidence, Grant, Journal, Operation, Status, new_id};
use buwei_host_core::*;
use serde_json::json;
use std::sync::Mutex;
const OWNER:&str="@owner:local";const FIRST:&str="@first:local";const SECOND:&str="@second:local";const ROOM:&str="!activity:local";
fn prefs()->Preferences {Preferences{earliest:17,latest:23,group:1}}
#[test] fn opaque_matrix_room_ids_are_supported() {assert!(Activity::new(OWNER.into(),"!opaqueMatrixRoom".into(),"活动".into(),1,19,21).is_ok());assert!(Activity::new(OWNER.into(),"!".into(),"活动".into(),1,19,21).is_err());}
fn activity()->Activity {let mut a=Activity::new(OWNER.into(),ROOM.into(),"羽毛球".into(),1,19,21).unwrap();a.add_person(FIRST.into(),"小一".into(),prefs(),true).unwrap();a.add_person(SECOND.into(),"小二".into(),prefs(),true).unwrap();a}
fn path()->std::path::PathBuf {std::env::temp_dir().join(format!("buwei-host-{}.db",new_id()))}
fn grant()->(Authority,Grant) {let a=Authority::default();a.set_account(Some(OWNER));let g=a.grant("buwei",&["invite","create"],100,1000).unwrap();(a,g)}
struct Host {state:Mutex<Activity>, mode:u8}
impl Adapter for Host {
    fn validate(&self,action:&Action,revision:u64)->action_receipts::Result<()> {let a=self.state.lock().unwrap();if revision!=a.revision || action.target!=a.room || a.candidate()!=action.payload["person"].as_str(){Err(Error::Conflict)}else{Ok(())}}
    fn dispatch(&self,op:&Operation)->Dispatch {
        let mut a=self.state.lock().unwrap();if a.reserve(op,102).is_err(){return Dispatch::Rejected("规则已变化".into());}
        if self.mode==1 {a.mark_unknown(&op.id).unwrap();return Dispatch::Uncertain("回执待核实".into());}
        let event=format!("${}",op.id);a.record_delivery(&op.id,OWNER,ROOM,&event,&op.digest).unwrap();
        Dispatch::Verified(Evidence{operation_id:op.id.clone(),external_id:event,account:OWNER.into(),target:ROOM.into(),digest:op.digest.clone()})
    }
    fn lookup(&self,op:&Operation)->action_receipts::Result<Option<Evidence>> {let a=self.state.lock().unwrap();Ok(a.invitations.iter().find(|i|i.operation_id==op.id).and_then(|i|i.server_event.as_ref()).map(|id|Evidence{operation_id:op.id.clone(),external_id:id.clone(),account:OWNER.into(),target:ROOM.into(),digest:op.digest.clone()}))}
}
fn queued(host:&Host,j:&mut Journal,g:&Grant)->Operation {
    let a=host.state.lock().unwrap();let op=j.prepare(g,Action{permission:"invite".into(),target:ROOM.into(),summary:"邀请下一位候补".into(),payload:json!({"person":a.candidate().unwrap(),"until":200})},a.revision,100,120).unwrap();drop(a);j.confirm(g,&op.id,host,101).unwrap()
}
fn executed(mode:u8)->(Host,Grant,Journal,Operation) {let(_,g)=grant();let mut j=Journal::open(path()).unwrap();let h=Host{state:Mutex::new(activity()),mode};let op=queued(&h,&mut j,&g);let done=j.execute(&g,&op.id,&h,102).unwrap();(h,g,j,done)}

#[test]fn sequential_invite_accept_cancel_next() {let(h,_,_,op)=executed(0);let mut a=h.state.lock().unwrap();assert_eq!(a.held(),1);assert_eq!(a.confirmed(),0);a.receive_reply(&op.id,FIRST,ROOM,&format!("${}",op.id),true,103,104).unwrap();assert_eq!(a.held(),0);assert_eq!(a.confirmed(),1);assert_eq!(a.candidate(),None);a.cancel_own(FIRST).unwrap();assert_eq!(a.candidate(),Some(SECOND));}
#[test]fn duplicate_execute_and_duplicate_reply_do_not_take_another_seat() {let(h,g,mut j,op)=executed(0);j.execute(&g,&op.id,&h,103).unwrap();let mut a=h.state.lock().unwrap();assert_eq!(a.invitations.len(),1);a.receive_reply(&op.id,FIRST,ROOM,&format!("${}",op.id),true,103,104).unwrap();assert!(a.receive_reply(&op.id,FIRST,ROOM,&format!("${}",op.id),true,104,105).is_err());assert_eq!(a.confirmed(),1);}
#[test]fn other_sender_cannot_accept_invitation() {let(h,_,_,op)=executed(0);let mut a=h.state.lock().unwrap();assert!(a.receive_reply(&op.id,SECOND,ROOM,&format!("${}",op.id),true,103,104).is_err());assert_eq!(a.confirmed(),0);assert_eq!(a.held(),1);}
#[test]fn wrong_room_or_wrong_relation_cannot_accept() {let(h,_,_,op)=executed(0);let mut a=h.state.lock().unwrap();assert!(a.receive_reply(&op.id,FIRST,"!other:local",&format!("${}",op.id),true,103,104).is_err());assert!(a.receive_reply(&op.id,FIRST,ROOM,"$unrelated",true,103,104).is_err());assert_eq!(a.confirmed(),0);}
#[test]fn deadline_and_future_server_time_cannot_accept() {let(h,_,_,op)=executed(0);let mut a=h.state.lock().unwrap();for(server,now)in[(200,200),(201,220),(300,104)] {assert!(a.receive_reply(&op.id,FIRST,ROOM,&format!("${}",op.id),true,server,now).is_err());}assert_eq!(a.confirmed(),0);}
#[test]fn timely_reply_is_applied_before_expiry_even_when_read_late(){let(h,_,_,op)=executed(0);let mut a=h.state.lock().unwrap();a.receive_reply(&op.id,FIRST,ROOM,&format!("${}",op.id),true,199,230).unwrap();assert_eq!(a.expire(230),0);assert_eq!(a.confirmed(),1);assert_eq!(a.free(),0);}
#[test]fn released_reservation_cannot_be_taken_by_an_old_timely_reply(){let(h,_,_,op)=executed(0);let mut a=h.state.lock().unwrap();a.expire(200);assert!(a.receive_reply(&op.id,FIRST,ROOM,&format!("${}",op.id),true,199,230).is_err());assert_eq!(a.confirmed(),0);assert_eq!(a.candidate(),Some(SECOND));}
#[test]fn checkpoint_survives_restart_and_is_bound_to_room(){let(_,g)=grant();let p=path();let mut s=Store::open(&p).unwrap();s.create(&g,&activity(),101).unwrap();assert_eq!(s.sync_checkpoint(ROOM).unwrap(),None);s.save_sync_checkpoint(ROOM,"$last-sdk-event").unwrap();assert!(s.save_sync_checkpoint("!other","$different").is_err());assert!(s.save_sync_checkpoint(ROOM,"invalid").is_err());drop(s);assert_eq!(Store::open(&p).unwrap().sync_checkpoint(ROOM).unwrap().as_deref(),Some("$last-sdk-event"));}
#[test]fn stale_pending_snapshot_blocks_reply_at_deadline_before_send(){let(h,_,_,op)=executed(0);let a=h.state.lock().unwrap();let original=serde_json::to_value(&*a).unwrap();assert_eq!(a.pending_invitation_for(FIRST,199).unwrap().operation_id,op.id);assert!(a.pending_invitation_for(FIRST,200).is_err());assert!(a.pending_invitation_for(FIRST,201).is_err());assert_eq!(original,serde_json::to_value(&*a).unwrap());}
#[test]fn reply_preflight_requires_recipient_delivery_and_pending_result(){let(h,_,_,op)=executed(0);let mut a=h.state.lock().unwrap();assert!(a.pending_invitation_for(SECOND,103).is_err());a.invitations[0].delivery=Delivery::Unknown;assert!(a.pending_invitation_for(FIRST,103).is_err());a.invitations[0].delivery=Delivery::Delivered;a.receive_reply(&op.id,FIRST,ROOM,&format!("${}",op.id),true,103,104).unwrap();assert!(a.pending_invitation_for(FIRST,105).is_err());assert_eq!(a.confirmed(),1);}
#[test]fn decline_moves_to_next_eligible_person() {let(h,_,_,op)=executed(0);let mut a=h.state.lock().unwrap();a.receive_reply(&op.id,FIRST,ROOM,&format!("${}",op.id),false,103,104).unwrap();assert_eq!(a.candidate(),Some(SECOND));assert_eq!(a.confirmed(),0);}
#[test]fn expiration_does_not_automatically_reinvite_same_person() {let(h,_,_,_)=executed(0);let mut a=h.state.lock().unwrap();assert_eq!(a.expire(200),1);assert_eq!(a.expire(201),0);assert_eq!(a.people[0].status,PersonStatus::Expired);assert_eq!(a.candidate(),Some(SECOND));}
#[test]fn uncertain_send_holds_seat_until_verified_even_after_deadline() {let(h,_,_,op)=executed(1);assert_eq!(op.status,Status::Unknown);let mut a=h.state.lock().unwrap();assert_eq!(a.expire(200),0);assert_eq!(a.free(),0);assert_eq!(a.candidate(),None);assert!(a.receive_reply(&op.id,FIRST,ROOM,"$claimed",true,103,104).is_err());a.record_delivery(&op.id,OWNER,ROOM,"$verified",&op.digest).unwrap();assert_eq!(a.expire(201),1);assert_eq!(a.candidate(),Some(SECOND));}
#[test]fn stale_confirmation_never_creates_reservation() {let(_,g)=grant();let mut j=Journal::open(path()).unwrap();let h=Host{state:Mutex::new(activity()),mode:0};let op=queued(&h,&mut j,&g);h.state.lock().unwrap().paused=true;h.state.lock().unwrap().revision+=1;assert!(j.execute(&g,&op.id,&h,102).is_err());assert_eq!(h.state.lock().unwrap().held(),0);}
#[test]fn prepared_or_queued_action_cannot_bypass_confirmation() {let(_,g)=grant();let mut j=Journal::open(path()).unwrap();let h=Host{state:Mutex::new(activity()),mode:0};let mut op=queued(&h,&mut j,&g);let mut a=h.state.lock().unwrap();assert!(a.reserve(&op,102).is_err());op.status=Status::Prepared;assert!(a.reserve(&op,102).is_err());assert_eq!(a.held(),0);}
#[test]fn stored_action_content_and_actor_cannot_be_changed() {let(h,_,_,mut op)=executed(0);let mut a=h.state.lock().unwrap();op.account=SECOND.into();assert!(a.reserve(&op,103).is_err());op.account=OWNER.into();op.action.payload["person"]=json!(SECOND);assert!(a.reserve(&op,103).is_err());assert_eq!(a.held(),1);}
#[test]fn advice_is_typed_but_does_not_change_queue_or_capacity() {let a=activity();for v in [json!({"earliest":17,"latest":23,"group":1,"needs_clarification":false,"explanation":"理解候补需求","sender":OWNER}),json!({"earliest":23,"latest":17,"group":1,"needs_clarification":false,"explanation":"错误时段"}),json!({"earliest":17,"latest":23,"group":31,"needs_clarification":false,"explanation":"越界人数"})] {assert!(ModelAdvice::parse(&v).is_err());}assert_eq!(a.candidate(),Some(FIRST));assert_eq!(a.capacity,1);}
#[test]fn unconfirmed_preferences_groups_and_wrong_times_do_not_jump_queue() {let mut a=activity();a.people[0].preferences_confirmed=false;assert_eq!(a.candidate(),Some(SECOND));a.people[0].preferences_confirmed=true;a.people[0].preferences.group=2;assert_eq!(a.candidate(),Some(SECOND));a.people[0].preferences.group=1;a.people[0].preferences.latest=20;assert_eq!(a.candidate(),Some(SECOND));}
#[test]fn max_thirty_people_and_duplicate_identity_are_rejected() {let mut a=activity();assert!(a.add_person(FIRST.into(),"重复".into(),prefs(),true).is_err());for n in 2..30 {a.add_person(format!("@p{n}:local"),"候补".into(),prefs(),true).unwrap();}assert!(a.add_person("@over:local".into(),"超员".into(),prefs(),true).is_err());assert_eq!(a.people.len(),30);}
#[test]fn sqlite_restart_preserves_hold_and_operation_number() {let(h,g,_,op)=executed(1);let p=path();let mut store=Store::open(&p).unwrap();store.create(&g,&h.state.lock().unwrap(),103).unwrap();drop(store);let a=Store::open(&p).unwrap().load().unwrap().unwrap();assert_eq!(a.invitations[0].operation_id,op.id);assert_eq!(a.invitations[0].delivery,Delivery::Unknown);assert_eq!(a.held(),1);}
#[test]fn stale_sqlite_writer_rolls_back_its_change() {let(_,g)=grant();let p=path();let mut one=Store::open(&p).unwrap();let a=activity();one.create(&g,&a,101).unwrap();let mut two=Store::open(&p).unwrap();one.update(a.revision,|a|a.confirm_preferences(FIRST,Preferences{earliest:18,latest:23,group:1})).unwrap();assert!(two.update(a.revision,|a|a.confirm_preferences(SECOND,prefs())).is_err());assert_eq!(two.load().unwrap().unwrap().revision,a.revision+1);}
#[test]fn revoked_or_switched_account_cannot_create_activity() {let(_a,g)=grant();g.revoke();assert!(Store::open(path()).unwrap().create(&g,&activity(),101).is_err());let(a,g)=grant();a.set_account(Some(SECOND));assert!(Store::open(path()).unwrap().create(&g,&activity(),101).is_err());drop(a);}
#[test]fn model_job_digest_changes_with_text_or_business_revision() {assert_ne!(advice_digest("晚上可以",1),advice_digest("晚上可以",2));assert_ne!(advice_digest("晚上可以",1),advice_digest("晚上不行",1));}

#[test] fn own_preference_update_preserves_queue_position() {
    let mut a=activity();let joined=a.people[0].joined;
    a.join_own(FIRST.into(),"小一".into(),Preferences{earliest:18,latest:22,group:1}).unwrap();
    assert_eq!(a.people.len(),2);assert_eq!(a.people[0].joined,joined);
    assert_eq!(a.candidate(),Some(FIRST));
}
#[test] fn cancelled_person_rejoins_at_end_of_queue() {
    let (h,_,_,op)=executed(0);let mut a=h.state.lock().unwrap();
    a.receive_reply(&op.id,FIRST,ROOM,&format!("${}",op.id),true,103,104).unwrap();a.cancel_own(FIRST).unwrap();
    a.join_own(FIRST.into(),"小一".into(),prefs()).unwrap();
    assert_eq!(a.people.len(),2);assert!(a.people[0].joined>a.people[1].joined);
    assert_eq!(a.people[0].status,PersonStatus::Waiting);assert_eq!(a.candidate(),Some(SECOND));
}
#[test] fn held_and_confirmed_person_cannot_join_again() {
    let (h,_,_,op)=executed(0);let mut a=h.state.lock().unwrap();let before=a.clone();
    assert!(a.join_own(FIRST.into(),"小一".into(),prefs()).is_err());assert_eq!(a.revision,before.revision);
    a.receive_reply(&op.id,FIRST,ROOM,&format!("${}",op.id),true,103,104).unwrap();
    assert!(a.join_own(FIRST.into(),"小一".into(),prefs()).is_err());assert_eq!(a.confirmed(),1);
}

#[test]fn verified_inbox_and_business_change_commit_together_across_restart(){
    let(_,g)=grant();let p=path();let mut s=Store::open(&p).unwrap();let a=activity();s.create(&g,&a,101).unwrap();
    assert_eq!(s.apply_verified_event("$one",|a|a.confirm_preferences(FIRST,Preferences{earliest:18,latest:23,group:1})).unwrap(),Some(true));
    let revision=s.load().unwrap().unwrap().revision;drop(s);let mut s=Store::open(&p).unwrap();
    assert_eq!(s.apply_verified_event("$one",|a|a.confirm_preferences(FIRST,prefs())).unwrap(),None);assert_eq!(s.load().unwrap().unwrap().revision,revision);
}
#[test]fn rejected_inbox_operation_rolls_back_partial_mutation(){
    let(_,g)=grant();let mut s=Store::open(path()).unwrap();let a=activity();s.create(&g,&a,101).unwrap();
    assert_eq!(s.apply_verified_event("$bad",|a|{a.people.clear();Err("reject".into())}).unwrap(),Some(false));
    assert_eq!(s.load().unwrap().unwrap().people.len(),2);assert_eq!(s.apply_verified_event("$bad",|_|Ok(())).unwrap(),None);
}
#[test]fn participant_cache_rejects_room_changes_revision_rollback_and_equivocation(){
    let mut s=Store::open(path()).unwrap();let a=activity();s.cache_verified_snapshot(&a).unwrap();
    let mut changed=a.clone();changed.room="!other".into();assert!(s.cache_verified_snapshot(&changed).is_err());
    changed=a.clone();changed.title="same revision different content".into();assert!(s.cache_verified_snapshot(&changed).is_err());
    changed=a.clone();changed.confirm_preferences(FIRST,Preferences{earliest:18,latest:23,group:1}).unwrap();s.cache_verified_snapshot(&changed).unwrap();assert!(s.cache_verified_snapshot(&a).is_err());
}
