use super::item;
use super::session::Session;
use chrono::NaiveDateTime;
use eg::common::circulator::Circulator;
use eg::constants as C;
use eg::result::EgResult;
use evergreen as eg;
use std::collections::HashMap;

// TODO move AlerType into sip2::spec
pub enum AlertType {
    Unknown,
    LocalHold,
    RemoteHold,
    Ill,
    Transit,
    Other,
}

impl From<&str> for AlertType {
    fn from(v: &str) -> AlertType {
        match v {
            "00" => Self::Unknown,
            "01" => Self::LocalHold,
            "02" => Self::RemoteHold,
            "03" => Self::Ill,
            "04" => Self::Transit,
            "99" => Self::Other,
            _ => panic!("Unknown alert type: {}", v),
        }
    }
}

impl From<AlertType> for &str {
    fn from(a: AlertType) -> &'static str {
        match a {
            AlertType::Unknown => "00",
            AlertType::LocalHold => "01",
            AlertType::RemoteHold => "02",
            AlertType::Ill => "03",
            AlertType::Transit => "04",
            AlertType::Other => "99",
        }
    }
}

pub struct CheckinResult {
    ok: bool,
    current_loc: String,
    permanent_loc: String,
    destination_loc: Option<String>,
    patron_barcode: Option<String>,
    alert_type: Option<AlertType>,
    hold_patron_name: Option<String>,
    hold_patron_barcode: Option<String>,
}

impl Session {
    pub fn handle_checkin(&mut self, msg: &sip2::Message) -> EgResult<sip2::Message> {
        self.set_authtoken()?;

        let barcode = msg
            .get_field_value("AB")
            .ok_or_else(|| format!("handle_item_info() missing item barcode"))?;

        let current_loc_op = msg.get_field_value("AP");
        let return_date = &msg.fixed_fields()[2];

        // KCLS only
        // cancel == un-fulfill hold this copy currently fulfills
        let undo_hold_fulfillment = match msg.get_field_value("BI") {
            Some(v) => v.eq("Y"),
            None => false,
        };

        log::info!("{self} Checking in item {barcode}");

        let item = match self.get_item_details(&barcode)? {
            Some(c) => c,
            None => {
                return Ok(self.return_checkin_item_not_found(&barcode));
            }
        };

        let mut blocked_on_co = false;
        let result = match self.handle_block_on_checked_out(&item) {
            Some(r) => {
                blocked_on_co = true;
                r
            }
            None => self.checkin(
                &item,
                current_loc_op,
                return_date.value(),
                undo_hold_fulfillment,
                self.account().settings().checkin_override_all(),
            )?,
        };

        let mut resp = sip2::Message::from_values(
            &sip2::spec::M_CHECKIN_RESP,
            &[
                sip2::util::num_bool(result.ok),                   // checkin ok
                sip2::util::sip_bool(!item.magnetic_media),        // resensitize
                sip2::util::sip_bool(item.magnetic_media),         // magnetic
                sip2::util::sip_bool(result.alert_type.is_some()), // alert
                &sip2::util::sip_date_now(),
            ],
            &[
                ("AB", &barcode),
                ("AO", self.account().settings().institution()),
                ("AJ", &item.title),
                ("AP", &result.current_loc),
                ("AQ", &result.permanent_loc),
                ("BG", &item.owning_loc),
                ("BT", &item.fee_type),
                ("CI", "N"), // security inhibit
            ],
        )
        .unwrap();

        if let Some(ref bc) = result.patron_barcode {
            resp.add_field("AA", bc);
        }
        if let Some(at) = result.alert_type {
            resp.add_field("CV", at.into());
        }
        if let Some(ref loc) = result.destination_loc {
            resp.add_field("CT", loc);
        }
        if let Some(ref bc) = result.hold_patron_barcode {
            resp.add_field("CY", bc);
        }
        if let Some(ref n) = result.hold_patron_name {
            resp.add_field("DA", n);
        }
        if blocked_on_co {
            resp.add_field("AF", "Item Is Currently Checked Out");
        }

        Ok(resp)
    }

