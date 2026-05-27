// SPDX-License-Identifier: AGPL-3.0-or-later
use kaizen::experiment::types::Classification;
use quint_connect::*;
use serde::Deserialize;

// --- Quint classification type (mirrors the Quint enum) ---

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum SpecClass {
    Control,
    Treatment,
    Excluded,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum SpecFp {
    ControlFp,
    TreatmentFp,
    OtherFp,
}

impl From<SpecClass> for Classification {
    fn from(s: SpecClass) -> Self {
        match s {
            SpecClass::Control => Classification::Control,
            SpecClass::Treatment => Classification::Treatment,
            SpecClass::Excluded => Classification::Excluded,
        }
    }
}

// --- State (mirrors Quint vars manual_tag, git_class, conflict) ---

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct BindingState {
    manual_tag: SpecClass,
    git_class: SpecClass,
    prompt_class: SpecClass,
    conflict: bool,
}

// --- Driver ---

#[derive(Debug)]
struct BindingDriver {
    manual_tag: Classification,
    git_class: Classification,
    prompt_class: Classification,
    conflict: bool,
}

impl Default for BindingDriver {
    fn default() -> Self {
        Self {
            manual_tag: Classification::Excluded,
            git_class: Classification::Excluded,
            prompt_class: Classification::Excluded,
            conflict: false,
        }
    }
}

fn resolve(
    manual: &Classification,
    prompt: &Classification,
    git: &Classification,
) -> Classification {
    if *manual != Classification::Excluded {
        manual.clone()
    } else if *prompt != Classification::Excluded {
        prompt.clone()
    } else {
        git.clone()
    }
}

fn classify_prompt(fp: SpecFp) -> Classification {
    match fp {
        SpecFp::ControlFp => Classification::Control,
        SpecFp::TreatmentFp => Classification::Treatment,
        SpecFp::OtherFp => Classification::Excluded,
    }
}

fn spec_class(c: &Classification) -> SpecClass {
    match c {
        Classification::Control => SpecClass::Control,
        Classification::Treatment => SpecClass::Treatment,
        Classification::Excluded => SpecClass::Excluded,
    }
}

impl State<BindingDriver> for BindingState {
    fn from_driver(d: &BindingDriver) -> Result<Self> {
        Ok(BindingState {
            manual_tag: spec_class(&d.manual_tag),
            git_class: spec_class(&d.git_class),
            prompt_class: spec_class(&d.prompt_class),
            conflict: d.conflict,
        })
    }
}

impl Driver for BindingDriver {
    type State = BindingState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.manual_tag = Classification::Excluded;
                self.git_class  = Classification::Excluded;
                self.prompt_class = Classification::Excluded;
                self.conflict   = false;
            },
            step => {
                self.manual_tag = Classification::Excluded;
                self.git_class  = Classification::Excluded;
                self.prompt_class = Classification::Excluded;
                self.conflict   = false;
            },
            classify_via_git(g: SpecClass) => {
                self.git_class = g.into();
            },
            classify_via_prompt(fp: SpecFp) => {
                self.prompt_class = classify_prompt(fp);
            },
            apply_manual_tag(v: SpecClass) => {
                let incoming: Classification = v.into();
                self.conflict = self.manual_tag != Classification::Excluded
                    && self.manual_tag != incoming;
                if self.manual_tag == Classification::Excluded || self.manual_tag == incoming {
                    self.manual_tag = incoming;
                }
            }
        })
    }
}

// Spot-check: manual always wins over git in the resolved classification.
#[test]
fn manual_beats_git() {
    let d = BindingDriver {
        manual_tag: Classification::Control,
        git_class: Classification::Treatment,
        ..Default::default()
    };
    assert_eq!(
        resolve(&d.manual_tag, &d.prompt_class, &d.git_class),
        Classification::Control
    );
}

// Spot-check: git used when no manual tag.
#[test]
fn git_used_without_manual() {
    let d = BindingDriver {
        git_class: Classification::Treatment,
        ..Default::default()
    };
    assert_eq!(
        resolve(&d.manual_tag, &d.prompt_class, &d.git_class),
        Classification::Treatment
    );
}

#[test]
fn prompt_exact_match_beats_git_without_manual() {
    let d = BindingDriver {
        prompt_class: Classification::Control,
        git_class: Classification::Treatment,
        ..Default::default()
    };
    assert_eq!(
        resolve(&d.manual_tag, &d.prompt_class, &d.git_class),
        Classification::Control
    );
}

#[quint_run(
    spec = "specs/experiment-binding.qnt",
    max_samples = 20,
    max_steps = 6,
    seed = "0x2"
)]
fn experiment_binding_run() -> impl Driver {
    BindingDriver::default()
}
