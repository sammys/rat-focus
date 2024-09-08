#![doc = include_str!("../readme.md")]

mod focus;
mod zrect;

#[allow(unused_imports)]
use log::debug;
use ratatui::layout::Rect;
use std::cell::Cell;
use std::fmt::{Debug, Display, Formatter};
use std::sync::{Arc, RwLock};

pub use crate::focus::{handle_focus, handle_mouse_focus, Focus};
pub use crate::zrect::ZRect;

pub mod event {
    //! Rexported eventhandling traits.
    pub use rat_event::{
        crossterm, ct_event, flow, try_flow, util, ConsumedEvent, HandleEvent, MouseOnly, Outcome,
        Regular,
    };
}

/// Holds the flags for the focus.
/// This struct is embedded in the widget state.
///
/// This is a wrapper around Rc<_some_impl_>, so cloning this is
/// cheap and what you want to do if you implement any high-level
/// focus-handling.
///
/// Attention:
/// Equality for FocusFlag means pointer-equality of the underlying
/// Rc using Rc::ptr_eq.
///
/// See [HasFocusFlag], [on_gained!](crate::on_gained!) and
/// [on_lost!](crate::on_lost!).
///
#[derive(Clone, Default)]
pub struct FocusFlag(Arc<FocusFlagCore>);

/// The same as FocusFlag, but distinct to mark the focus for
/// a container.
///
/// This serves the purpose of
///
/// * summarizing the focus for the container. If any of the
///     widgets of the container has the focus, the container
///     itself has the focus).
/// * identifying the container.
#[derive(Clone, Default)]
pub struct ContainerFlag(Arc<FocusFlagCore>);

/// Equality for FocusFlag means pointer equality of the underlying
/// Rc using Rc::ptr_eq.
impl PartialEq for FocusFlag {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for FocusFlag {}

impl Display for FocusFlag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "|{}|", self.0.name)
    }
}

/// Equality for ContainerFlag means pointer equality of the underlying
/// Rc using Rc::ptr_eq.
impl PartialEq for ContainerFlag {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ContainerFlag {}

impl Display for ContainerFlag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "|{}|", self.0.name)
    }
}

// not Clone, always Rc<>
#[derive(Default)]
struct FocusFlagCore {
    /// Field name for debugging purposes.
    name: Box<str>,
    /// Focus.
    focus: RwLock<bool>,
    /// This widget just gained the focus. This flag is set by [Focus::handle]
    /// if there is a focus transfer, and will be reset by the next
    /// call to [Focus::handle].
    ///
    /// See [on_gained!](crate::on_gained!)
    gained: RwLock<bool>,
    /// This widget just lost the focus. This flag is set by [Focus::handle]
    /// if there is a focus transfer, and will be reset by the next
    /// call to [Focus::handle].
    ///
    /// See [on_lost!](crate::on_lost!)
    lost: RwLock<bool>,
}

/// Focus navigation for widgets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Navigation {
    /// Widget is not reachable with normal keyboard or mouse navigation.
    None,
    /// Widget is not reachable with keyboard navigation, but can be focused with the mouse.
    Mouse,
    /// Widget cannot be reached with normal keyboard navigation, but can be left.
    /// (e.g. Tabs, Split-Divider)
    Leave,
    /// Widget can be reached with normal keyboard navigation, but not left.
    /// (e.g. TextArea)
    Reach,
    /// Widget can be reached with normal keyboard navigation, but only be left with
    /// backward navigation.
    ReachLeaveFront,
    /// Widget can be reached with normal keyboard navigation, but only be left with
    /// forward navigation.
    ReachLeaveBack,
    /// Widget can be reached and left with normal keyboard navigation.
    #[default]
    Regular,
}

/// Trait for a widget that has a focus flag.
pub trait HasFocusFlag {
    /// Access to the flag for the rest.
    fn focus(&self) -> FocusFlag;

    /// Access the area for mouse focus.
    fn area(&self) -> Rect;

    /// The widget might have several disjointed areas.
    /// This is the case if it is showing a popup, but there
    /// might be other causes.
    ///
    /// This is seen as a higher resolution image of the
    /// area given with area(). That means the result of
    /// area() is the union of all areas given here.
    fn z_areas(&self) -> &[ZRect] {
        &[]
    }

    /// Declares how the widget interacts with focus.
    ///
    /// Default is Navigation::Regular.
    fn navigable(&self) -> Navigation {
        Navigation::Regular
    }

    /// Focused?
    fn is_focused(&self) -> bool {
        self.focus().get()
    }

    /// Just lost focus.
    fn lost_focus(&self) -> bool {
        self.focus().lost()
    }

    /// Just gained focus.
    fn gained_focus(&self) -> bool {
        self.focus().gained()
    }
}

/// Is this a container widget.
pub trait HasFocus {
    /// Returns a Focus struct.
    fn focus(&self) -> Focus;

    /// Returns the container-flag
    fn container(&self) -> Option<ContainerFlag> {
        self.focus().container_flag()
    }

    /// Area of the container.
    fn area(&self) -> Rect {
        self.focus().container_area()
    }

    /// Focused?
    fn is_focused(&self) -> bool {
        if let Some(flag) = self.container() {
            flag.get()
        } else {
            false
        }
    }

    /// Just lost focus.
    fn lost_focus(&self) -> bool {
        if let Some(flag) = self.container() {
            flag.lost()
        } else {
            false
        }
    }

    /// Just gained focus.
    fn gained_focus(&self) -> bool {
        if let Some(flag) = self.container() {
            flag.gained()
        } else {
            false
        }
    }
}

