use std::cell::{Cell, UnsafeCell};

use rust_state::{Path, PathExt, Selector};

use crate::graphics::Color;
use crate::state::theme::{BasicInfoThemePathExt, InterfaceThemePathExt};
use crate::state::{ClientState, client_theme};
use crate::world::{WeightPressure, format_exp_pair, format_exp_percent, format_weight, format_zeny, weight_pressure};

pub struct ZenyLabel<A> {
    path: A,
    last: Cell<Option<u64>>,
    text: UnsafeCell<String>,
}

impl<A> ZenyLabel<A> {
    pub fn new(path: A) -> Self {
        Self {
            path,
            last: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for ZenyLabel<A>
where
    A: Path<ClientState, u64>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let zeny = *self.path.follow_safe(state);
        if self.last.get() != Some(zeny) {
            unsafe {
                *self.text.get() = format_zeny(zeny);
            }
            self.last.set(Some(zeny));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

pub struct WeightLabel<A, B> {
    current: A,
    maximum: B,
    last: Cell<Option<(u32, u32)>>,
    text: UnsafeCell<String>,
}

impl<A, B> WeightLabel<A, B> {
    pub fn new(current: A, maximum: B) -> Self {
        Self {
            current,
            maximum,
            last: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A, B> Selector<ClientState, String> for WeightLabel<A, B>
where
    A: Path<ClientState, u32>,
    B: Path<ClientState, u32>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let current = *self.current.follow_safe(state);
        let maximum = *self.maximum.follow_safe(state);
        if self.last.get() != Some((current, maximum)) {
            unsafe {
                *self.text.get() = format_weight(current, maximum);
            }
            self.last.set(Some((current, maximum)));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

pub struct WeightColor<A, B> {
    current: A,
    maximum: B,
    color: UnsafeCell<Color>,
}

impl<A, B> WeightColor<A, B> {
    pub fn new(current: A, maximum: B) -> Self {
        Self {
            current,
            maximum,
            color: UnsafeCell::new(Color::WHITE),
        }
    }
}

impl<A, B> Selector<ClientState, Color> for WeightColor<A, B>
where
    A: Path<ClientState, u32>,
    B: Path<ClientState, u32>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a Color> {
        let current = *self.current.follow_safe(state);
        let maximum = *self.maximum.follow_safe(state);
        let theme = client_theme().basic_info();
        let color = match weight_pressure(current, maximum) {
            WeightPressure::Normal => *theme.weight_color().follow_safe(state),
            WeightPressure::Caution => *theme.weight_caution_color().follow_safe(state),
            WeightPressure::Overweight => *theme.weight_over_color().follow_safe(state),
        };
        unsafe {
            *self.color.get() = color;
            Some(&*self.color.get())
        }
    }
}

#[allow(dead_code)]
pub struct ExpPercentLabel<A, B> {
    prefix: &'static str,
    current: A,
    next: B,
    last: Cell<Option<(u64, u64)>>,
    text: UnsafeCell<String>,
}

impl<A, B> ExpPercentLabel<A, B> {
    #[allow(dead_code)]
    pub fn new(prefix: &'static str, current: A, next: B) -> Self {
        Self {
            prefix,
            current,
            next,
            last: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A, B> Selector<ClientState, String> for ExpPercentLabel<A, B>
where
    A: Path<ClientState, u64>,
    B: Path<ClientState, u64>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let current = *self.current.follow_safe(state);
        let next = *self.next.follow_safe(state);
        if self.last.get() != Some((current, next)) {
            unsafe {
                *self.text.get() = format!("{} {}", self.prefix, format_exp_percent(current, next));
            }
            self.last.set(Some((current, next)));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

#[allow(dead_code)]
pub struct ExpTotalLabel<A, B> {
    current: A,
    next: B,
    last: Cell<Option<(u64, u64)>>,
    text: UnsafeCell<String>,
}

impl<A, B> ExpTotalLabel<A, B> {
    #[allow(dead_code)]
    pub fn new(current: A, next: B) -> Self {
        Self {
            current,
            next,
            last: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A, B> Selector<ClientState, String> for ExpTotalLabel<A, B>
where
    A: Path<ClientState, u64>,
    B: Path<ClientState, u64>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let current = *self.current.follow_safe(state);
        let next = *self.next.follow_safe(state);
        if self.last.get() != Some((current, next)) {
            unsafe {
                *self.text.get() = format_exp_pair(current, next);
            }
            self.last.set(Some((current, next)));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}
