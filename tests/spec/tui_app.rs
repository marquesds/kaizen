// SPDX-License-Identifier: AGPL-3.0-or-later

#[path = "tui_app/driver.rs"]
mod driver;

use driver::TuiDriver;
use quint_connect::*;

#[quint_run(
    spec = "specs/tui-app.qnt",
    max_samples = 20,
    max_steps = 10,
    seed = "0x7"
)]
fn tui_app_run() -> impl Driver {
    TuiDriver::default()
}