    /// Returns a CheckinResult if the checkin is blocked due to the
    /// item being currently checked out.
    fn handle_block_on_checked_out(&self, item: &item::Item) -> Option<CheckinResult> {
        if !self.account().checkin_block_on_checked_out() {
            return None;
        }

        if item.copy_status != C::COPY_STATUS_CHECKED_OUT {
            return None;
        }

        log::info!("Blocking checkin on checked out item");

        Some(CheckinResult {
            ok: false,
            current_loc: item.current_loc.to_string(),
            permanent_loc: item.permanent_loc.to_string(),
            destination_loc: None,
            patron_barcode: None,
            alert_type: Some(AlertType::Other),
            hold_patron_name: None,
            hold_patron_barcode: None,
        })
    }

    fn return_checkin_item_not_found(&self, barcode: &str) -> sip2::Message {
        sip2::Message::from_values(
            &sip2::spec::M_CHECKIN_RESP,
            &[
                "0", // checkin ok
                "N", // resensitize
                "N", // magnetic
                "N", // alert
                &sip2::util::sip_date_now(),
            ],
            &[
                ("AB", &barcode),
                ("AO", self.account().settings().institution()),
                ("CV", AlertType::Unknown.into()),
            ],
        )
        .unwrap()
    }

    fn checkin(
        &mut self,
        item: &item::Item,
        current_loc_op: Option<&str>,
        return_date: &str,
        cancel: bool,
        ovride: bool,
    ) -> EgResult<CheckinResult> {
        if self.account().settings().use_native_checkin() {
            self.checkin_native(item, current_loc_op, return_date, cancel, ovride)
        } else {
            self.checkin_api(item, current_loc_op, return_date, cancel, ovride)
        }
    }

    /// Checkin variant that calls the traditional open-ils.circ APIs.
    fn checkin_api(
        &mut self,
        item: &item::Item,
        current_loc_op: Option<&str>,
        return_date: &str,
        cancel: bool,
        ovride: bool,
    ) -> EgResult<CheckinResult> {
        let mut args = json::object! {
            copy_barcode: item.barcode.as_str(),
            hold_as_transit: self.account().settings().checkin_holds_as_transits(),
        };

        if cancel {
            args["revert_hold_fulfillment"] = json::from(cancel);
        }

        if return_date.trim().len() == 18 {
            let fmt = sip2::spec::SIP_DATE_FORMAT;

            // Use NaiveDate since SIP dates don't typically include a
            // time zone value.
            if let Some(sip_date) = NaiveDateTime::parse_from_str(return_date, fmt).ok() {
                let iso_date = sip_date.format("%Y-%m-%d").to_string();
                log::info!("{self} Checking in with backdate: {iso_date}");

                args["backdate"] = json::from(iso_date);
            } else {
                log::warn!("{self} Invalid checkin return date: {return_date}");
            }
        }

        if let Some(sn) = current_loc_op {
            if let Some(org) = self.org_from_sn(sn)? {
                args["circ_lib"] = org["id"].clone();
            }
        }

        if !args.has_key("circ_lib") {
            args["circ_lib"] = json::from(self.get_ws_org_id()?);
        }

        let method = match ovride {
            true => "open-ils.circ.checkin.override",
            false => "open-ils.circ.checkin",
        };

        let params = vec![json::from(self.authtoken()?), args];

        let mut resp =
            match self
                .osrf_client_mut()
                .send_recv_one("open-ils.circ", method, params)?
            {
                Some(r) => r,
                None => Err(format!("API call {method} failed to return a response"))?,
            };

        log::debug!("{self} Checkin of {} returned: {resp}", item.barcode);

        let evt_json = if resp.is_array() {
            resp[0].take()
        } else {
            resp
        };

        let evt = eg::event::EgEvent::parse(&evt_json)
            .ok_or(format!("API call {method} failed to return an event"))?;

        if !ovride
            && self
                .account()
                .settings()
                .checkin_override()
                .contains(&evt.textcode().to_string())
        {
            return self.checkin(item, current_loc_op, return_date, cancel, true);
        }

        let mut current_loc = item.current_loc.to_string(); // item.circ_lib
        let mut permanent_loc = item.permanent_loc.to_string(); // item.circ_lib
        let mut destination_loc = None;
        if let Some(org_id) = evt.org() {
            if let Some(org) = self.org_from_id(*org_id)? {
                if let Some(sn) = org["shortname"].as_str() {
                    destination_loc = Some(sn.to_string());
                }
            }
        }

        let copy = &evt.payload()["copy"];
        if copy.is_object() {
            // If the API returned a copy, collect data about the copy
            // for our response.  It could mean the copy's circ lib
            // changed because it floats.

            log::debug!("{self} Checkin of {} returned a copy object", item.barcode);

            if let Ok(circ_lib) = eg::util::json_int(&copy["circ_lib"]) {
                if circ_lib != item.circ_lib {
                    if let Some(org) = self.org_from_id(circ_lib)? {
                        let loc = org["shortname"].as_str().unwrap();
                        current_loc = loc.to_string();
                        permanent_loc = loc.to_string();
                    }
                }
            }
        }

        let mut result = CheckinResult {
            ok: false,
            current_loc,
            permanent_loc,
            destination_loc,
            patron_barcode: None,
            alert_type: None,
            hold_patron_name: None,
            hold_patron_barcode: None,
        };

        let circ = &evt.payload()["circ"];
        if circ.is_object() {
            log::debug!(
                "{self} Checkin of {} returned a circulation object",
                item.barcode
            );

            if let Some(user) = self.get_user_and_card(eg::util::json_int(&circ["usr"])?)? {
                if let Some(bc) = user["card"]["barcode"].as_str() {
                    result.patron_barcode = Some(bc.to_string());
                }
            }
        }

        self.handle_hold(&evt, &mut result)?;

        if evt.textcode().eq("SUCCESS") || evt.textcode().eq("NO_CHANGE") {
            result.ok = true;
        } else if evt.textcode().eq("ROUTE_ITEM") {
            result.ok = true;
            if result.alert_type.is_none() {
                result.alert_type = Some(AlertType::Transit);
            }
        } else {
            result.ok = false;
            if result.alert_type.is_none() {
                result.alert_type = Some(AlertType::Unknown);
            }
        }

        Ok(result)
    }

