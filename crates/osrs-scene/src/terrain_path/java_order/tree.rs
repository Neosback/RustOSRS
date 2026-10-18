//! Red/black insertion for the integer-key iteration model. Child 0 is left, child 1 right.
use super::Order;

impl Order {
    pub(super) fn insert_tree(&mut self, mut parent: usize, node: usize) -> usize {
        loop {
            let side = usize::from(self.nodes[node].hash > self.nodes[parent].hash);
            if let Some(child) = self.nodes[parent].children[side] {
                parent = child;
            } else {
                self.nodes[parent].children[side] = Some(node);
                self.nodes[node].parent = Some(parent);
                return parent;
            }
        }
    }

    fn rotate(&mut self, mut root: usize, pivot: usize, side: usize) -> usize {
        let Some(child) = self.nodes[pivot].children[1 - side] else {
            return root;
        };
        let middle = self.nodes[child].children[side];
        self.nodes[pivot].children[1 - side] = middle;
        if let Some(middle) = middle {
            self.nodes[middle].parent = Some(pivot);
        }
        let parent = self.nodes[pivot].parent;
        self.nodes[child].parent = parent;
        if let Some(parent) = parent {
            let slot = usize::from(self.nodes[parent].children[1] == Some(pivot));
            self.nodes[parent].children[slot] = Some(child);
        } else {
            root = child;
            self.nodes[child].red = false;
        }
        self.nodes[child].children[side] = Some(pivot);
        self.nodes[pivot].parent = Some(child);
        root
    }

    pub(super) fn balance(&mut self, mut root: usize, mut node: usize) -> usize {
        self.nodes[node].red = true;
        loop {
            let Some(mut parent) = self.nodes[node].parent else {
                self.nodes[node].red = false;
                return node;
            };
            if !self.nodes[parent].red {
                return root;
            }
            let Some(grand) = self.nodes[parent].parent else {
                return root;
            };
            let side = usize::from(self.nodes[grand].children[1] == Some(parent));
            if let Some(uncle) = self.nodes[grand].children[1 - side]
                && self.nodes[uncle].red
            {
                self.nodes[uncle].red = false;
                self.nodes[parent].red = false;
                self.nodes[grand].red = true;
                node = grand;
                continue;
            }
            if self.nodes[parent].children[1 - side] == Some(node) {
                root = self.rotate(root, parent, side);
                std::mem::swap(&mut node, &mut parent);
            }
            self.nodes[parent].red = false;
            self.nodes[grand].red = true;
            return self.rotate(root, grand, 1 - side);
        }
    }
}
