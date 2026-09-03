//! Reading and writing a table.

use crate::value::Value;

use super::super::arena::Heap;
use super::super::handle::TableRef;
use super::key::{InvalidKey, KeyError, normalize};
use super::store::{Resume, Table, best_array_size, bucket_of};

impl Heap {
    /// The value at a key, or `nil`.
    pub fn table_get(&self, handle: TableRef, key: Value) -> Value {
        let Ok(key) = normalize(key) else {
            return Value::Nil;
        };

        let body = self.table(handle);

        if let Some(value) = body.array_get(key) {
            return value;
        }

        match self.find_node(body, key) {
            Some(index) => body.node_value(index),
            None => Value::Nil,
        }
    }

    /// Puts a value at a key. Assigning `nil` erases it.
    pub fn table_set(
        &mut self,
        handle: TableRef,
        key: Value,
        value: Value,
    ) -> Result<(), KeyError> {
        let key = normalize(key)?;

        self.barrier(handle);

        let mut body = self.take_table(handle);
        self.set_in(&mut body, key, value);
        self.put_table(handle, body);

        Ok(())
    }

    /// The entry after a key, or `None` at the end.
    pub fn table_next(
        &self,
        handle: TableRef,
        key: Value,
    ) -> Result<Option<(Value, Value)>, InvalidKey> {
        let body = self.table(handle);

        let resume = match key {
            Value::Nil => Resume::Array(0),
            Value::Int(number) if body.in_array(number) => Resume::Array(number as usize),
            other => match self.find_node(body, other) {
                Some(index) => Resume::Node(index + 1),
                None => return Err(InvalidKey),
            },
        };

        Ok(body.entry_from(resume))
    }

    /// A border of the table.
    pub fn table_length(&self, handle: TableRef) -> i64 {
        let body = self.table(handle);

        if let Some(border) = body.array_border() {
            return border;
        }

        let mut filled = body.array_len() as i64;

        if !self.has_integer(body, filled + 1) {
            return filled;
        }

        // Double until the far end is empty, then halve the gap.
        let mut empty = filled + 1;

        while self.has_integer(body, empty) {
            filled = empty;

            let Some(doubled) = empty.checked_mul(2) else {
                while self.has_integer(body, filled + 1) {
                    filled += 1;
                }

                return filled;
            };

            empty = doubled;
        }

        while empty - filled > 1 {
            let middle = filled + (empty - filled) / 2;

            if self.has_integer(body, middle) {
                filled = middle;
            } else {
                empty = middle;
            }
        }

        filled
    }

    fn has_integer(&self, body: &Table, key: i64) -> bool {
        if body.in_array(key) {
            return !matches!(body.array_get(Value::Int(key)), Some(Value::Nil) | None);
        }

        match self.find_node(body, Value::Int(key)) {
            Some(index) => !matches!(body.node_value(index), Value::Nil),
            None => false,
        }
    }

    fn find_node(&self, body: &Table, key: Value) -> Option<usize> {
        let mut index = body.main_position(self.key_hash(key))?;

        loop {
            if self.keys_equal(body.node_key(index), key) {
                return Some(index);
            }

            index = body.node_next(index)?;
        }
    }

    /// Writes into a body that is currently out of the heap.
    fn set_in(&self, body: &mut Table, key: Value, value: Value) {
        if body.array_set(key, value) {
            return;
        }

        if let Some(index) = self.find_node(body, key) {
            body.set_node_value(index, value);
            return;
        }

        // Erasing a key the table never had must not make a node for it.
        if matches!(value, Value::Nil) {
            return;
        }

        self.insert_new(body, key, value);
    }

    fn insert_new(&self, body: &mut Table, key: Value, value: Value) {
        let hash = self.key_hash(key);

        let Some(main) = body.main_position(hash) else {
            return self.refresh(body, key, value);
        };

        if body.node_is_free(main) {
            body.put_node(main, key, value, None);
            return;
        }

        let Some(free) = body.take_free() else {
            return self.refresh(body, key, value);
        };

        let squatter = body.node_key(main);
        let squatter_main = body
            .main_position(self.key_hash(squatter))
            .expect("the table has nodes");

        if squatter_main == main {
            // It belongs here; the new key chains after it.
            body.put_node(free, key, value, body.node_next(main));
            body.set_node_next(main, Some(free));
        } else {
            // It is noly passing through. Move it aside and tkae its place.
            let mut previous = squatter_main;
            while body.node_next(previous) != Some(main) {
                previous = body.node_next(previous).expect("main is on this chain");
            }

            body.set_node_next(previous, Some(free));
            body.put_node(free, squatter, body.node_value(main), body.node_next(main));
            body.put_node(main, key, value, None);
        }
    }

    /// Rebuilds both parts around everything the table holds plus the key that did not fit.
    fn refresh(&self, body: &mut Table, key: Value, value: Value) {
        let mut entries = body.drain_all();
        entries.push((key, value));

        let mut counts = [0usize; usize::BITS as usize];
        let mut integers = 0;

        for (entry_key, _) in &entries {
            if let Value::Int(number) = entry_key
                && let Some(bucket) = bucket_of(*number)
            {
                counts[bucket] += 1;
                integers += 1;
            }
        }

        let (array_size, in_array) = best_array_size(&counts, integers);
        body.rebuild(array_size, entries.len() - in_array);

        for (entry_key, entry_value) in entries {
            if !body.array_set(entry_key, entry_value) {
                self.insert_new(body, entry_key, entry_value);
            }
        }
    }
}
