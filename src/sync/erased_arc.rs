use alloc::sync::{Arc, Weak};
use core::{
    clone::Clone,
    marker::{PhantomData, Send, Sized, Sync},
    mem::ManuallyDrop,
    ops::Drop,
    option::{Option, Option::Some},
};

use crate::{erased_ptr::TypeErasedPtr, vtable::RcVTable};

pub struct TypeErasedArc {
    ptr: TypeErasedPtr,
    vtable: &'static RcVTable,
}

impl TypeErasedArc {
    #[inline]
    pub(crate) fn new<T: ?Sized + Send + Sync>(arc: Arc<T>) -> Self {
        Self {
            ptr: TypeErasedPtr::new(Arc::into_raw(arc)),
            vtable: &ArcErased::<T>::VTABLE,
        }
    }

    #[inline]
    pub(crate) fn as_ptr(&self) -> TypeErasedPtr {
        unsafe { (self.vtable.as_ptr)(self.ptr) }
    }

    #[inline]
    pub(crate) fn arc_ptr(&self) -> TypeErasedPtr {
        self.ptr
    }

    #[inline]
    pub(crate) fn downgrade(&self) -> TypeErasedWeak {
        TypeErasedWeak {
            // SAFETY: downgrade is guaranteed to return an erased pointer to Weak<T>
            ptr: unsafe { (self.vtable.downgrade)(self.ptr) },
            vtable: self.vtable,
        }
    }

    #[inline]
    pub(crate) fn strong_count(&self) -> usize {
        // SAFETY: once set in TypeErasedArc::new, self.vtable is never modified,
        // which guarantees that self.vtable and self.ptr match
        unsafe { (self.vtable.strong_count)(self.ptr) }
    }

    #[inline]
    pub(crate) fn weak_count(&self) -> usize {
        // SAFETY: once set in TypeErasedArc::new, self.vtable is never modified,
        // which guarantees that self.vtable and self.ptr match
        unsafe { (self.vtable.weak_count)(self.ptr) }
    }
}

impl Clone for TypeErasedArc {
    #[inline]
    fn clone(&self) -> Self {
        unsafe {
            // SAFETY: once set in TypeErasedArc::new, self.vtable is never modified,
            // which guarantees that self.vtable and self.ptr match
            (self.vtable.clone)(self.ptr);
        }
        Self { ..*self }
    }
}

impl Drop for TypeErasedArc {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: once set in TypeErasedArc::new, self.vtable is never modified,
        // which guarantees that self.vtable and self.ptr match
        unsafe { (self.vtable.drop)(self.ptr) }
    }
}

pub(crate) struct TypeErasedWeak {
    ptr: TypeErasedPtr,
    vtable: &'static RcVTable,
}

impl TypeErasedWeak {
    #[inline]
    pub(crate) fn upgrade(&self) -> Option<TypeErasedArc> {
        Some(TypeErasedArc {
            // SAFETY: upgrade_weak is guaranteed to return an erased pointer to Arc<T>
            ptr: unsafe { (self.vtable.upgrade_weak)(self.ptr) }?,
            vtable: self.vtable,
        })
    }

    #[inline]
    pub(crate) fn strong_count(&self) -> usize {
        // SAFETY: once set in TypeErasedWeak::new, self.vtable is never modified,
        // which guarantees that self.vtable and self.ptr match
        unsafe { (self.vtable.strong_count_weak)(self.ptr) }
    }

    #[inline]
    pub(crate) fn weak_count(&self) -> usize {
        // SAFETY: once set in TypeErasedWeak::new, self.vtable is never modified,
        // which guarantees that self.vtable and self.ptr match
        unsafe { (self.vtable.weak_count_weak)(self.ptr) }
    }
}

impl Clone for TypeErasedWeak {
    #[inline]
    fn clone(&self) -> Self {
        // SAFETY: once set in TypeErasedWeak::new, self.vtable is never modified,
        // which guarantees that self.vtable and self.ptr match
        unsafe { (self.vtable.clone_weak)(self.ptr) }
        Self { ..*self }
    }
}

