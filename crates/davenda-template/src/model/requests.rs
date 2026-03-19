use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotFill {
    Template(TemplateSelector),
    Nodes(Vec<Node>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRenderRequest {
    pub(crate) layout: TemplateSelector,
    pub(crate) model: RenderModel,
    pub(crate) slots: BTreeMap<SlotName, SlotFill>,
}

impl DocumentRenderRequest {
    pub fn new(layout: TemplateSelector, model: RenderModel) -> Self {
        Self {
            layout,
            model,
            slots: BTreeMap::new(),
        }
    }

    pub fn with_slot_fill(mut self, slot: SlotName, fill: SlotFill) -> Self {
        self.slots.insert(slot, fill);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentRenderRequest {
    pub(crate) fragment: TemplateSelector,
    pub(crate) model: RenderModel,
}

impl FragmentRenderRequest {
    pub fn new(fragment: TemplateSelector, model: RenderModel) -> Self {
        Self { fragment, model }
    }
}
