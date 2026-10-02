//! Native-only consent preview. UI can return a nonce, never an identity.
use super::*;
pub(crate) struct Consent { pub id:String, account:String, expires:u64 }
impl Consent {
    pub fn prepare(account:String,clock:u64)->Self {Self{id:new_id(),account,expires:clock.saturating_add(120)}}
    pub fn check(&self,id:&str,account:&str,clock:u64)->Result<()> {
        if self.id!=id||self.account!=account||clock>=self.expires{return Err("授权预览已变化或过期，请重新查看授权范围".into());}Ok(())
    }
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn consent_binds_account_nonce_and_deadline(){let p=Consent::prepare("@a:server".into(),100);assert!(p.check(&p.id,"@a:server",101).is_ok());assert!(p.check("other","@a:server",101).is_err());assert!(p.check(&p.id,"@b:server",101).is_err());assert!(p.check(&p.id,"@a:server",220).is_err());}
}
