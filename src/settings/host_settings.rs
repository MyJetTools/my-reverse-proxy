use std::sync::Arc;

use serde::*;

use crate::{http_proxy_pass::AllowedUserList, types::WhiteListedIpList};

use super::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HostSettings {
    pub endpoint: EndpointSettings,
    pub locations: Vec<LocationSettings>,
}

impl HostSettings {
    pub fn get_allowed_users(
        &self,
        allowed_users: &AllowedUsersSettings,
        endpoint_template_settings: Option<&EndpointTemplateSettings>,
    ) -> Result<Option<Arc<AllowedUserList>>, String> {
        let mut result = None;
        if let Some(allowed_user_id) = &self.endpoint.allowed_users {
            if let Some(allowed_users) = allowed_users.get_configuration(allowed_user_id) {
                result = Some(Arc::new(allowed_users));
            }
        }

        for location_settings in &self.locations {
            let mut whitelisted_ip = WhiteListedIpList::new();
            whitelisted_ip.apply(
                self.endpoint
                    .get_white_listed_ip(endpoint_template_settings)
                    .as_deref(),
            );
            whitelisted_ip.apply(location_settings.whitelisted_ip.as_deref());
        }

        return Ok(result);
    }
}
