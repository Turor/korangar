//! A script whose dialog the player closed early.
//!
//! Hercules has no packet that aborts a script on a `next` page.
//! `CZ_CLOSE_DIALOG` only ends it on a `close` page. On any other page
//! `npc_scriptcont` simply resumes, so the server keeps sending pages.
//! Escape or the title-bar X has to hide the dialog at once, so the client
//! remembers the script and answers what follows without showing it. Pages
//! get `next`, a menu gets cancel (255, which ends the script), inputs get an
//! empty reply, and a `close` page gets `CZ_CLOSE_DIALOG`, which ends it.

use ragnarok_packets::EntityId;

/// A dismissed script that goes quiet this long is forgotten.
pub const DISMISS_TIMEOUT_MS: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptEvent {
    Text,
    Next,
    Close,
    Menu,
    NumberInput,
    TextInput,
    /// Cutins carry no NPC id.
    Cutin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DismissedReply {
    /// Drop the packet without showing it.
    Swallow,
    Next,
    Close,
    MenuCancel,
    NumberCancel,
    TextCancel,
}

#[derive(Debug, Default)]
pub struct DismissedScript {
    active: Option<(EntityId, u32)>,
}

impl DismissedScript {
    pub fn dismiss(&mut self, npc_id: EntityId, now_ms: u32) {
        self.active = Some((npc_id, now_ms));
    }

    #[cfg(test)]
    pub fn is_dismissed(&self, npc_id: EntityId) -> bool {
        self.active.is_some_and(|(active, _)| active == npc_id)
    }

    /// The reply for a script packet, or `None` when the packet belongs to a
    /// script that is shown normally.
    pub fn intercept(&mut self, npc_id: Option<EntityId>, event: ScriptEvent, now_ms: u32) -> Option<DismissedReply> {
        let (active, since) = self.active?;

        if now_ms.wrapping_sub(since) > DISMISS_TIMEOUT_MS {
            self.active = None;
            return None;
        }

        if let Some(npc_id) = npc_id
            && npc_id != active
        {
            // Another NPC started talking: the old script is over.
            self.active = None;
            return None;
        }

        self.active = Some((active, now_ms));

        let reply = match event {
            ScriptEvent::Text | ScriptEvent::Cutin => DismissedReply::Swallow,
            ScriptEvent::Next => DismissedReply::Next,
            ScriptEvent::Close => DismissedReply::Close,
            ScriptEvent::Menu => DismissedReply::MenuCancel,
            ScriptEvent::NumberInput => DismissedReply::NumberCancel,
            ScriptEvent::TextInput => DismissedReply::TextCancel,
        };

        if matches!(reply, DismissedReply::Close | DismissedReply::MenuCancel) {
            // Both end the script on Hercules.
            self.active = None;
        }

        Some(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NPC: EntityId = EntityId(110_000_123);

    #[test]
    fn nothing_is_intercepted_until_a_dialog_is_dismissed() {
        let mut dismissed = DismissedScript::default();
        assert_eq!(dismissed.intercept(Some(NPC), ScriptEvent::Text, 0), None);
        assert_eq!(dismissed.intercept(None, ScriptEvent::Cutin, 0), None);
    }

    #[test]
    fn pages_after_escape_are_answered_without_showing_them() {
        let mut dismissed = DismissedScript::default();
        dismissed.dismiss(NPC, 1_000);

        assert_eq!(
            dismissed.intercept(Some(NPC), ScriptEvent::Text, 1_100),
            Some(DismissedReply::Swallow)
        );
        assert_eq!(
            dismissed.intercept(None, ScriptEvent::Cutin, 1_100),
            Some(DismissedReply::Swallow)
        );
        assert_eq!(
            dismissed.intercept(Some(NPC), ScriptEvent::Next, 1_150),
            Some(DismissedReply::Next)
        );
        assert_eq!(
            dismissed.intercept(Some(NPC), ScriptEvent::NumberInput, 1_200),
            Some(DismissedReply::NumberCancel)
        );
        assert_eq!(
            dismissed.intercept(Some(NPC), ScriptEvent::TextInput, 1_200),
            Some(DismissedReply::TextCancel)
        );
        assert!(dismissed.is_dismissed(NPC));
        assert_eq!(
            dismissed.intercept(Some(NPC), ScriptEvent::Close, 1_300),
            Some(DismissedReply::Close)
        );
        assert!(!dismissed.is_dismissed(NPC), "close ends the script");
        assert_eq!(dismissed.intercept(Some(NPC), ScriptEvent::Text, 1_400), None);
    }

    #[test]
    fn a_menu_is_cancelled_and_ends_the_script() {
        let mut dismissed = DismissedScript::default();
        dismissed.dismiss(NPC, 0);
        assert_eq!(
            dismissed.intercept(Some(NPC), ScriptEvent::Menu, 10),
            Some(DismissedReply::MenuCancel)
        );
        assert_eq!(dismissed.intercept(Some(NPC), ScriptEvent::Menu, 20), None);
    }

    #[test]
    fn another_npc_or_a_quiet_script_is_shown_normally() {
        let mut dismissed = DismissedScript::default();
        dismissed.dismiss(NPC, 0);
        assert_eq!(dismissed.intercept(Some(EntityId(7)), ScriptEvent::Text, 10), None);
        assert!(!dismissed.is_dismissed(NPC));

        dismissed.dismiss(NPC, 0);
        assert_eq!(
            dismissed.intercept(Some(NPC), ScriptEvent::Next, DISMISS_TIMEOUT_MS),
            Some(DismissedReply::Next)
        );
        assert_eq!(
            dismissed.intercept(Some(NPC), ScriptEvent::Next, DISMISS_TIMEOUT_MS * 2 + 1),
            None,
            "timeout restarts on every answered packet"
        );
    }
}
