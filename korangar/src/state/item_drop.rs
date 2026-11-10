use korangar_interface::element::StateElement;
use rust_state::RustState;

/// Typed amount for dropping a stack on the ground. Modeled on roBrowser's
/// item-drop InputBox: the field starts at the full stack, and only 1..=max
/// is accepted.
#[derive(Debug, Clone, Default, RustState, StateElement)]
pub struct ItemDropPrompt {
    index: u16,
    max_amount: u16,
    pub amount_text: String,
    pub prompt: String,
    pub amount_ghost: String,
}

impl ItemDropPrompt {
    pub fn begin(&mut self, index: u16, max_amount: u16, name: &str) {
        self.index = index;
        self.max_amount = max_amount;
        self.amount_text = max_amount.to_string();
        self.prompt = format!("Drop {name}. Enter an amount (1-{max_amount}).");
        self.amount_ghost = "amount".to_owned();
    }

    pub fn index(&self) -> u16 {
        self.index
    }

    /// Empty, zero, and non-numeric text are rejected. Leading zeros parse as
    /// the number (`0005` is 5).
    pub fn amount(&self) -> Option<u16> {
        let value = self.amount_text.trim().parse::<u16>().ok()?;
        (1..=self.max_amount).contains(&value).then_some(value)
    }

    pub fn invalid_amount_message(&self) -> String {
        format!("Enter an amount from 1 to {}.", self.max_amount)
    }
}

#[cfg(test)]
mod tests {
    use super::ItemDropPrompt;

    #[test]
    fn amount_accepts_only_the_stack_range() {
        let mut prompt = ItemDropPrompt::default();
        prompt.begin(4, 5, "Red Potion");
        assert_eq!(prompt.index(), 4);
        assert_eq!(prompt.amount_text, "5");
        assert_eq!(prompt.amount(), Some(5));

        prompt.amount_text = "0005".to_owned();
        assert_eq!(prompt.amount(), Some(5));
        prompt.amount_text = " 1 ".to_owned();
        assert_eq!(prompt.amount(), Some(1));

        for text in ["", "0", "6", "x", "-1"] {
            prompt.amount_text = text.to_owned();
            assert_eq!(prompt.amount(), None, "{text}");
        }
        assert_eq!(prompt.invalid_amount_message(), "Enter an amount from 1 to 5.");
    }
}
