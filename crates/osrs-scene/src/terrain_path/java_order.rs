//! Iteration of an insertion-only Java HashMap<Integer, _>, for the fitter's packed tile keys.
//! Values and lookup stay in the fitter. This models bucket chains, resize splits and red/black
//! tree bins because their traversal order affects in-place refinement. There are no removals or
//! duplicate keys; spreading an Integer hash is bijective, so tree hash comparisons never tie.
mod tree;

#[derive(Default, Clone)]
struct Bucket {
    chain: Vec<usize>,
    root: Option<usize>,
}

#[derive(Default)]
struct Node {
    hash: i32,
    parent: Option<usize>,
    children: [Option<usize>; 2],
    red: bool,
}

#[derive(Default)]
struct Order {
    buckets: Vec<Bucket>,
    nodes: Vec<Node>,
}

impl Order {
    fn insert(&mut self, key: u32) {
        if self.buckets.is_empty() {
            self.buckets.resize(16, Bucket::default());
        }
        let hash = (key ^ (key >> 16)) as i32;
        let at = hash as usize & (self.buckets.len() - 1);
        let index = self.nodes.len();
        self.nodes.push(Node {
            hash,
            ..Node::default()
        });
        if let Some(root) = self.buckets[at].root {
            let parent = self.insert_tree(root, index);
            let chain = &mut self.buckets[at].chain;
            let position = chain.iter().position(|n| *n == parent).unwrap_or(0);
            chain.insert(position + 1, index);
            let root = self.balance(root, index);
            self.front(at, root);
        } else {
            self.buckets[at].chain.push(index);
            if self.buckets[at].chain.len() >= 9 {
                if self.buckets.len() < 64 {
                    self.resize();
                } else {
                    self.treeify(at);
                }
            }
        }
        if self.nodes.len() > self.buckets.len() * 3 / 4 {
            self.resize();
        }
    }

    fn resize(&mut self) {
        let old = std::mem::take(&mut self.buckets);
        let size = old.len();
        self.buckets.resize(size * 2, Bucket::default());
        for (at, bucket) in old.into_iter().enumerate() {
            for &node in &bucket.chain {
                let target = if self.nodes[node].hash as usize & size == 0 {
                    at
                } else {
                    at + size
                };
                self.buckets[target].chain.push(node);
            }
            if bucket.root.is_none() {
                continue;
            }
            let split =
                !self.buckets[at].chain.is_empty() && !self.buckets[at + size].chain.is_empty();
            for target in [at, at + size] {
                if self.buckets[target].chain.len() <= 6 {
                    continue; // Java untreeifies this partition; chain order is retained.
                }
                if split {
                    self.treeify(target);
                } else {
                    self.buckets[target].root = bucket.root;
                }
            }
        }
    }

    fn treeify(&mut self, at: usize) {
        let chain = self.buckets[at].chain.clone();
        let mut root = None;
        for node in chain {
            self.nodes[node].parent = None;
            self.nodes[node].children = [None; 2];
            self.nodes[node].red = false;
            root = Some(if let Some(previous) = root {
                self.insert_tree(previous, node);
                self.balance(previous, node)
            } else {
                node
            });
        }
        if let Some(root) = root {
            self.front(at, root);
        }
    }

    fn front(&mut self, at: usize, root: usize) {
        let chain = &mut self.buckets[at].chain;
        if let Some(position) = chain.iter().position(|n| *n == root) {
            chain.remove(position);
            chain.insert(0, root);
        }
        self.buckets[at].root = Some(root);
    }
}

pub(super) fn java_order(tiles: Vec<(i32, i32)>) -> Vec<(i32, i32)> {
    let mut order = Order::default();
    for &(x, y) in &tiles {
        order.insert((x as u32) << 16 | y as u32);
    }
    order
        .buckets
        .into_iter()
        .flat_map(|b| b.chain)
        .map(|i| tiles[i])
        .collect()
}

#[cfg(test)]
mod tests;
