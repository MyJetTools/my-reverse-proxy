use std::{collections::HashMap, sync::Arc};

use crate::{oauth::OAuthContext, settings::OAuthSettings};

/// The live `oauth:` blocks, keyed by the name endpoints refer to them by.
///
/// Unlike the google-auth list this holds running state — the authorization
/// codes currently in flight — so a settings reload keeps the existing context
/// unless the block itself changed. Rebuilding on every reload would invalidate
/// a consent the user is in the middle of giving.
pub struct OAuthCredentialsList {
    items: HashMap<String, Arc<OAuthContext>>,
}

impl OAuthCredentialsList {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    pub fn add_or_update(&mut self, key: String, item: OAuthContext) {
        self.items.insert(key, Arc::new(item));
    }

    pub fn get(&self, key: &str) -> Option<Arc<OAuthContext>> {
        self.items.get(key).cloned()
    }

    /// Whether the block is already loaded with exactly these settings, so the
    /// reload can leave it alone.
    pub fn is_up_to_date(&self, key: &str, settings: &OAuthSettings) -> bool {
        match self.items.get(key) {
            Some(context) => context.settings_are_the_same(settings),
            None => false,
        }
    }
}
