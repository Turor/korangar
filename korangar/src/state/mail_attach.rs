//! Rodex write-form attachments: items and the fee.
//!
//! Hercules keeps the attached stacks server-side (`sd->rodex.tmp`). The
//! client asks with `0x0A04` / `0x0A06` and only changes its list on the
//! `0x0A05`/`0x0B3F` / `0x0A07` acks, so the window always shows what the
//! server will send. One request is in flight at a time.

use std::sync::Arc;

use korangar_interface::element::StateElement;
use rust_state::RustState;

use crate::graphics::Texture;

/// Most stacks one mail carries (`RODEX_MAX_ITEM`).
pub const MAX_ATTACHMENTS: usize = 5;
/// Zeny Hercules charges per attached stack (`ATTACHITEM_COST`).
pub const FEE_PER_ITEM: i64 = 2500;
/// Attachment weight limit in displayed units (`RODEX_WEIGHT_LIMIT / 10`).
pub const WEIGHT_LIMIT: u16 = 2000;

/// One attached inventory stack.
#[derive(Debug, Clone, Default, RustState, StateElement)]
pub struct MailAttachment {
    /// Stored inventory index (wire index minus 2).
    pub index: u16,
    pub item_id: u32,
    pub count: u16,
    pub name: String,
    /// "x12", drawn at the right of the row.
    pub count_text: String,
    #[hidden_element]
    pub texture: Option<Arc<Texture>>,
}

impl MailAttachment {
    fn set_count(&mut self, count: u16) {
        self.count = count;
        self.count_text = format!("x{count}");
    }
}

/// An inventory stack offered to the form (drag and drop or autopilot).
#[derive(Debug, Clone)]
pub struct AttachCandidate {
    pub index: u16,
    pub item_id: u32,
    pub name: String,
    pub texture: Option<Arc<Texture>>,
    /// Inventory amount; 1 for equipment.
    pub amount: u16,
    pub equipped: bool,
}

/// What to do with a dropped stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachStep {
    /// Send `0x0A04` now.
    Send {
        index: u16,
        count: u16,
    },
    /// Ask how many first (roBrowser's InputBox for stacks above 1).
    AskAmount,
    Rejected(String),
}

#[derive(Debug, Clone, Default)]
pub struct PendingAttach {
    candidate: Option<AttachCandidateSlot>,
    detach_index: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct AttachCandidateSlot {
    index: u16,
    item_id: u32,
    name: String,
    texture: Option<Arc<Texture>>,
}

/// Attachment list, amount prompt and fee line of the write form.
#[derive(Debug, Clone, Default, RustState, StateElement)]
pub struct MailAttachments {
    #[hidden_element]
    pub items: Vec<MailAttachment>,
    /// "Items 1/5, weight 3/2000".
    pub summary: String,
    /// Fee line under the zeny box.
    pub fee_text: String,
    /// Amount prompt for a stack above 1.
    pub amount_prompt: String,
    pub amount_text: String,
    #[hidden_element]
    weight: u16,
    #[hidden_element]
    asking: Option<(AttachCandidateSlot, u16)>,
    #[hidden_element]
    pending: PendingAttach,
}

impl MailAttachments {
    pub fn clear(&mut self) {
        *self = Self::default();
        self.refresh(0);
    }

    pub fn is_busy(&self) -> bool {
        self.pending.candidate.is_some() || self.pending.detach_index.is_some()
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// How many of inventory `index` are already on the form.
    pub fn attached_count(&self, index: u16) -> u16 {
        self.items
            .iter()
            .find(|item| item.index == index)
            .map(|item| item.count)
            .unwrap_or(0)
    }

    /// Zeny Hercules takes on send: the amount, 2500 per stack, and 2% of
    /// the amount (`zeny + items * ATTACHITEM_COST + 2 * zeny / 100`).
    pub fn fee(&self, zeny: i64) -> i64 {
        self.items.len() as i64 * FEE_PER_ITEM + (2 * zeny.max(0)) / 100
    }

    /// Rebuild the summary and fee lines for `zeny`.
    pub fn refresh(&mut self, zeny: i64) {
        self.summary = format!(
            "Items {}/{MAX_ATTACHMENTS}, weight {}/{WEIGHT_LIMIT}",
            self.items.len(),
            self.weight
        );
        let fee = self.fee(zeny);
        self.fee_text = format!("Fee {} z, total {} z", group(fee), group(fee + zeny.max(0)));
    }

    /// A stack dropped on the form. `write_open` is whether the server has
    /// the write form open.
    pub fn offer(&mut self, write_open: bool, candidate: AttachCandidate) -> AttachStep {
        if !write_open {
            return AttachStep::Rejected("Open the write form first.".to_owned());
        }
        if self.is_busy() {
            return AttachStep::Rejected("Wait for the last item change to finish.".to_owned());
        }
        if candidate.equipped {
            return AttachStep::Rejected(format!("Take off {} before attaching it.", candidate.name));
        }
        let already = self.attached_count(candidate.index);
        if already == 0 && self.items.len() >= MAX_ATTACHMENTS {
            return AttachStep::Rejected(format!("A mail holds at most {MAX_ATTACHMENTS} items."));
        }
        let left = candidate.amount.saturating_sub(already);
        if left == 0 {
            return AttachStep::Rejected(format!("All of your {} is already attached.", candidate.name));
        }
        let slot = AttachCandidateSlot {
            index: candidate.index,
            item_id: candidate.item_id,
            name: candidate.name,
            texture: candidate.texture,
        };
        if left == 1 {
            let index = slot.index;
            self.pending.candidate = Some(slot);
            return AttachStep::Send { index, count: 1 };
        }
        self.amount_prompt = format!("Attach how many {}? (1-{left})", slot.name);
        self.amount_text = left.to_string();
        self.asking = Some((slot, left));
        AttachStep::AskAmount
    }

    /// The amount prompt was confirmed. Returns the `0x0A04` to send.
    pub fn confirm_amount(&mut self) -> Result<(u16, u16), String> {
        let Some((slot, max)) = self.asking.as_ref() else {
            return Err("No item is waiting for an amount.".to_owned());
        };
        let max = *max;
        let count = self
            .amount_text
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|count| (1..=max).contains(count))
            .ok_or_else(|| format!("Enter an amount from 1 to {max}."))?;
        let index = slot.index;
        let (slot, _) = self.asking.take().expect("checked above");
        self.pending.candidate = Some(slot);
        self.amount_prompt.clear();
        Ok((index, count))
    }

