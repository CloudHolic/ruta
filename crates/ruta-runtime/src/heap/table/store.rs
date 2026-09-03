//! How a table holds what it holds: the array part, the node part, and how big each gets.

use std::mem;

use crate::value::Value;

/// A table's two parts.
#[derive(Debug, Default)]
pub(in crate::heap) struct Table {
    array: Vec<Value>,
    nodes: Vec<Node>,
    /// Where the walk for an unused node stopped last time.
    free: usize,
}

/// Where a walk resumes.
#[derive(Debug, Clone, Copy)]
pub(super) enum Resume {
    /// From this index of the array part.
    Array(usize),
    /// From this node.
    Node(usize),
}

/// One entry of the hash part.
#[derive(Debug, Clone, Copy, Default)]
struct Node {
    key: Value,
    value: Value,
    /// The reference stores an offset because its node array moves when it grows.
    next: Option<u32>,
}

impl Table {
    pub(in crate::heap) fn with_hints(array_hint: usize, hash_hint: usize) -> Self {
        Self {
            array: vec![Value::Nil; array_hint],
            nodes: vec![Node::default(); nodes_for(hash_hint)],
            free: 0,
        }
    }

    pub(super) fn array_len(&self) -> usize {
        self.array.len()
    }

    /// Whether an integer key falls inside the array part.
    pub(super) fn in_array(&self, key: i64) -> bool {
        key >= 1 && key as u128 <= self.array.len() as u128
    }

    /// The first live entry at or after a resume point.
    pub(super) fn entry_from(&self, resume: Resume) -> Option<(Value, Value)> {
        let node_start = match resume {
            Resume::Array(start) => {
                for index in start..self.array.len() {
                    if !matches!(self.array[index], Value::Nil) {
                        return Some((Value::Int(index as i64 + 1), self.array[index]));
                    }
                }

                0
            }

            Resume::Node(start) => start,
        };

        for index in node_start..self.nodes.len() {
            if !matches!(self.nodes[index].value, Value::Nil) {
                return Some((self.nodes[index].key, self.nodes[index].value));
            }
        }

        None
    }

    /// A border inside the array part, if the array part ends on an empty slot.
    pub(super) fn array_border(&self) -> Option<i64> {
        if self.array.is_empty() || !matches!(self.array[self.array.len() - 1], Value::Nil) {
            return None;
        }

        let mut low = 0;
        let mut high = self.array.len();

        while high - low > 1 {
            let middle = low + (high - low) / 2;

            if matches!(self.array[middle - 1], Value::Nil) {
                high = middle;
            } else {
                low = middle;
            }
        }

        Some(low as i64)
    }

    pub(super) fn array_get(&self, key: Value) -> Option<Value> {
        let index = self.array_index(key)?;

        Some(self.array[index])
    }

    pub(super) fn array_set(&mut self, key: Value, value: Value) -> bool {
        match self.array_index(key) {
            Some(index) => {
                self.array[index] = value;
                true
            }
            None => false,
        }
    }

    /// The node a hash belongs in, if there is a hash part at all.
    pub(super) fn main_position(&self, hash: u32) -> Option<usize> {
        if self.nodes.is_empty() {
            return None;
        }

        Some(hash as usize & (self.nodes.len() - 1))
    }

    pub(super) fn node_key(&self, index: usize) -> Value {
        self.nodes[index].key
    }

    pub(super) fn node_value(&self, index: usize) -> Value {
        self.nodes[index].value
    }

    pub(super) fn node_next(&self, index: usize) -> Option<usize> {
        self.nodes[index].next.map(|next| next as usize)
    }

    pub(super) fn node_is_free(&self, index: usize) -> bool {
        matches!(self.nodes[index].key, Value::Nil)
    }

    pub(super) fn set_node_value(&mut self, index: usize, value: Value) {
        self.nodes[index].value = value;
    }

    pub(super) fn set_node_next(&mut self, index: usize, next: Option<usize>) {
        self.nodes[index].next = next.map(|next| next as u32);
    }

    pub(super) fn put_node(&mut self, index: usize, key: Value, value: Value, next: Option<usize>) {
        self.nodes[index] = Node {
            key,
            value,
            next: next.map(|next| next as u32),
        };
    }

    /// An unused node, if the hash part still has one.
    pub(super) fn take_free(&mut self) -> Option<usize> {
        while self.free < self.nodes.len() {
            let index = self.nodes.len() - 1 - self.free;
            self.free += 1;

            if matches!(self.nodes[index].key, Value::Nil) {
                return Some(index);
            }
        }

        None
    }

    /// Everything the table holds, emptied out.
    pub(super) fn drain_all(&mut self) -> Vec<(Value, Value)> {
        let array = mem::take(&mut self.array);
        let nodes = mem::take(&mut self.nodes);
        self.free = 0;

        let from_array = array
            .into_iter()
            .enumerate()
            .filter(|(_, value)| !matches!(value, Value::Nil))
            .map(|(index, value)| (Value::Int(index as i64 + 1), value));

        let from_nodes = nodes
            .into_iter()
            .filter(|node| !matches!(node.value, Value::Nil))
            .map(|node| (node.key, node.value));

        from_array.chain(from_nodes).collect()
    }

    /// Lays out both parts of a known number of entries.
    pub(super) fn rebuild(&mut self, array_size: usize, hash_count: usize) {
        self.array = vec![Value::Nil; array_size];
        self.nodes = vec![Node::default(); nodes_for(hash_count)];
        self.free = 0;
    }

    fn array_index(&self, key: Value) -> Option<usize> {
        let Value::Int(number) = key else {
            return None;
        };

        if number < 1 || number as u128 > self.array.len() as u128 {
            return None;
        }

        Some(number as usize - 1)
    }
}

/// Which power-of-two slice an integer key falls in.
pub(super) fn bucket_of(key: i64) -> Option<usize> {
    let key = u64::try_from(key).ok()?;

    if key == 0 {
        return None;
    }

    let power = key.ilog2() as usize;

    Some(if key.is_power_of_two() {
        power
    } else {
        power + 1
    })
}

/// The array size that takes the most integer keys while still paying for itself, and how many keys that is.
pub(super) fn best_array_size(counts: &[usize], integers: usize) -> (usize, usize) {
    let mut covered = 0;
    let mut best = (0, 0);

    for (power, count) in counts.iter().enumerate() {
        if covered == integers {
            break;
        }

        covered += count;

        if *count > 0 && array_is_worth_it(1usize << power, covered) {
            best = (1usize << power, covered);
        }
    }

    best
}

/// The node count that holds `wanted` keys.
fn nodes_for(wanted: usize) -> usize {
    if wanted == 0 {
        return 0;
    }

    (wanted + 1).next_power_of_two()
}

/// Whether `slots` array entries cost no more than the `nodes` hash entries they replace.
fn array_is_worth_it(slots: usize, nodes: usize) -> bool {
    slots.saturating_mul(size_of::<Value>()) <= nodes.saturating_mul(size_of::<Node>())
}
