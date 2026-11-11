//! The people who can sign in to this server (the website's Account → Members).
use serde::Deserialize;
use serde_json::{json, Value};

use crate::client::{enc, Client};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Member {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub perms: Vec<String>,
    #[serde(default, rename = "hideExplicit")]
    pub hide_explicit: bool,
}

/// All the things a member can be allowed to do (web/js/views/account.js PERMS).
pub const ALL_PERMISSIONS: usize = 6;

impl Member {
    /// "Admin", "Member", or "Member · custom permissions", as the website words it.
    pub fn role(&self) -> &'static str {
        if self.admin || self.perms.len() == ALL_PERMISSIONS {
            "Admin"
        } else if self.perms.len() == 2 && self.perms.iter().any(|p| p == "request") && self.perms.iter().any(|p| p == "parties") {
            "Member"
        } else {
            "Member · custom permissions"
        }
    }
}

impl Client {
    pub fn members(&self) -> Result<Vec<Member>> {
        let v: Value = self.get_json("/api/users")?;
        Ok(serde_json::from_value(v["users"].clone()).unwrap_or_default())
    }

    /// Change what a member may do (`{"perms": [..]}`) or their explicit-music setting (`{"hideExplicit": bool}`).
    /// Answers with the member as the server now has them.
    pub fn edit_member(&self, id: &str, body: Value) -> Result<Member> {
        let res = self.request("PUT", &format!("/api/users/{}", enc(id))).send_json(body)?;
        let v: Value = res.into_json().map_err(|e| Error::BadAnswer(e.to_string()))?;
        serde_json::from_value(v["user"].clone()).map_err(|e| Error::BadAnswer(e.to_string()))
    }

    /// Set a new password for a member (they're signed out everywhere).
    pub fn set_member_password(&self, id: &str, password: &str) -> Result<()> {
        self.request("POST", &format!("/api/users/{}/password", enc(id))).send_json(json!({ "password": password }))?;
        Ok(())
    }

    pub fn remove_member(&self, id: &str) -> Result<()> {
        self.request("DELETE", &format!("/api/users/{}", enc(id))).call()?;
        Ok(())
    }

    /// Add a member (they get the basics; permissions are changed afterwards).
    pub fn add_member(&self, name: &str, password: &str) -> Result<()> {
        self.request("POST", "/api/users").send_json(json!({ "name": name, "password": password }))?;
        Ok(())
    }
}
