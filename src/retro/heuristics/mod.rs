//! Heuristic bets H1–H8.

mod h1;
mod h2;
mod h3;
mod h4;
mod h5;
mod h6;
mod h7;
mod h8;

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
    v
}
