#[derive(Copy, Clone, Debug, Default)]
pub struct Key {
    is_down: bool,
    was_down: bool,
    is_pressed: bool,
    is_released: bool,
    force_both: bool,
}

impl Key {
    pub fn reset(&mut self) {
        self.update();
        self.is_down = false;
    }

    pub fn set_down(&mut self, is_down: bool) {
        self.is_down = is_down;

        // Key was released before we had a chance to call the update method, so we make
        // sure that we don't drop the key press.
        if !is_down && !self.was_down {
            self.force_both = true;
        }
    }

    /// Record a press for the next update even if this key was already down.
    /// Used by the headless autopilot so scripted keys are not dropped between
    /// frames.
    pub fn arm_press(&mut self) {
        self.is_down = true;
        self.was_down = false;
        self.force_both = false;
    }

    pub fn update(&mut self) {
        self.is_pressed = self.force_both || (self.is_down && !self.was_down);
        self.is_released = self.force_both || (!self.is_down && self.was_down);
        self.was_down = self.is_down;
        self.force_both = false;
    }

    pub fn down(&self) -> bool {
        self.is_down
    }

    pub fn pressed(&self) -> bool {
        self.is_pressed
    }

    pub fn released(&self) -> bool {
        self.is_released
    }
}
