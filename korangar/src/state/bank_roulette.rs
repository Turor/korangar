use korangar_interface::element::StateElement;
use korangar_networking::RoulettePrize;
use rust_state::RustState;

/// Parse a positive zeny or item count from a text box.
pub fn parse_positive_u32(text: &str) -> Option<u32> {
    let value = text.trim().parse::<u32>().ok()?;
    (value > 0).then_some(value)
}

/// Deposit ack and `ZC_BANKING_CHECK`: 0 ok, 1 failed, 2 not enough zeny, 3
/// vault full.
pub fn bank_reason_label(reason: i16) -> &'static str {
    match reason {
        0 => "ok",
        1 => "failed",
        2 => "not enough zeny",
        3 => "vault is full",
        _ => "unknown",
    }
}

/// Withdraw ack: 0 ok, 1 not enough vault zeny, 2 unknown error.
pub fn withdraw_reason_label(reason: i16) -> &'static str {
    match reason {
        0 => "ok",
        1 => "not enough zeny",
        2 => "failed",
        _ => "unknown",
    }
}

pub fn roulette_generate_label(result: u8) -> &'static str {
    match result {
        0 => "advanced",
        1 => "failed",
        2 => "not enough points",
        3 => "losing cell",
        _ => "unknown",
    }
}

/// Claim ack (`ZC_RECV_ROULETTE_ITEM`): 0 received, 1 failed, 2 too many items,
/// 3 overweight.
pub fn roulette_claim_label(result: u8) -> &'static str {
    match result {
        0 => "received",
        1 => "failed",
        2 => "too many items",
        3 => "overweight",
        _ => "unknown",
    }
}

/// Prize granted by the last spin. The claim packet's item id is the next
/// bonus, not this prize. The count doubles when the cell item matches the
/// bonus that was showing before the claim.
pub fn claimed_spin_prize(prizes: &[RoulettePrize], stage: i32, cell: i32, previous_bonus_item_id: u32) -> Option<(u32, u32)> {
    let row = u16::try_from(stage).ok()?;
    let position = u16::try_from(cell).ok()?;
    let prize = prizes.iter().find(|prize| prize.row == row && prize.position == position)?;
    let mut count = u32::from(prize.count);
    if previous_bonus_item_id != 0 && prize.item_id == previous_bonus_item_id {
        count = count.saturating_mul(2);
    }
    Some((prize.item_id, count))
}