    pub fn cancel_amount(&mut self) {
        self.asking = None;
        self.amount_prompt.clear();
    }

    pub fn is_asking(&self) -> bool {
        self.asking.is_some()
    }

    /// `0x0A05` / `0x0B3F`. Returns the chat line.
    pub fn attached(&mut self, result: u8, index: Option<u16>, count: u16, item_id: u32, weight: u16) -> Option<String> {
        let slot = self.pending.candidate.take()?;
        if result != 0 {
            return Some(attach_failure(result, &slot.name));
        }
        let index = index.unwrap_or(slot.index);
        self.weight = weight;
        match self.items.iter_mut().find(|item| item.index == index) {
            Some(item) => item.set_count(item.count.saturating_add(count)),
            None => self.items.push(MailAttachment {
                index,
                item_id: if item_id != 0 { item_id } else { slot.item_id },
                count,
                name: slot.name.clone(),
                count_text: format!("x{count}"),
                texture: slot.texture,
            }),
        }
        Some(format!("Attached {} x{count}.", slot.name))
    }

    /// Take a whole stack off. Returns the `0x0A06` to send.
    pub fn detach(&mut self, index: u16) -> Result<(u16, u16), String> {
        if self.is_busy() {
            return Err("Wait for the last item change to finish.".to_owned());
        }
        let count = self.attached_count(index);
        if count == 0 {
            return Err("That item is not attached.".to_owned());
        }
        self.pending.detach_index = Some(index);
        Ok((index, count))
    }

