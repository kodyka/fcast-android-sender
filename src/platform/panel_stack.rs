//! Panel navigation stack backing the Slint panel model.

pub(crate) struct PanelStack(std::cell::RefCell<Vec<crate::Panel>>);

impl PanelStack {
    pub(crate) fn new() -> Self {
        Self(std::cell::RefCell::new(Vec::new()))
    }

    pub(crate) fn push_panel(&self, current: crate::Panel) {
        if current != crate::Panel::None {
            self.0.borrow_mut().insert(0, current);
        }
    }

    pub(crate) fn pop_panel(&self) -> crate::Panel {
        if self.0.borrow().is_empty() {
            return crate::Panel::None;
        }
        self.0.borrow_mut().remove(0)
    }

    pub(crate) fn clear(&self) {
        self.0.borrow_mut().clear();
    }

    pub(crate) fn as_model(&self) -> slint::ModelRc<crate::Panel> {
        std::rc::Rc::new(slint::VecModel::from(self.0.borrow().clone())).into()
    }
}