impl Debug for FocusFlag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FocusFlag")
            .field("name", &self.name())
            .field("focus", &self.get())
            .field("gained", &self.gained())
            .field("lost", &self.lost())
            .finish()
    }
}

impl Debug for ContainerFlag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerFlag")
            .field("name", &self.name())
            .field("focus", &self.get())
            .field("gained", &self.gained())
            .field("lost", &self.lost())
            .finish()
    }
}

impl FocusFlag {
    /// Create a default flag.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a named flag.
    pub fn named(name: &str) -> Self {
        Self(Arc::new(FocusFlagCore::named(name)))
    }

    /// Has the focus.
    #[inline]
    pub fn get(&self) -> bool {
        *(self.0.focus.read().unwrap())
    }

    /// Set the focus.
    #[inline]
    pub fn set(&self, focus: bool) {
        *(self.0.focus.write().unwrap()) = focus;
    }

    /// Get the field-name.
    #[inline]
    pub fn name(&self) -> &str {
        self.0.name.as_ref()
    }

    /// Just lost the focus.
    #[inline]
    pub fn lost(&self) -> bool {
        *(self.0.lost.read().unwrap())
    }

    #[inline]
    pub fn set_lost(&self, lost: bool) {
        *(self.0.lost.write().unwrap()) = lost;
    }

    /// Just gained the focus.
    #[inline]
    pub fn gained(&self) -> bool {
        *(self.0.gained.read().unwrap())
    }

    #[inline]
    pub fn set_gained(&self, gained: bool) {
        *(self.0.gained.write().unwrap()) = gained;
    }

    /// Reset all flags to false.
    #[inline]
    pub fn clear(&self) {
        *(self.0.focus.write().unwrap()) = false;
        *(self.0.lost.write().unwrap()) = false;
        *(self.0.gained.write().unwrap()) = false;
    }
}

impl ContainerFlag {
    /// Create a default flag.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a named flag.
    pub fn named(name: &str) -> Self {
        Self(Arc::new(FocusFlagCore::named(name)))
    }

    /// Has the focus.
    #[inline]
    pub fn get(&self) -> bool {
        *(self.0.focus.read().unwrap())
    }

    /// Set the focus.
    #[inline]
    pub fn set(&self, focus: bool) {
        *(self.0.focus.write().unwrap()) = focus;
    }

    /// Get the field-name.
    #[inline]
    pub fn name(&self) -> &str {
        self.0.name.as_ref()
    }

    /// Just lost the focus.
    #[inline]
    pub fn lost(&self) -> bool {
        *(self.0.lost.read().unwrap())
    }

    #[inline]
    pub fn set_lost(&self, lost: bool) {
        *(self.0.lost.write().unwrap()) = lost;
    }

    /// Just gained the focus.
    #[inline]
    pub fn gained(&self) -> bool {
        *(self.0.gained.read().unwrap())
    }

    #[inline]
    pub fn set_gained(&self, gained: bool) {
        *(self.0.gained.write().unwrap()) = gained
    }

    /// Reset all flags to false.
    #[inline]
    pub fn clear(&self) {
        *(self.0.focus.write().unwrap()) = false;
        *(self.0.lost.write().unwrap()) = false;
        *(self.0.gained.write().unwrap()) = false;    }
}

impl FocusFlagCore {
    pub(crate) fn named(name: &str) -> Self {
        Self {
            name: name.into(),
            focus: RwLock::new(false),
            gained: RwLock::new(false),
            lost: RwLock::new(false),
        }
    }
}

/// Does a match on the state struct of a widget. If `widget_state.lost_focus()` is true
/// the block is executed. This requires that `widget_state` implements [HasFocusFlag],
/// but that's the basic requirement for this whole crate.
///
/// ```rust ignore
/// use rat_focus::on_lost;
///
/// on_lost!(
///     state.field1 => {
///         // do checks
///     },
///     state.field2 => {
///         // do checks
///     }
/// );
/// ```
#[macro_export]
macro_rules! on_lost {
    ($($field:expr => $validate:expr),*) => {{
        use $crate::HasFocusFlag;
        $(if $field.lost_focus() { _ = $validate })*
    }};
}

/// Does a match on the state struct of a widget. If `widget_state.gained_focus()` is true
/// the block is executed. This requires that `widget_state` implements [HasFocusFlag],
/// but that's the basic requirement for this whole crate.
///
/// ```rust ignore
/// use rat_focus::on_gained;
///
/// on_gained!(
///     state.field1 => {
///         // do prep
///     },
///     state.field2 => {
///         // do prep
///     }
/// );
/// ```
#[macro_export]
macro_rules! on_gained {
    ($($field:expr => $validate:expr),*) => {{
        use $crate::HasFocusFlag;
        $(if $field.gained_focus() { _ = $validate })*
    }};
}

/// Does a match on the state struct of a widget. If
/// `widget_state.is_focused()` is true the block is executed.
/// There is a `_` branch too, that is evaluated if none of the
/// given widget-states has the focus.
///
/// This requires that `widget_state` implements [HasFocusFlag],
/// but that's the basic requirement for this whole crate.
///
/// ```rust ignore
/// use rat_focus::match_focus;
///
/// let res = match_focus!(
///     state.field1 => {
///         // do this
///         true
///     },
///     state.field2 => {
///         // do that
///         true
///     },
///     _ => {
///         false
///     }
/// );
///
/// if res {
///     // react
/// }
/// ```
///
#[macro_export]
macro_rules! match_focus {
    ($($field:expr => $block:expr),* $(, _ => $final:expr)?) => {{
        use $crate::HasFocusFlag;
        if false {
            unreachable!();
        }
        $(else if $field.is_focused() { $block })*
        $(else { $final })?
    }};
}
