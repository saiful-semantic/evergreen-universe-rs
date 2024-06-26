#![allow(dead_code)]
use crate::spec;

/// Collection of friendly-named SIP request parameters for common tasks.
///
/// This is not a complete set of friendly-ified parameters.  Just a start.
#[derive(Debug, Clone)]
pub struct ParamSet {
    institution: Option<String>,
    terminal_pwd: Option<String>,
    sip_user: Option<String>,
    sip_pass: Option<String>,
    location: Option<String>,
    patron_id: Option<String>,
    patron_pwd: Option<String>,
    item_id: Option<String>,
    start_item: Option<usize>,
    end_item: Option<usize>,

    /// Fee Paid amount
    pay_amount: Option<String>,

    pay_type: Option<spec::PayType>,

    fee_type: Option<spec::FeeType>,

    /// Fee Paid ILS Transaction ID
    fee_id: Option<String>,

    /// Fee Paid SIP Client / External Transaction ID
    transaction_id: Option<String>,

    /// Indicates which position (if any) of the patron summary string
    /// that should be set to 'Y' (i.e. activated).  Only one summary
    /// index may be activated per message.  Positions are zero-based.
    summary: Option<usize>,
}

impl Default for ParamSet {
    fn default() -> Self {
        Self::new()
    }
}

impl ParamSet {
    pub fn new() -> Self {
        ParamSet {
            institution: None,
            terminal_pwd: None,
            sip_user: None,
            sip_pass: None,
            location: None,
            patron_id: None,
            patron_pwd: None,
            item_id: None,
            start_item: None,
            end_item: None,
            summary: None,
            pay_amount: None,
            transaction_id: None,
            fee_id: None,
            pay_type: None,
            fee_type: None,
        }
    }

    pub fn institution(&self) -> Option<&str> {
        self.institution.as_deref()
    }
    pub fn terminal_pwd(&self) -> Option<&str> {
        self.terminal_pwd.as_deref()
    }
    pub fn sip_user(&self) -> Option<&str> {
        self.sip_user.as_deref()
    }
    pub fn sip_pass(&self) -> Option<&str> {
        self.sip_pass.as_deref()
    }
    pub fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }
    pub fn patron_id(&self) -> Option<&str> {
        self.patron_id.as_deref()
    }
    pub fn patron_pwd(&self) -> Option<&str> {
        self.patron_pwd.as_deref()
    }
    pub fn item_id(&self) -> Option<&str> {
        self.item_id.as_deref()
    }
    pub fn start_item(&self) -> Option<usize> {
        self.start_item
    }
    pub fn end_item(&self) -> Option<usize> {
        self.end_item
    }
    pub fn summary(&self) -> Option<usize> {
        self.summary
    }
    pub fn pay_amount(&self) -> Option<&str> {
        self.pay_amount.as_deref()
    }
    pub fn transaction_id(&self) -> Option<&str> {
        self.transaction_id.as_deref()
    }
    pub fn fee_id(&self) -> Option<&str> {
        self.fee_id.as_deref()
    }
    pub fn pay_type(&self) -> Option<spec::PayType> {
        self.pay_type
    }
    pub fn fee_type(&self) -> Option<spec::FeeType> {
        self.fee_type
    }

    // ---

    pub fn set_institution(&mut self, value: &str) -> &mut Self {
        self.institution = Some(value.to_string());
        self
    }
    pub fn set_terminal_pwd(&mut self, value: &str) -> &mut Self {
        self.terminal_pwd = Some(value.to_string());
        self
    }
    pub fn set_sip_user(&mut self, value: &str) -> &mut Self {
        self.sip_user = Some(value.to_string());
        self
    }
    pub fn set_sip_pass(&mut self, value: &str) -> &mut Self {
        self.sip_pass = Some(value.to_string());
        self
    }
    pub fn set_location(&mut self, value: &str) -> &mut Self {
        self.location = Some(value.to_string());
        self
    }
    pub fn set_patron_id(&mut self, value: &str) -> &mut Self {
        self.patron_id = Some(value.to_string());
        self
    }
    pub fn set_patron_pwd(&mut self, value: &str) -> &mut Self {
        self.patron_pwd = Some(value.to_string());
        self
    }
    pub fn set_item_id(&mut self, value: &str) -> &mut Self {
        self.item_id = Some(value.to_string());
        self
    }
    pub fn set_start_item(&mut self, value: usize) -> &mut Self {
        self.start_item = Some(value);
        self
    }
    pub fn set_end_item(&mut self, value: usize) -> &mut Self {
        self.end_item = Some(value);
        self
    }
    pub fn set_summary(&mut self, value: usize) -> &mut Self {
        self.summary = Some(value);
        self
    }
    pub fn set_pay_amount(&mut self, amount: &str) -> &mut Self {
        self.pay_amount = Some(amount.to_string());
        self
    }
    pub fn set_transaction_id(&mut self, id: &str) -> &mut Self {
        self.transaction_id = Some(id.to_string());
        self
    }
    pub fn set_fee_id(&mut self, id: &str) -> &mut Self {
        self.fee_id = Some(id.to_string());
        self
    }
    pub fn set_pay_type(&mut self, pt: spec::PayType) -> &mut Self {
        self.pay_type = Some(pt);
        self
    }
    pub fn set_fee_type(&mut self, pt: spec::FeeType) -> &mut Self {
        self.fee_type = Some(pt);
        self
    }
}
