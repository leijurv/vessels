use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::Value;

struct ContextState {
    channel_types: HashMap<u32, (TypeId, TypeId)>,
    unused_indices: Vec<u32>,
    next_index: u32,
}

#[derive(Clone)]
pub struct Context {
    state: Arc<RwLock<ContextState>>,
}

impl Context {
    pub(crate) fn new_with<V: Value>() -> Self {
        let mut channel_types = HashMap::new();

        channel_types.insert(
            0,
            (
                TypeId::of::<V::ConstructItem>(),
                TypeId::of::<V::DeconstructItem>(),
            ),
        );

        Context {
            state: Arc::new(RwLock::new(ContextState {
                channel_types,
                next_index: 1,
                unused_indices: vec![],
            })),
        }
    }

    pub(crate) fn new() -> Self {
        Context {
            state: Arc::new(RwLock::new(ContextState {
                channel_types: HashMap::new(),
                next_index: 1,
                unused_indices: vec![],
            })),
        }
    }

    pub(crate) fn get(&self, channel: &'_ u32) -> Option<(TypeId, TypeId)> {
        self.state
            .read()
            .unwrap()
            .channel_types
            .get(channel)
            .map(|c| *c)
    }

    pub(crate) fn create<V: Value>(&self) -> u32 {
        let mut state = self.state.write().unwrap();
        let c = TypeId::of::<V::ConstructItem>();
        let d = TypeId::of::<V::DeconstructItem>();

        if let Some(id) = state.unused_indices.pop() {
            state.channel_types.insert(id, (c, d));
            id
        } else {
            let id = state.next_index;
            state.next_index += 1;
            state.channel_types.insert(id, (c, d));
            id
        }
    }

    pub(crate) fn add<V: Value>(&self, handle: u32) {
        let mut state = self.state.write().unwrap();
        let c = TypeId::of::<V::ConstructItem>();
        let d = TypeId::of::<V::DeconstructItem>();
        state.channel_types.insert(handle, (c, d));
    }

    pub(crate) fn len(&self) -> usize {
        self.state.read().unwrap().channel_types.len()
    }

    pub(crate) fn only(&self) -> Option<(u32, (TypeId, TypeId))> {
        let state = self.state.read().unwrap();
        if state.channel_types.len() == 1 {
            let item = state.channel_types.iter().next().unwrap();
            Some((*item.0, *item.1))
        } else {
            None
        }
    }
}
