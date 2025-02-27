//! # Ids
//!
//! [`Id`]s are unique identifiers for views.
//! They're used to identify views in the view tree.
//!
//! ## Ids and Id paths
//!
//! These ids are assigned via the [ViewContext](crate::ViewContext) and are unique across the entire application.
//!

use std::{any::Any, cell::RefCell, collections::HashMap, sync::atomic::AtomicU64};

use kurbo::{Point, Rect};

use crate::{
    animate::Animation,
    context::{EventCallback, MenuCallback, ResizeCallback},
    event::EventListener,
    style::{Style, StyleClassRef, StyleSelector},
    update::{UpdateMessage, CENTRAL_DEFERRED_UPDATE_MESSAGES, CENTRAL_UPDATE_MESSAGES},
    view_data::{ChangeFlags, StackOffset},
};

thread_local! {
    pub(crate) static ID_PATHS: RefCell<HashMap<Id,IdPath>> = Default::default();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Hash)]
/// A stable identifier for an element.
pub struct Id(u64);

#[derive(Clone, Default, Debug)]
pub struct IdPath(pub(crate) Vec<Id>);

impl IdPath {
    /// Returns the slice of the ids including the first id identifying the window.
    pub(crate) fn dispatch(&self) -> &[Id] {
        &self.0[..]
    }
}

impl Id {
    /// Allocate a new, unique `Id`.
    pub fn next() -> Id {
        static WIDGET_ID_COUNTER: AtomicU64 = AtomicU64::new(0);
        Id(WIDGET_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }

    pub fn to_raw(self) -> u64 {
        self.0
    }

    pub fn new(&self) -> Id {
        let mut id_path =
            ID_PATHS.with(|id_paths| id_paths.borrow().get(self).cloned().unwrap_or_default());
        let new_id = if id_path.0.is_empty() {
            // if id_path is empty, it means the id was generated by next() and it's not
            // tracked yet, so we can just reuse it
            *self
        } else {
            Self::next()
        };
        id_path.0.push(new_id);
        ID_PATHS.with(|id_paths| {
            id_paths.borrow_mut().insert(new_id, id_path);
        });
        new_id
    }

    pub(crate) fn set_parent(&self, parent: Id) {
        ID_PATHS.with(|id_paths| {
            let mut id_paths = id_paths.borrow_mut();
            let mut id_path = id_paths.get(&parent).cloned().unwrap();
            id_path.0.push(*self);
            id_paths.insert(*self, id_path);
        });
    }

    pub fn parent(&self) -> Option<Id> {
        ID_PATHS.with(|id_paths| {
            id_paths.borrow().get(self).and_then(|id_path| {
                let id_path = &id_path.0;
                let len = id_path.len();
                if len >= 2 {
                    Some(id_path[len - 2])
                } else {
                    None
                }
            })
        })
    }

    pub fn id_path(&self) -> Option<IdPath> {
        ID_PATHS.with(|id_paths| id_paths.borrow().get(self).cloned())
    }

    pub fn has_id_path(&self) -> bool {
        ID_PATHS.with(|id_paths| id_paths.borrow().contains_key(self))
    }

    pub fn remove_id_path(&self) {
        ID_PATHS.with(|id_paths| id_paths.borrow_mut().remove(self));
    }

    pub fn root_id(&self) -> Option<Id> {
        ID_PATHS.with(|id_paths| {
            id_paths
                .borrow()
                .get(self)
                .and_then(|path| path.0.first().copied())
        })
    }

    pub fn request_focus(&self) {
        self.add_update_message(UpdateMessage::Focus(*self));
    }

    pub fn request_active(&self) {
        self.add_update_message(UpdateMessage::Active(*self));
    }

    pub fn update_disabled(&self, is_disabled: bool) {
        self.add_update_message(UpdateMessage::Disabled {
            id: *self,
            is_disabled,
        });
    }

    pub fn request_paint(&self) {
        self.add_update_message(UpdateMessage::RequestPaint);
    }

    pub fn request_layout(&self) {
        self.add_update_message(UpdateMessage::RequestChange {
            id: *self,
            flags: ChangeFlags::LAYOUT,
        });
    }

    pub fn update_state(&self, state: impl Any) {
        self.add_update_message(UpdateMessage::State {
            id: *self,
            state: Box::new(state),
        });
    }

    pub fn update_state_deferred(&self, state: impl Any) {
        CENTRAL_DEFERRED_UPDATE_MESSAGES.with(|msgs| {
            msgs.borrow_mut().push((*self, Box::new(state)));
        });
    }

    pub(crate) fn update_style(&self, style: Style, offset: StackOffset<Style>) {
        self.add_update_message(UpdateMessage::Style {
            id: *self,
            style,
            offset,
        });
    }

    pub fn update_class(&self, class: StyleClassRef) {
        self.add_update_message(UpdateMessage::Class { id: *self, class });
    }

    pub(crate) fn update_style_selector(&self, style: Style, selector: StyleSelector) {
        self.add_update_message(UpdateMessage::StyleSelector {
            id: *self,
            style,
            selector,
        });
    }

    pub fn keyboard_navigatable(&self) {
        self.add_update_message(UpdateMessage::KeyboardNavigable { id: *self });
    }

    pub fn draggable(&self) {
        self.add_update_message(UpdateMessage::Draggable { id: *self });
    }

    pub fn update_event_listener(&self, listener: EventListener, action: Box<EventCallback>) {
        self.add_update_message(UpdateMessage::EventListener {
            id: *self,
            listener,
            action,
        });
    }

    pub fn update_resize_listener(&self, action: Box<ResizeCallback>) {
        self.add_update_message(UpdateMessage::ResizeListener { id: *self, action });
    }

    pub fn update_move_listener(&self, action: Box<dyn Fn(Point)>) {
        self.add_update_message(UpdateMessage::MoveListener { id: *self, action });
    }

    pub fn update_cleanup_listener(&self, action: Box<dyn Fn()>) {
        self.add_update_message(UpdateMessage::CleanupListener { id: *self, action });
    }

    pub fn update_animation(&self, animation: Animation) {
        self.add_update_message(UpdateMessage::Animation {
            id: *self,
            animation,
        });
    }

    pub fn clear_focus(&self) {
        self.add_update_message(UpdateMessage::ClearFocus(*self));
    }

    pub fn update_context_menu(&self, menu: Box<MenuCallback>) {
        self.add_update_message(UpdateMessage::ContextMenu { id: *self, menu });
    }

    pub fn update_popout_menu(&self, menu: Box<MenuCallback>) {
        self.add_update_message(UpdateMessage::PopoutMenu { id: *self, menu });
    }

    pub fn scroll_to(&self, rect: Option<Rect>) {
        self.add_update_message(UpdateMessage::ScrollTo { id: *self, rect });
    }

    pub fn inspect(&self) {
        self.add_update_message(UpdateMessage::Inspect);
    }

    fn add_update_message(&self, msg: UpdateMessage) {
        CENTRAL_UPDATE_MESSAGES.with(|msgs| {
            msgs.borrow_mut().push((*self, msg));
        });
    }
}