impl Drop for TypeErasedWeak {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: once set in TypeErasedWeak::new, self.vtable is never modified,
        // which guarantees that self.vtable and self.ptr match
        unsafe { (self.vtable.drop_weak)(self.ptr) }
    }
}

pub(crate) struct ArcErased<T: ?Sized>(PhantomData<*const T>);

impl<T: ?Sized> ArcErased<T> {
    // A "vtable" for Arc<T> and sync::Weak<T> where T: ?Sized
    const VTABLE: RcVTable = RcVTable {
        as_ptr: Self::as_ptr,
        clone: Self::clone,
        drop: Self::drop,
        downgrade: Self::downgrade,
        strong_count: Self::strong_count,
        weak_count: Self::weak_count,
        clone_weak: Self::clone_weak,
        drop_weak: Self::drop_weak,
        upgrade_weak: Self::upgrade_weak,
        strong_count_weak: Self::strong_count_weak,
        weak_count_weak: Self::weak_count_weak,
    };

    // Must be called with an erased pointer to Arc<T>
    unsafe fn as_ptr(ptr: TypeErasedPtr) -> TypeErasedPtr {
        let arc = Self::as_manually_drop_arc(ptr);
        TypeErasedPtr::new(Arc::as_ptr(&arc))
    }

    // Must be called with an erased pointer to Arc<T>
    unsafe fn clone(ptr: TypeErasedPtr) {
        let arc: *const T = ptr.as_ptr();
        Arc::increment_strong_count(arc);
    }

    // Must be called with an erased pointer to Arc<T>
    unsafe fn drop(ptr: TypeErasedPtr) {
        let arc: Arc<T> = Arc::from_raw(ptr.as_ptr());
        core::mem::drop(arc);
    }

    // Must be called with an erased pointer to Arc<T>
    unsafe fn downgrade(ptr: TypeErasedPtr) -> TypeErasedPtr {
        let arc = Self::as_manually_drop_arc(ptr);
        let weak = Arc::downgrade(&arc);
        TypeErasedPtr::new(Weak::into_raw(weak))
    }

    // Must be called with an erased pointer to Arc<T>
    unsafe fn strong_count(ptr: TypeErasedPtr) -> usize {
        let arc = Self::as_manually_drop_arc(ptr);
        Arc::strong_count(&arc)
    }
    // Must be called with an erased pointer to Arc<T>
    unsafe fn weak_count(ptr: TypeErasedPtr) -> usize {
        let arc = Self::as_manually_drop_arc(ptr);
        Arc::weak_count(&arc)
    }
    // Must be called with an erased pointer to sync::Weak<T>
    unsafe fn clone_weak(ptr: TypeErasedPtr) {
        let weak = Self::as_manually_drop_weak(ptr);
        let _cloned = weak.clone();
    }
    // Must be called with an erased pointer to sync::Weak<T>
    unsafe fn drop_weak(ptr: TypeErasedPtr) {
        let weak: Weak<T> = Weak::from_raw(ptr.as_ptr());
        core::mem::drop(weak);
    }
    // Must be called with an erased pointer to sync::Weak<T>
    unsafe fn upgrade_weak(ptr: TypeErasedPtr) -> Option<TypeErasedPtr> {
        let weak = Self::as_manually_drop_weak(ptr);
        let arc = weak.upgrade();
        arc.map(|arc| TypeErasedPtr::new(Arc::into_raw(arc)))
    }
    // Must be called with an erased pointer to sync::Weak<T>
    unsafe fn strong_count_weak(ptr: TypeErasedPtr) -> usize {
        let weak = Self::as_manually_drop_weak(ptr);
        Weak::strong_count(&weak)
    }
    // Must be called with an erased pointer to sync::Weak<T>
    unsafe fn weak_count_weak(ptr: TypeErasedPtr) -> usize {
        let weak = Self::as_manually_drop_weak(ptr);
        Weak::weak_count(&weak)
    }

    // Must be called with an erased pointer to Arc<T>
    #[inline]
    unsafe fn as_manually_drop_arc(ptr: TypeErasedPtr) -> ManuallyDrop<Arc<T>> {
        ManuallyDrop::new(Arc::from_raw(ptr.as_ptr()))
    }

