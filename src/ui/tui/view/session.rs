// SPDX-License-Identifier: AGPL-3.0-or-later
use super::{PAGE_FLOOR, PREFETCH};
use crate::core::event::SessionRecord;
use std::collections::{BTreeMap, HashSet};

const MAX_SESSION_PAGES: usize = 5;

pub struct SessionView {
    pub page_size: usize,
    pub cursor: usize,
    pub window: BTreeMap<usize, SessionRecord>,
    pub total: usize,
    generation: u64,
    in_flight: HashSet<usize>,
    loaded_once: bool,
}

impl SessionView {
    pub fn new() -> Self {
        Self {
            page_size: PAGE_FLOOR,
            cursor: 0,
            window: BTreeMap::new(),
            total: 0,
            generation: 0,
            in_flight: HashSet::new(),
            loaded_once: false,
        }
    }

    pub fn reset(&mut self) {
        self.cursor = 0;
        self.total = 0;
        self.window.clear();
        self.in_flight.clear();
        self.loaded_once = false;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn set_viewport_height(&mut self, height: usize) {
        self.page_size = (height + PREFETCH * 2).max(PAGE_FLOOR);
    }

    pub fn request_page(&mut self, offset: usize) -> bool {
        self.in_flight.insert(offset)
    }

    pub fn finish_page(&mut self, offset: usize, rows: Vec<SessionRecord>, total: usize) {
        self.in_flight.remove(&offset);
        self.total = total;
        self.loaded_once = true;
        self.cursor = self.cursor.min(self.total.saturating_sub(1));
        rows.into_iter().enumerate().for_each(|(i, row)| {
            self.window.insert(offset + i, row);
        });
        self.evict_far_pages();
    }

    pub fn finish_error(&mut self, offset: usize) {
        self.in_flight.remove(&offset);
    }

    pub fn selected(&self) -> Option<&SessionRecord> {
        self.window.get(&self.cursor)
    }

    pub fn visible_rows(&self, height: usize) -> Vec<(usize, Option<&SessionRecord>)> {
        let start = self.viewport_start(height);
        let end = start.saturating_add(height).min(self.total.max(start));
        (start..end).map(|i| (i, self.window.get(&i))).collect()
    }

    pub fn selected_local_index(&self, height: usize) -> Option<usize> {
        (self.total > 0).then(|| self.cursor.saturating_sub(self.viewport_start(height)))
    }

    pub fn move_by(&mut self, delta: isize) {
        let next = self.cursor.saturating_add_signed(delta);
        self.cursor = next.min(self.total.saturating_sub(1));
    }

    pub fn jump_last(&mut self) {
        self.cursor = self.total.saturating_sub(1);
    }

    pub fn needed_page_offsets(&self, height: usize) -> Vec<usize> {
        if self.total == 0 && self.loaded_once {
            return Vec::new();
        }
        let start = self.viewport_start(height).saturating_sub(PREFETCH);
        let end = self
            .viewport_start(height)
            .saturating_add(height)
            .saturating_add(PREFETCH)
            .min(self.total.max(self.page_size));
        self.missing_offsets(start, end)
    }

    fn missing_offsets(&self, start: usize, end: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut offset = self.page_start_for(start);
        while offset < end {
            if !self.window.contains_key(&offset) && !self.in_flight.contains(&offset) {
                out.push(offset);
            }
            offset = offset.saturating_add(self.page_size);
        }
        out
    }

    fn page_start_for(&self, index: usize) -> usize {
        index / self.page_size * self.page_size
    }

    fn viewport_start(&self, height: usize) -> usize {
        (self.cursor + 1).saturating_sub(height)
    }

    fn evict_far_pages(&mut self) {
        let current = self.page_start_for(self.cursor);
        let keep = self.page_size * MAX_SESSION_PAGES;
        self.window.retain(|idx, _| idx.abs_diff(current) <= keep);
    }
}
