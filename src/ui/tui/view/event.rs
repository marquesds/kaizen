// SPDX-License-Identifier: AGPL-3.0-or-later
use super::{PAGE_FLOOR, PREFETCH};
use crate::core::event::Event;
use std::collections::{BTreeMap, HashSet};

const MAX_EVENT_PAGES: usize = 5;

pub struct EventView {
    pub page_size: usize,
    pub cursor: usize,
    pub window: BTreeMap<usize, Event>,
    pub total_loaded: usize,
    pub has_more: bool,
    session_id: Option<String>,
    generation: u64,
    in_flight: HashSet<u64>,
    loaded_once: bool,
}

impl EventView {
    pub fn new() -> Self {
        Self {
            page_size: PAGE_FLOOR,
            cursor: 0,
            window: BTreeMap::new(),
            total_loaded: 0,
            has_more: false,
            session_id: None,
            generation: 0,
            in_flight: HashSet::new(),
            loaded_once: false,
        }
    }

    pub fn reset_for(&mut self, session_id: &str) {
        if self.session_id.as_deref() == Some(session_id) {
            return;
        }
        self.clear();
        self.session_id = Some(session_id.to_string());
    }

    pub fn clear(&mut self) {
        self.session_id = None;
        self.window.clear();
        self.in_flight.clear();
        self.loaded_once = false;
        self.cursor = 0;
        self.total_loaded = 0;
        self.has_more = false;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub fn set_viewport_height(&mut self, height: usize) {
        self.page_size = (height + PREFETCH * 2).max(PAGE_FLOOR);
    }

    pub fn request_page(&mut self, after_seq: u64) -> bool {
        self.in_flight.insert(after_seq)
    }

    pub fn finish_page(&mut self, after_seq: u64, rows: Vec<Event>) {
        self.in_flight.remove(&after_seq);
        self.loaded_once = true;
        self.has_more = rows.len() == self.page_size;
        rows.into_iter().for_each(|row| {
            let idx = row.seq as usize;
            self.total_loaded = self.total_loaded.max(idx + 1);
            self.window.insert(idx, row);
        });
        self.cursor = self.cursor.min(self.visible_total().saturating_sub(1));
        self.evict_far_pages();
    }

    pub fn finish_error(&mut self, after_seq: u64) {
        self.in_flight.remove(&after_seq);
    }

    pub fn selected(&self) -> Option<&Event> {
        self.window.get(&self.cursor)
    }

    pub fn visible_rows(&self, height: usize) -> Vec<(usize, Option<&Event>)> {
        let start = self.viewport_start(height);
        let end = start
            .saturating_add(height)
            .min(self.visible_total().max(start));
        (start..end).map(|i| (i, self.window.get(&i))).collect()
    }

    pub fn selected_local_index(&self, height: usize) -> Option<usize> {
        (self.visible_total() > 0).then(|| self.cursor.saturating_sub(self.viewport_start(height)))
    }

    pub fn move_by(&mut self, delta: isize) {
        let next = self.cursor.saturating_add_signed(delta);
        self.cursor = next.min(self.visible_total().saturating_sub(1));
    }

    pub fn jump_last_loaded(&mut self) {
        self.cursor = self.visible_total().saturating_sub(1);
    }

    pub fn needed_after_seq(&self, height: usize) -> Vec<u64> {
        if self.no_more_rows() {
            return Vec::new();
        }
        let start = self.viewport_start(height);
        let end = start.saturating_add(height).saturating_add(PREFETCH);
        self.missing_pages(start, end)
    }

    fn missing_pages(&self, start: usize, end: usize) -> Vec<u64> {
        let mut out = Vec::new();
        let mut seq = (start / self.page_size * self.page_size) as u64;
        while (seq as usize) < end {
            if self.loaded_once && !self.has_more && seq as usize >= self.total_loaded {
                break;
            }
            if !self.window.contains_key(&(seq as usize)) && !self.in_flight.contains(&seq) {
                out.push(seq);
            }
            seq = seq.saturating_add(self.page_size as u64);
        }
        out
    }

    fn no_more_rows(&self) -> bool {
        self.session_id.is_none() || (self.total_loaded == 0 && self.loaded_once && !self.has_more)
    }

    fn visible_total(&self) -> usize {
        self.total_loaded + usize::from(self.has_more)
    }

    fn viewport_start(&self, height: usize) -> usize {
        (self.cursor + 1).saturating_sub(height)
    }

    fn evict_far_pages(&mut self) {
        let current = self.cursor / self.page_size * self.page_size;
        let keep = self.page_size * MAX_EVENT_PAGES;
        self.window.retain(|idx, _| idx.abs_diff(current) <= keep);
    }
}
