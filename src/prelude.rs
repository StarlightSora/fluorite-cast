//! Re-exports commonly used symbols when using this libary.
//! 
//! Declare this in your code to bring all the re-exports this module does into scope:
//! ```
//! use fluorite_cast::prelude::*;
//! ```
use super::fluorite_cast;
use super::fluorite_cast_config;
use super::fluorite_cast::builtins;

pub use super::fluorite_fluid_config::FluoriteFluidConfig;

pub use super::fluorite_fluid_area3d::FluoriteFluidArea3D;

pub use fluorite_cast::FluoriteCast;
pub use fluorite_cast::FluoriteSpaceCastResult;

pub use builtins::FluoriteBuiltinConfig;

pub use fluorite_cast_config::CollisionDetectionMode;
pub use fluorite_cast_config::MaybeExecuteCodeVia;
pub use fluorite_cast_config::FluoriteCastCfgGravity;
pub use fluorite_cast_config::FluoriteCastCfgHitDetection;
pub use fluorite_cast_config::FluoriteCastCfgBuiltinFlags;
pub use fluorite_cast_config::FluoriteCastCfgMethods;
pub use fluorite_cast_config::FluoriteCastConfig;

pub use super::fluorite_cast_factory::FluoriteCastFactory;