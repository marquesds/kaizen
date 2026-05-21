// SPDX-License-Identifier: AGPL-3.0-or-later

mod bin_kaizen;

fn main() -> anyhow::Result<()> {
    bin_kaizen::run()
}