    /// Checkoin that runs within the current thread as a direct
    /// Rust call.
    fn checkin_native(
        &mut self,
        item: &item::Item,
        current_loc_op: Option<&str>,
        return_date: &str,
        cancel: bool,
        ovride: bool,
    ) -> EgResult<CheckinResult> {
        let mut options: HashMap<String, json::JsonValue> = HashMap::new();
        options.insert("copy_barcode".to_string(), item.barcode.as_str().into());

        if self.account().settings().checkin_holds_as_transits() {
            options.insert("hold_as_transit".to_string(), json::from(true));
        }

        if cancel {
            options.insert("revert_hold_fulfillment".to_string(), json::from(cancel));
        }

        if return_date.trim().len() == 18 {
            let fmt = sip2::spec::SIP_DATE_FORMAT;

            // Use NaiveDate since SIP dates don't typically include a
            // time zone value.
            if let Some(sip_date) = NaiveDateTime::parse_from_str(return_date, fmt).ok() {
                let iso_date = sip_date.format("%Y-%m-%d").to_string();
                log::info!("{self} Checking in with backdate: {iso_date}");

                options.insert("backdate".to_string(), json::from(iso_date));
            } else {
                log::warn!("{self} Invalid checkin return date: {return_date}");
            }
        }

        if let Some(sn) = current_loc_op {
            if let Some(org) = self.org_from_sn(sn)? {
                options.insert("circ_lib".to_string(), org["id"].clone());
            } else {
                log::warn!("Unknown org unit provided for current location: {sn}");
            }
        }

        if !options.contains_key("circ_lib") {
            options.insert("circ_lib".to_string(), json::from(self.get_ws_org_id()?));
        }

        log::info!("{self} checkin with params: {:?}", options);

        let editor = self.editor().clone();

        let mut circulator = Circulator::new(editor, options)?;
        circulator.is_override = ovride;
        circulator.begin()?;

        // Collect needed data then kickoff the checkin process.
        let result = circulator.checkin();

        log::info!("{self} Checkin of {} returned: {result:?}", item.barcode);

        let err_bind;
        let evt = match result {
            Ok(()) => {
                circulator.commit()?;
                circulator
                    .events()
                    .get(0)
                    .ok_or_else(|| format!("API call failed to return an event"))?
            }
            Err(err) => {
                circulator.rollback()?;
                err_bind = Some(err.event_or_default());
                err_bind.as_ref().unwrap()
            }
        };

        if !ovride
            && self
                .account()
                .settings()
                .checkin_override()
                .contains(&evt.textcode().to_string())
        {
            return self.checkin(item, current_loc_op, return_date, cancel, true);
        }

        let mut current_loc = item.current_loc.to_string(); // item.circ_lib
        let mut permanent_loc = item.permanent_loc.to_string(); // item.circ_lib

        let mut destination_loc = None;
        if let Some(org_id) = evt.org() {
            if let Some(org) = self.org_from_id(*org_id)? {
                if let Some(sn) = org["shortname"].as_str() {
                    destination_loc = Some(sn.to_string());
                }
            }
        }

        let copy = &evt.payload()["copy"];
        if copy.is_object() {
            // If the API returned a copy, collect data about the copy
            // for our response.  It could mean the copy's circ lib
            // changed because it floats.

            log::debug!("{self} Checkin of {} returned a copy object", item.barcode);

            if let Ok(circ_lib) = eg::util::json_int(&copy["circ_lib"]) {
                if circ_lib != item.circ_lib {
                    if let Some(org) = self.org_from_id(circ_lib)? {
                        let loc = org["shortname"].as_str().unwrap();
                        current_loc = loc.to_string();
                        permanent_loc = loc.to_string();
                    }
                }
            }
        }

        let mut result = CheckinResult {
            ok: false,
            current_loc,
            permanent_loc,
            destination_loc,
            patron_barcode: None,
            alert_type: None,
            hold_patron_name: None,
            hold_patron_barcode: None,
        };

        let circ = &evt.payload()["circ"];
        if circ.is_object() {
            log::debug!(
                "{self} Checkin of {} returned a circulation object",
                item.barcode
            );

            if let Some(user) = self.get_user_and_card(eg::util::json_int(&circ["usr"])?)? {
                if let Some(bc) = user["card"]["barcode"].as_str() {
                    result.patron_barcode = Some(bc.to_string());
                }
            }
        }

        self.handle_hold(&evt, &mut result)?;

        if evt.textcode().eq("SUCCESS") || evt.textcode().eq("NO_CHANGE") {
            result.ok = true;
        } else if evt.textcode().eq("ROUTE_ITEM") {
            result.ok = true;
            if result.alert_type.is_none() {
                result.alert_type = Some(AlertType::Transit);
            }
        } else {
            result.ok = false;
            if result.alert_type.is_none() {
                result.alert_type = Some(AlertType::Unknown);
            }
        }

        Ok(result)
    }