    /// `0x0A07`. Returns the chat line.
    pub fn detached(&mut self, removed: bool, index: Option<u16>, count: u16, weight: u16) -> Option<String> {
        let expected = self.pending.detach_index.take()?;
        if !removed {
            return Some("The item could not be taken off the mail.".to_owned());
        }
        let index = index.unwrap_or(expected);
        self.weight = weight;
        let position = self.items.iter().position(|item| item.index == index)?;
        let name = self.items[position].name.clone();
        let left = self.items[position].count.saturating_sub(count);
        if left == 0 {
            self.items.remove(position);
        } else {
            self.items[position].set_count(left);
        }
        Some(format!("Took {name} x{count} off the mail."))
    }
}

/// `enum rodex_add_item` results.
pub fn attach_failure(result: u8, name: &str) -> String {
    match result {
        1 => format!("{name} is too heavy to mail (the limit is {WEIGHT_LIMIT})."),
        3 => format!("A mail holds at most {MAX_ATTACHMENTS} items."),
        4 => format!("{name} cannot be mailed."),
        _ => format!("{name} could not be attached."),
    }
}

fn group(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (position, digit) in digits.chars().enumerate() {
        if position > 0 && (digits.len() - position).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    if value < 0 { format!("-{out}") } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(index: u16, name: &str, amount: u16) -> AttachCandidate {
        AttachCandidate {
            index,
            item_id: 500 + u32::from(index),
            name: name.to_owned(),
            texture: None,
            amount,
            equipped: false,
        }
    }

    #[test]
    fn single_items_attach_on_the_ack_only() {
        let mut list = MailAttachments::default();
        assert_eq!(
            list.offer(false, stack(3, "Knife", 1)),
            AttachStep::Rejected("Open the write form first.".to_owned())
        );
        assert_eq!(list.offer(true, stack(3, "Knife", 1)), AttachStep::Send { index: 3, count: 1 });
        assert!(list.is_busy());
        assert!(matches!(list.offer(true, stack(4, "Shirt", 1)), AttachStep::Rejected(_)));
        assert_eq!(list.count(), 0, "nothing shows before the ack");
        assert_eq!(list.attached(0, Some(3), 1, 1201, 40).as_deref(), Some("Attached Knife x1."));
        assert_eq!(list.count(), 1);
        assert_eq!(list.items[0].item_id, 1201);
        assert_eq!(list.attached(0, Some(3), 1, 1201, 40), None, "no request pending");
        list.refresh(0);
        assert_eq!(list.summary, "Items 1/5, weight 40/2000");
    }

    #[test]
    fn stacks_ask_for_an_amount_within_what_is_left() {
        let mut list = MailAttachments::default();
        assert_eq!(list.offer(true, stack(7, "Red Potion", 30)), AttachStep::AskAmount);
        assert!(list.amount_prompt.contains("1-30"));
        assert_eq!(list.amount_text, "30");
        list.amount_text = "0".to_owned();
        assert!(list.confirm_amount().is_err());
        list.amount_text = "31".to_owned();
        assert!(list.confirm_amount().is_err());
        list.amount_text = " 12 ".to_owned();
        assert_eq!(list.confirm_amount(), Ok((7, 12)));
        list.attached(0, Some(7), 12, 501, 84);

        // A second drop of the same stack offers only the rest and merges.
        assert_eq!(list.offer(true, stack(7, "Red Potion", 30)), AttachStep::AskAmount);
        assert!(list.amount_prompt.contains("1-18"));
        list.amount_text = "18".to_owned();
        assert_eq!(list.confirm_amount(), Ok((7, 18)));
        list.attached(0, Some(7), 18, 501, 210);
        assert_eq!(list.count(), 1);
        assert_eq!(list.items[0].count, 30);
        assert_eq!(list.items[0].count_text, "x30");
        assert!(matches!(list.offer(true, stack(7, "Red Potion", 30)), AttachStep::Rejected(message) if message.contains("already")));

        assert_eq!(list.offer(true, stack(8, "Jellopy", 5)), AttachStep::AskAmount);
        list.cancel_amount();
        assert!(!list.is_asking());
        assert!(list.confirm_amount().is_err());
    }

    #[test]
    fn server_refusals_explain_themselves() {
        let mut list = MailAttachments::default();
        list.offer(true, stack(2, "Free Ticket", 1));
        assert_eq!(
            list.attached(4, None, 0, 0, 0).as_deref(),
            Some("Free Ticket cannot be mailed.")
        );
        assert_eq!(list.count(), 0);
        assert!(!list.is_busy());
        assert!(attach_failure(1, "Anvil").contains("too heavy"));
        assert!(attach_failure(3, "x").contains("at most 5"));
        assert!(attach_failure(2, "x").contains("could not"));

        let mut equipped = stack(1, "Knife", 1);
        equipped.equipped = true;
        assert!(matches!(list.offer(true, equipped), AttachStep::Rejected(message) if message.contains("Take off")));
    }

    #[test]
    fn five_stacks_fill_a_mail() {
        let mut list = MailAttachments::default();
        for index in 0..5 {
            assert_eq!(list.offer(true, stack(index, "Item", 1)), AttachStep::Send { index, count: 1 });
            list.attached(0, Some(index), 1, 0, 1);
        }
        assert_eq!(list.items[4].item_id, 504, "a zero item id keeps the dropped one");
        assert!(matches!(list.offer(true, stack(9, "Item", 1)), AttachStep::Rejected(message) if message.contains("at most 5")));
    }

    #[test]
    fn detaching_takes_the_whole_stack_off_on_the_ack() {
        let mut list = MailAttachments::default();
        list.offer(true, stack(7, "Red Potion", 1));
        list.attached(0, Some(7), 1, 501, 7);
        assert!(list.detach(9).is_err());
        assert_eq!(list.detach(7), Ok((7, 1)));
        assert!(list.detach(7).is_err(), "one change at a time");
        assert_eq!(list.count(), 1, "still attached until the ack");
        assert_eq!(
            list.detached(true, Some(7), 1, 0).as_deref(),
            Some("Took Red Potion x1 off the mail.")
        );
        assert_eq!(list.count(), 0);
        assert_eq!(list.detached(true, Some(7), 1, 0), None);

        list.offer(true, stack(7, "Red Potion", 1));
        list.attached(0, Some(7), 1, 501, 7);
        list.detach(7).unwrap();
        assert!(list.detached(false, None, 0, 7).unwrap().contains("could not"));
        assert_eq!(list.count(), 1);
    }

    #[test]
    fn the_fee_matches_hercules() {
        let mut list = MailAttachments::default();
        assert_eq!(list.fee(1000), 20);
        assert_eq!(list.fee(49), 0, "2 * 49 / 100 rounds down");
        list.offer(true, stack(1, "Knife", 1));
        list.attached(0, Some(1), 1, 1201, 40);
        list.offer(true, stack(2, "Shirt", 1));
        list.attached(0, Some(2), 1, 2301, 10);
        assert_eq!(list.fee(150_000), 5000 + 3000);
        list.refresh(150_000);
        assert_eq!(list.fee_text, "Fee 8,000 z, total 158,000 z");
        assert_eq!(group(1_234_567), "1,234,567");
        assert_eq!(group(999), "999");
    }
}
