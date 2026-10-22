//! Native Ubi model/reducer/view integration; no syntax or compiler builtins.
use crate::Runtime;
use serde_json::Value;
use ubi_ui_platform::{Application, Changes, Error, Event, Tree, View};

/// Owns Ubi state and commits reducer/view results transactionally.
pub struct NativeApplication {
    runtime: Runtime,
    module: String,
    model: Value,
    tree: Tree,
    pub view_evaluations: u64,
}
impl NativeApplication {
    pub fn new(runtime: Runtime, module: impl Into<String>) -> Result<Self, Error> {
        let module = module.into();
        let model = runtime
            .invoke(&module, "init", &[])
            .map_err(|e| Error(e.to_string()))?;
        let view = runtime
            .invoke(&module, "view", std::slice::from_ref(&model))
            .map_err(|e| Error(e.to_string()))?;
        Ok(Self {
            runtime,
            module,
            model,
            tree: Tree::new(View::decode(view)?),
            view_evaluations: 1,
        })
    }
    pub fn model(&self) -> &Value {
        &self.model
    }
    pub fn revision(&self) -> u64 {
        self.tree.revision
    }
}
impl Application for NativeApplication {
    fn view(&self) -> &View {
        self.tree.view()
    }
    fn dispatch(&mut self, event: &Event) -> Result<Changes, Error> {
        // Events must correspond to an enabled node's registered message. The
        // runtime independently validates every payload against the Ubi type.
        let node = self
            .view()
            .nodes
            .get(&event.key)
            .ok_or("event target unmounted")?;
        if node.disabled || node.busy {
            return Err("event target disabled".into());
        }
        let tag = event
            .value
            .get("tag")
            .and_then(Value::as_str)
            .ok_or("invalid event envelope")?;
        let values = event
            .value
            .get("values")
            .and_then(Value::as_array)
            .ok_or("invalid event payload")?;
        let expected = match tag {
            "Activate" => &node.activate,
            "Input" => &node.input,
            "Toggle" => &node.toggle,
            "Select" => &node.change,
            _ => return Err("unsupported native event".into()),
        };
        if expected
            .as_ref()
            .is_none_or(|message| values.first() != Some(message))
        {
            return Err("event message does not match binding".into());
        }
        let next = self
            .runtime
            .invoke(
                &self.module,
                "update",
                &[self.model.clone(), event.value.clone()],
            )
            .map_err(|e| Error(e.to_string()))?;
        if next == self.model {
            return Ok(Changes::default());
        }
        let view = self
            .runtime
            .invoke(&self.module, "view", std::slice::from_ref(&next))
            .map_err(|e| Error(e.to_string()))?;
        let view = View::decode(view)?;
        let changes = self.tree.reconcile(view);
        self.model = next;
        self.view_evaluations += 1;
        Ok(changes)
    }
}
