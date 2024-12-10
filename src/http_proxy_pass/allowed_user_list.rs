pub struct AllowedUserList {
    users: Vec<String>,
}

impl AllowedUserList {
    pub fn new<'s>(users: Vec<String>) -> Self {
        Self { users }
    }

    pub fn is_allowed(&self, user: &str) -> bool {
        self.users.contains(&user.to_string())
    }

    pub fn get_list(&self) -> &[String] {
        &self.users
    }

    pub fn is_empty(&self) -> bool {
        self.users.is_empty()
    }
}