    /// See if checkin resulted in a hold capture and collect
    /// related info.
    fn handle_hold(
        &mut self,
        evt: &eg::event::EgEvent,
        result: &mut CheckinResult,
    ) -> EgResult<()> {
        let rh = &evt.payload()["remote_hold"];
        let lh = &evt.payload()["hold"];

        let hold = if rh.is_object() {
            rh
        } else if lh.is_object() {
            lh
        } else {
            return Ok(());
        };

        log::debug!("{self} Checkin returned a hold object id={}", hold["id"]);

        if let Some(user) = self.get_user_and_card(eg::util::json_int(&hold["usr"])?)? {
            result.hold_patron_name = Some(self.format_user_name(&user));
            if let Some(bc) = user["card"]["barcode"].as_str() {
                result.hold_patron_barcode = Some(bc.to_string());
            }
        }

        let pickup_lib_id;
        let pickup_lib = &hold["pickup_lib"];

        // hold pickup lib may or may not be fleshed here.
        if pickup_lib.is_object() {
            result.destination_loc = Some(pickup_lib["shortname"].as_str().unwrap().to_string());
            pickup_lib_id = eg::util::json_int(&pickup_lib["id"])?;
        } else {
            pickup_lib_id = eg::util::json_int(&pickup_lib)?;
            if let Some(org) = self.org_from_id(pickup_lib_id)? {
                if let Some(sn) = org["shortname"].as_str() {
                    result.destination_loc = Some(sn.to_string());
                }
            }
        }

        if pickup_lib_id == self.get_ws_org_id()? {
            result.alert_type = Some(AlertType::LocalHold);
        } else {
            result.alert_type = Some(AlertType::RemoteHold);
        }

        Ok(())
    }
}
