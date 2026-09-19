use crate::app::SowApp;

impl SowApp {
    pub(crate) fn poll_portal_intents(&mut self) {
        if let Some(id) = crate::store_portals::poll_pending_invite_lobby() {
            self.ui.app.main_menu_state.pending_join_lobby_id = Some(id);
            self.ui.app.main_menu_state.is_waiting = true;
            if self.net.client.is_some() {
                self.send_join_if_connected(Some(id), false);
            }
        }
        if crate::store_portals::poll_auth_changed() {
            let fallback = self.ui.app.main_menu_state.player_name.clone();
            let identity = crate::store_portals::load_identity(&fallback);
            if let Some(url) = identity.avatar_url {
                self.ui.app.asset_loader.queue_portal_avatar(url);
            }
            if crate::store_portals::should_fetch_cloud_profile() {
                self.fetch_cloud_progress();
            }
        }
    }
}
