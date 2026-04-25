// SPDX-License-Identifier: AGPL-3.0-or-later
//! Heuristic bets H1–H14.

mod h1;
mod h10;
mod h11;
mod h12;
mod h13;
mod h14;
mod h15;
mod h2;
mod h3;
mod h4;
mod h5;
mod h6;
mod h7;
mod h8;
mod h9;

use crate::retro::types::{Bet, Inputs};

pub fn all_bets(inputs: &Inputs) -> Vec<Bet> {
    let mut v = Vec::new();
    v.extend(h1::run(inputs));
    v.extend(h2::run(inputs));
    v.extend(h3::run(inputs));
    v.extend(h4::run(inputs));
    v.extend(h5::run(inputs));
    v.extend(h6::run(inputs));
    v.extend(h7::run(inputs));
    v.extend(h8::run(inputs));
    v.extend(h9::run(inputs));
    v.extend(h10::run(inputs));
    v.extend(h11::run(inputs));
    v.extend(h12::run(inputs));
    v.extend(h13::run(inputs));
    v.extend(h14::run(inputs));
    v.extend(h15::run(inputs));
    v
}
