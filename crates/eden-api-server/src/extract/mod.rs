// use std::marker::PhantomData;

// use crate::auth::AuthRequirement;

// pub struct HasGranted<T: RequirementLogic>(PhantomData<fn(T) -> bool>);

// pub trait RequirementLogic {
//     const REQUIREMENT: AuthRequirement;
// }

// // impl<const T: u64> Requires<T> {
// //     pub fn stuff(&self) {
// //         PermissionScope::from_bits_retain(T);
// //     }
// // }