fn format_prizes(prizes: &[RoulettePrize]) -> String {
    if prizes.is_empty() {
        return "No prizes.".to_owned();
    }
    let mut rows: Vec<u16> = prizes.iter().map(|prize| prize.row).collect();
    rows.sort_unstable();
    rows.dedup();
    rows.into_iter()
        .map(|row| {
            let mut cells: Vec<&RoulettePrize> = prizes.iter().filter(|prize| prize.row == row).collect();
            cells.sort_by_key(|prize| prize.position);
            let cells = cells
                .into_iter()
                .map(|prize| format!("[{}] {}x{}", prize.position, prize.item_id, prize.count))
                .collect::<Vec<_>>()
                .join(" ");
            format!("row {row}: {cells}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// One wheel row with the active cell marked. `landed` uses a stronger mark
/// than the in-flight spin highlight.
pub fn format_roulette_wheel(prizes: &[RoulettePrize], stage: i32, cell: i32, landed: bool) -> String {
    if prizes.is_empty() {
        return "Prize table not loaded.".to_owned();
    }
    let row = u16::try_from(stage).unwrap_or(0);
    let mut cells: Vec<&RoulettePrize> = prizes.iter().filter(|prize| prize.row == row).collect();
    if cells.is_empty() {
        cells = prizes.iter().filter(|prize| prize.row == prizes[0].row).collect();
    }
    cells.sort_by_key(|prize| prize.position);
    let shown = cells
        .into_iter()
        .map(|prize| {
            let mark = i32::from(prize.position) == cell;
            if mark && landed {
                format!(">>[{}] {}x{}<<", prize.position, prize.item_id, prize.count)
            } else if mark {
                format!("*[{}] {}x{}*", prize.position, prize.item_id, prize.count)
            } else {
                format!("[{}] {}x{}", prize.position, prize.item_id, prize.count)
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("Wheel row {row}: {shown}")
}

/// Duration of the text wheel spin, matching roBrowser CSS (`transition: 3s`).
pub const ROULETTE_SPIN_MS: u32 = 3000;

/// Ease-out cell index for a timed spin. Walks several laps before stopping
/// on `target_cell`.
pub fn spin_highlight_cell(positions: &[u16], target_cell: i32, elapsed_ms: u32, duration_ms: u32) -> (u16, bool) {
    if positions.is_empty() {
        return (0, true);
    }
    let target_pos = positions
        .iter()
        .position(|position| i32::from(*position) == target_cell)
        .unwrap_or(0);
    if elapsed_ms >= duration_ms {
        return (positions[target_pos], true);
    }
    let progress = elapsed_ms as f32 / duration_ms.max(1) as f32;
    // Cubic ease-out over five full laps plus the target index.
    let eased = 1.0 - (1.0 - progress).powi(3);
    let steps = (positions.len() * 5 + target_pos) as f32;
    let index = ((eased * steps).floor() as usize) % positions.len();
    (positions[index], false)
}

fn encode_prize_table(prizes: &[RoulettePrize]) -> String {
    prizes
        .iter()
        .map(|prize| format!("{} {} {} {}", prize.row, prize.position, prize.item_id, prize.count))
        .collect::<Vec<_>>()
        .join("\n")
}

fn decode_prize_table(text: &str) -> Vec<RoulettePrize> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            Some(RoulettePrize {
                row: parts.next()?.parse().ok()?,
                position: parts.next()?.parse().ok()?,
                item_id: parts.next()?.parse().ok()?,
                count: parts.next()?.parse().ok()?,
            })
        })
        .collect()
}

#[derive(RustState, StateElement)]
pub struct BankWindowState {
    pub status: String,
    pub vault_text: String,
    pub pocket_text: String,
    pub history: String,
    pub amount_text: String,
    pub amount_ghost: String,
    #[hidden_element]
    vault: i64,
    #[hidden_element]
    pocket: i32,
}

impl Default for BankWindowState {
    fn default() -> Self {
        Self {
            status: "Open the bank to read the vault.".to_owned(),
            vault_text: "Vault: —".to_owned(),
            pocket_text: "Pocket: —".to_owned(),
            history: "No deposits or withdrawals yet.".to_owned(),
            amount_text: String::new(),
            amount_ghost: "zeny".to_owned(),
            vault: 0,
            pocket: 0,
        }
    }
}

impl BankWindowState {
    pub fn amount(&self) -> Result<i32, &'static str> {
        let value = parse_positive_u32(&self.amount_text).ok_or("Enter a zeny amount.")?;
        i32::try_from(value).map_err(|_| "Amount is too large.")
    }

    pub fn note_open(&mut self) {
        self.status = "Bank is open.".to_owned();
    }

    pub fn note_balance(&mut self, money: i64, reason: i16) {
        self.vault = money;
        self.vault_text = format!("Vault: {money}z");
        self.status = format!("Vault {money} zeny ({})", bank_reason_label(reason));
    }

    pub fn note_deposit(&mut self, reason: i16, money: i64, zeny: i32) {
        self.remember_balances(money, zeny);
        let line = format!("Deposit {} — vault {money}, pocket {zeny}", bank_reason_label(reason));
        self.push_history(&line);
        self.status = line;
    }

    pub fn note_withdraw(&mut self, reason: i16, money: i64, zeny: i32) {
        self.remember_balances(money, zeny);
        let line = format!("Withdraw {} — vault {money}, pocket {zeny}", withdraw_reason_label(reason));
        self.push_history(&line);
        self.status = line;
    }

    pub fn note_closed(&mut self) {
        self.status = "Bank closed.".to_owned();
    }

    /// Add zeny to the amount box. Used by the +100,000 / +1,000,000 /
    /// +10,000,000 buttons.
    pub fn add_amount(&mut self, delta: i32) {
        let current = parse_positive_u32(&self.amount_text).unwrap_or(0);
        let next = current.saturating_add(delta as u32);
        self.amount_text = next.to_string();
    }

    /// Fill the amount box with the pocket zeny from the last bank ack.
    pub fn set_max_pocket(&mut self) -> Result<(), &'static str> {
        if self.pocket > 0 {
            self.amount_text = self.pocket.to_string();
            Ok(())
        } else {
            Err("Pocket zeny is not known yet.")
        }
    }

    fn remember_balances(&mut self, money: i64, zeny: i32) {
        self.vault = money;
        self.pocket = zeny;
        self.vault_text = format!("Vault: {money}z");
        self.pocket_text = format!("Pocket: {zeny}z");
    }

    fn push_history(&mut self, line: &str) {
        const LIMIT: usize = 8;
        let mut rows: Vec<&str> = if self.history.starts_with("No deposits") {
            Vec::new()
        } else {
            self.history.lines().collect()
        };
        rows.push(line);
        if rows.len() > LIMIT {
            rows = rows.split_off(rows.len() - LIMIT);
        }
        self.history = rows.join("\n");
    }
}

#[derive(RustState, StateElement)]
pub struct RouletteWindowState {
    pub status: String,
    pub points: String,
    pub wheel: String,
    pub prizes: String,
    /// `row position item_id count` lines. The claim ack does not repeat the
    /// granted item.
    #[hidden_element]
    prize_table: String,
    #[hidden_element]
    last_stage: i32,
    #[hidden_element]
    last_cell: i32,
    #[hidden_element]
    last_bonus_item_id: u32,
    #[hidden_element]
    has_spin: bool,
    /// Client tick when the current spin animation began. `0` means idle.
    #[hidden_element]
    spin_started_at: u32,
    /// Highlight shown while the wheel is still spinning.
    #[hidden_element]
    spin_cell: i32,
}

impl Default for RouletteWindowState {
    fn default() -> Self {
        Self {
            status: "Open the roulette to load points.".to_owned(),
            points: "Gold 0  Silver 0  Bronze 0".to_owned(),
            wheel: "Prize table not loaded.".to_owned(),
            prizes: String::new(),
            prize_table: String::new(),
            last_stage: 0,
            last_cell: 0,
            last_bonus_item_id: 0,
            has_spin: false,
            spin_started_at: 0,
            spin_cell: 0,
        }
    }
}

pub struct RouletteSpinView {
    pub result: i32,
    pub step: i32,
    pub index: i32,
    pub bonus_item_id: u32,
    pub gold: i32,
    pub silver: i32,
    pub bronze: i32,
}

impl RouletteWindowState {
    fn remember_spin(&mut self, spin: &RouletteSpinView) {
        self.last_stage = spin.step;
        self.last_cell = spin.index;
        self.last_bonus_item_id = spin.bonus_item_id;
        self.has_spin = true;
    }

    fn remember_points(&mut self, spin: &RouletteSpinView) {
        self.points = format!("Gold {}  Silver {}  Bronze {}", spin.gold, spin.silver, spin.bronze);
    }

    fn row_positions(&self) -> Vec<u16> {
        let table = decode_prize_table(&self.prize_table);
        let row = u16::try_from(self.last_stage).unwrap_or(0);
        let mut cells: Vec<u16> = table.iter().filter(|prize| prize.row == row).map(|prize| prize.position).collect();
        if cells.is_empty() {
            cells = table.iter().map(|prize| prize.position).collect();
            cells.sort_unstable();
            cells.dedup();
        } else {
            cells.sort_unstable();
        }
        cells
    }

    fn refresh_wheel(&mut self) {
        let table = decode_prize_table(&self.prize_table);
        let (cell, landed) = if self.spin_started_at != 0 {
            (self.spin_cell, false)
        } else {
            (self.last_cell, self.has_spin)
        };
        self.wheel = format_roulette_wheel(&table, self.last_stage, cell, landed);
    }

    pub fn note_open(&mut self, spin: &RouletteSpinView) {
        self.spin_started_at = 0;
        self.remember_spin(spin);
        self.remember_points(spin);
        self.spin_cell = spin.index;
        self.refresh_wheel();
        self.status = format!(
            "Roulette result {}, stage {}, cell {}, bonus {}.",
            spin.result, spin.step, spin.index, spin.bonus_item_id
        );
    }

    pub fn note_prizes(&mut self, prizes: &[RoulettePrize]) {
        self.prize_table = encode_prize_table(prizes);
        self.prizes = format_prizes(prizes);
        self.refresh_wheel();
    }

    /// Begin a 3s wheel animation that lands on the generate ack cell.
    pub fn note_spin(&mut self, spin: &RouletteSpinView, now_ms: u32) {
        self.remember_spin(spin);
        self.remember_points(spin);
        self.spin_started_at = now_ms.max(1);
        self.spin_cell = self.row_positions().first().copied().map(i32::from).unwrap_or(0);
        self.refresh_wheel();
        self.status = format!(
            "Spinning… {} — stage {}, cell {}, bonus {}.",
            roulette_generate_label(spin.result as u8),
            spin.step,
            spin.index,
            spin.bonus_item_id
        );
    }

    /// Advance or finish the wheel animation. Call once per client tick while
    /// the roulette window may be open.
    pub fn tick_spin(&mut self, now_ms: u32) {
        if self.spin_started_at == 0 {
            return;
        }
        let elapsed = now_ms.saturating_sub(self.spin_started_at);
        let positions = self.row_positions();
        let (cell, done) = spin_highlight_cell(&positions, self.last_cell, elapsed, ROULETTE_SPIN_MS);
        self.spin_cell = i32::from(cell);
        if done {
            self.spin_started_at = 0;
            self.status = format!(
                "Spin landed — stage {}, cell {}, bonus {}.",
                self.last_stage, self.last_cell, self.last_bonus_item_id
            );
        }
        self.refresh_wheel();
    }

    /// `next_bonus_item_id` is `AdditionItemID` on the claim ack. The granted
    /// item is the last spin cell.
    pub fn note_claim(&mut self, result: u8, next_bonus_item_id: u32) {
        let outcome = roulette_claim_label(result);
        let table = decode_prize_table(&self.prize_table);
        let prize = if self.has_spin {
            claimed_spin_prize(&table, self.last_stage, self.last_cell, self.last_bonus_item_id)
        } else {
            None
        };
        self.last_bonus_item_id = next_bonus_item_id;
        self.status = match prize {
            Some((item_id, count)) => {
                format!("Prize item {item_id} x{count} {outcome}. Next bonus {next_bonus_item_id}.")
            }
            None => format!("Prize {outcome}. Next bonus {next_bonus_item_id}."),
        };
    }

    pub fn note_closed(&mut self, result: u8) {
        self.status = format!("Roulette closed (code {result}).");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_reject_zero_and_blank() {
        assert_eq!(parse_positive_u32(" 12 "), Some(12));
        assert_eq!(parse_positive_u32("0"), None);
        assert_eq!(parse_positive_u32(""), None);
        assert!(BankWindowState::default().amount().is_err());
    }

    #[test]
    fn withdraw_reasons_are_not_the_deposit_table() {
        assert_eq!(bank_reason_label(1), "failed");
        assert_eq!(bank_reason_label(2), "not enough zeny");
        assert_eq!(withdraw_reason_label(1), "not enough zeny");
        assert_eq!(withdraw_reason_label(2), "failed");

        let mut bank = BankWindowState::default();
        bank.note_withdraw(1, 0, 100);
        assert!(bank.status.contains("not enough zeny"));
        assert!(!bank.status.contains("failed"));
        bank.note_withdraw(2, 50, 100);
        assert!(bank.status.starts_with("Withdraw failed"));
        bank.note_deposit(1, 50, 100);
        assert!(bank.status.starts_with("Deposit failed"));
        assert!(bank.history.contains("Withdraw failed"));
        assert!(bank.history.contains("Deposit failed"));
        assert!(bank.vault_text.contains("50"));
        assert!(bank.pocket_text.contains("100"));
        bank.add_amount(100_000);
        bank.add_amount(100_000);
        assert_eq!(bank.amount_text, "200000");
        bank.set_max_pocket().unwrap();
        assert_eq!(bank.amount_text, "100");
    }

    #[test]
    fn spin_highlight_walks_then_lands() {
        let positions = vec![0, 1, 2, 3, 4];
        let (start, done) = spin_highlight_cell(&positions, 4, 0, ROULETTE_SPIN_MS);
        assert!(!done);
        assert_eq!(start, 0);
        let (mid, done) = spin_highlight_cell(&positions, 4, ROULETTE_SPIN_MS / 2, ROULETTE_SPIN_MS);
        assert!(!done);
        assert!(positions.contains(&mid));
        let (end, done) = spin_highlight_cell(&positions, 4, ROULETTE_SPIN_MS, ROULETTE_SPIN_MS);
        assert!(done);
        assert_eq!(end, 4);
    }

    #[test]
    fn claim_names_the_spin_cell_and_keeps_result_codes() {
        let mut roulette = RouletteWindowState::default();
        roulette.note_prizes(&[
            RoulettePrize {
                row: 2,
                position: 4,
                item_id: 501,
                count: 3,
            },
            RoulettePrize {
                row: 1,
                position: 0,
                item_id: 909,
                count: 1,
            },
        ]);
        roulette.note_spin(
            &RouletteSpinView {
                result: 0,
                step: 2,
                index: 4,
                bonus_item_id: 501,
                gold: 1,
                silver: 2,
                bronze: 3,
            },
            1_000,
        );
        assert!(roulette.status.contains("Spinning"));
        assert!(roulette.wheel.contains("*[") || roulette.wheel.contains(">>["));
        roulette.tick_spin(1_000 + ROULETTE_SPIN_MS);
        assert!(roulette.wheel.contains(">>[4] 501x3<<"));
        assert!(roulette.status.contains("landed"));
        roulette.note_claim(0, 777);
        assert!(roulette.prizes.contains("row 2:"));
        assert!(roulette.points.contains("Gold 1"));
        assert!(roulette.status.contains("Prize item 501 x6 received"));
        assert!(roulette.status.contains("Next bonus 777"));

        roulette.note_claim(1, 1);
        assert!(roulette.status.contains("failed"));
        roulette.note_claim(2, 1);
        assert!(roulette.status.contains("too many items"));
        roulette.note_claim(3, 1);
        assert!(roulette.status.contains("overweight"));
    }
}
