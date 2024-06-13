use std::collections::HashMap;

use serde::*;

use crate::http_proxy_pass::AllowedUserList;

use super::FileSource;

#[derive(Debug, Clone)]
pub struct AllowedUsersSettingsModel {
    pub data: Option<HashMap<String, Vec<String>>>,
}

impl AllowedUsersSettingsModel {
    pub fn new(data: Option<HashMap<String, Vec<String>>>) -> Self {
        AllowedUsersSettingsModel { data }
    }
    pub async fn populate_from_file(&mut self, file: FileSource) -> Result<(), String> {
        let file_content = file.load_file_content().await;

        let allowed_users: Result<AllowedUsersRemoteYamlModel, _> =
            serde_yaml::from_slice(file_content.as_slice());

        let result = match allowed_users {
            Ok(result) => result,
            Err(err) => {
                return Err(format!(
                    "Error parsing allowed users remote file: {:?}, error: {:?}",
                    file.as_str().as_str(),
                    err
                ));
            }
        };

        if let Some(allowed_users) = result.allowed_users {
            for (key, value) in allowed_users {
                self.data
                    .get_or_insert_with(|| HashMap::new())
                    .insert(key, value);
            }
        }

        Ok(())
    }

    pub fn get_configuration(&self, name: &str) -> Option<AllowedUserList> {
        let result = self.data.as_ref()?;
        let result = result.get(name)?;
        Some(AllowedUserList::new(result.clone()))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AllowedUsersRemoteYamlModel {
    allowed_users: Option<HashMap<String, Vec<String>>>,
}