    // Must be called with an erased pointer to sync::Weak<T>
    #[inline]
    unsafe fn as_manually_drop_weak(ptr: TypeErasedPtr) -> ManuallyDrop<Weak<T>> {
        ManuallyDrop::new(Weak::from_raw(ptr.as_ptr()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn erased_arc_drops_object_when_last_instance_drops() {
        static mut DROPPED_COUNT: usize = 0;
        struct Drops;

        impl Drop for Drops {
            fn drop(&mut self) {
                // SAFETY: we're single-threaded and never hold a reference to this
                unsafe {
                    DROPPED_COUNT += 1;
                }
            }
        }
        let arc = Arc::new(Drops);
        let erased = TypeErasedArc::new(arc.clone());
        // The variable shouldn't drop after we drop the original Arc
        core::mem::drop(arc);
        assert_eq!(unsafe { DROPPED_COUNT }, 0);

        // The variable shouldn't drop after we drop a second erased instance
        let erased2 = erased.clone();
        core::mem::drop(erased2);
        assert_eq!(unsafe { DROPPED_COUNT }, 0);

        // The variable should drop after we drop the last instance
        // with the correct Drop implementation called
        core::mem::drop(erased);
        assert_eq!(unsafe { DROPPED_COUNT }, 1);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn erased_arc_drops_object_when_last_instance_drops_with_weak() {
        static mut DROPPED_COUNT: usize = 0;
        struct Drops;

        impl Drop for Drops {
            fn drop(&mut self) {
                // SAFETY: we're single-threaded and never hold a reference to this
                unsafe {
                    DROPPED_COUNT += 1;
                }
            }
        }
        let arc = Arc::new(Drops);
        let erased = TypeErasedArc::new(arc);
        let _weak = erased.downgrade();
        assert_eq!(unsafe { DROPPED_COUNT }, 0);

        // The variable shouldn't drop after we drop a second erased instance
        let erased2 = erased.clone();
        core::mem::drop(erased2);
        assert_eq!(unsafe { DROPPED_COUNT }, 0);

        // The variable should drop after we drop the last instance
        // with the correct Drop implementation called
        core::mem::drop(erased);
        assert_eq!(unsafe { DROPPED_COUNT }, 1);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn erased_arc_strong_count_tracks_instances() {
        let arc = Arc::new(42);
        let erased = TypeErasedArc::new(arc);
        assert_eq!(erased.strong_count(), 1);

        let erased2 = erased.clone();
        assert_eq!(erased.strong_count(), 2);
        assert_eq!(erased2.strong_count(), 2);

        let weak = erased.downgrade();
        assert_eq!(erased.strong_count(), 2);
        assert_eq!(erased2.strong_count(), 2);
        assert_eq!(weak.strong_count(), 2);

        core::mem::drop(erased);
        core::mem::drop(weak);
        assert_eq!(erased2.strong_count(), 1);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn erased_arc_weak_count() {
        let arc = Arc::new(42);
        let erased = TypeErasedArc::new(arc);
        assert_eq!(erased.weak_count(), 0);

        let erased2 = erased.clone();
        assert_eq!(erased.weak_count(), 0);
        assert_eq!(erased2.weak_count(), 0);

        core::mem::drop(erased);
        assert_eq!(erased2.weak_count(), 0);

        let weak = erased2.downgrade();
        assert_eq!(erased2.weak_count(), 1);
        assert_eq!(weak.weak_count(), 1);

        let weak2 = weak.clone();
        assert_eq!(weak.weak_count(), 2);
        assert_eq!(weak2.weak_count(), 2);

        core::mem::drop(erased2);
        // weak_count returns 0 when there are no remaning Arcs
        assert_eq!(weak.weak_count(), 0);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn weak_can_upgrade_when_there_are_instances() {
        let arc = Arc::new(42);
        let erased = TypeErasedArc::new(arc);
        let weak = erased.downgrade();

        let upgraded = weak.upgrade().unwrap();
        core::mem::drop(erased);
        assert_eq!(upgraded.strong_count(), 1);

        core::mem::drop(upgraded);

        let upgraded = weak.upgrade();
        assert!(upgraded.is_none());
    }
}
