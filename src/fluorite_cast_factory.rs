//! Contains `FluoriteCastFactory`, a factory type used to instantiate `FluoriteCast`s.
use godot::{meta::conv::ObjectToOwned, prelude::*};

use hashbrown::HashSet;

use super::prelude::*;

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy, PartialEq)]
#[godot(via = i64)]
pub enum FluoriteFactoryOrchestrationMode {
    /// All instantiated casts will use the `evaluate_mode` config of themselves.
    DoesNotOrchestrate,
    /// All instantiated casts will be forced to not self-evaluate, managed by the factory.
    /// You need to call `evaluate_tracked_casts` on the factory manually.
    ForceManual,
    #[default]
    /// All instantiated casts will be forced to evaluate every `physics_process`, automatically managed by the factory.
    EveryPhysicsProcess,
    /// All instantiated casts will be forced to evaluate every `process`, automatically managed by the factory.
    EveryProcess,
}

#[derive(GodotClass)]
#[class(init, base=Node3D)]
/// A factory type that instantiates and keeps track of `FluoriteCast`s.
pub struct FluoriteCastFactory {
    base: Base<Node3D>,
    tracked_instances: HashSet<Gd<FluoriteCast>>,
    #[export]
    /// All instantiated casts will be parented to this node.
    /// 
    /// **This field must always be `Some`** (non-`null`).
    /// If this invariant is broken, the cast will panic.
    pub parent_to: Option<Gd<Node3D>>,
    #[export]
    /// The global fluid forwarded to instantiated casts. You should have a `FluoriteFluidConfig` resource
    /// in your project, then assign it here in the editor or statically via code.
    ///
    /// **This field must always be `Some`** (non-`null`).
    /// If this invariant is broken, the cast will panic.
    pub global_fluid: Option<Gd<FluoriteFluidConfig>>,
    #[export]
    /// The scene to instantiate a payload from for every cast by default, if any.
    /// 
    /// This field is optional. If `None` (`null`), then casts will be not visible by default, but still run.
    /// You can always override this whenever you instantiate a cast on behalf of this factory in the arguments of `fire_cast`.
    pub default_payload_scene: Option<Gd<PackedScene>>,
    #[export]
    /// If not `DoesNotOrchestrate`, all casts that this factory instantiates will never autonomously call `evaluate`.
    /// 
    /// Instead, the factory will call `evaluate` on all tracked casts at once every `process`
    /// (if `EveryPhysicsProcess`) or `physics_process` (if `EveryProcess`).
    /// 
    /// If this is `ForceManual`, then `evaluate_tracked_casts` must be called manually on the factory.
    /// 
    /// **Note: if this is `EveryPhysicsProcess` or `EveryProcess`, then the factory must be present in the scene tree**, or else automatic evaluation calls cannot be made!
    pub orchestrates_evaluation_as: FluoriteFactoryOrchestrationMode,
}

#[godot_api]
impl FluoriteCastFactory {
    //#[signal]
    //pub fn freeing(this: Gd<FluoriteCast>);
    #[signal]
    /// Fired when an instantiated cast expires.
    pub fn expired(this: Gd<FluoriteCast>);
    #[signal]
    /// Fired when an instantiated cast terminates.
    pub fn terminated(this: Gd<FluoriteCast>, cast_result: Gd<FluoriteSpaceCastResult>);
    #[signal]
    /// Fired when an instantiated cast penetrates.
    pub fn penetrated(this: Gd<FluoriteCast>, cast_result: Gd<FluoriteSpaceCastResult>);

    #[func]
    /// Constructs a new `FluoriteCastFactory`.
    /// Always use this instead of `FluoriteCastFactory.new()`.
    pub fn new_factory(
        parent_to: Gd<Node3D>,
        global_fluid: Gd<FluoriteFluidConfig>,
        orchestrates_evaluation_as: FluoriteFactoryOrchestrationMode,
        default_payload_scene: Option<Gd<PackedScene>>,
        // TODO: Instance pooling struct maybe?
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                tracked_instances: HashSet::new(),
                parent_to: Some(parent_to),
                default_payload_scene,
                global_fluid: Some(global_fluid),
                orchestrates_evaluation_as,
            }
        })
    }
    #[func]
    /// Instantiate a cast and fire it.
    /// 
    /// If `payload_override` is provided, the cast will use that node instead.
    /// Note that `payload_override` is a `Node3D`, not a `PackedScene`.
    /// If you need to pass a `PackedScene`, instantiate it in the call site first.
    pub fn fire_cast(
        &mut self,
        from: Transform3D,
        towards: Vector3,
        with_config: Gd<FluoriteCastConfig>,
        custom_data: VarDictionary,
        // if you are injecting a payload_override, it must be pre-instantiated, this is a conscious decision for allowing better control on the caller's side
        payload_override: Option<Gd<Node3D>>,
    ) -> Gd<FluoriteCast> {
        let mut new_instance = FluoriteCast::new_cast(
            self.parent_to.as_ref().expect("parent_to should exist").clone(),
            payload_override.map_or_else( // concise, but looks kind of ugly
                || self.default_payload_scene.as_ref().map(|packed_scene| {
                    packed_scene.try_instantiate_as().expect("payload_scene should always extend Node3D, if provided")
                }
            ), |payload| Some(payload)),
            with_config,
            self.global_fluid.as_ref().expect("global_fluid should always exist").clone(),
            custom_data,
            if let FluoriteFactoryOrchestrationMode::DoesNotOrchestrate = self.orchestrates_evaluation_as {false} else {true},
        );
        let prev = self.tracked_instances.replace(new_instance.clone());
        prev.inspect(|x| {
            godot_warn!("tracked_instances was already occupied with node: {}", x.to_string())
        });

        // no, we are not making a decl macro to avoid breaking DRY
        let self_id = self.object_to_owned().instance_id();
        // this signal only fires if `orchestrates_evaluation` is false
        // if this is fired when it is true, then this code will crash due to mutable borrow aliasing
        new_instance.signals().freeing().connect(move |this| {
            let maybe_self = Gd::<Self>::try_from_instance_id(self_id);
            let _  = maybe_self.map(|mut actually_self| {
                //actually_self.signals().freeing().emit(&this); // is there ever a reason to propagate up the freeing signal??
                actually_self.bind_mut().on_cast_freeing(this);
            }).is_err_and(|_| {
                godot_warn!("Received freeing signal from a FluoriteCast instance, but the factory that instantiated it is already freed");
                true
            });
        });
        let self_id = self.object_to_owned().instance_id();
        new_instance.signals().expired().connect(move |this| {
            let maybe_self = Gd::<Self>::try_from_instance_id(self_id);
            let _  = maybe_self.map(|actually_self| {
                actually_self.signals().expired().emit(&this);
            }).is_err_and(|_| {
                godot_warn!("Received expired signal from a FluoriteCast instance, but the factory that instantiated it is already freed");
                true
            });
        });
        let self_id = self.object_to_owned().instance_id();
        new_instance.signals().terminated().connect(move |this, cast_result| {
            let maybe_self = Gd::<Self>::try_from_instance_id(self_id);
            let _  = maybe_self.map(|actually_self| {
                actually_self.signals().terminated().emit(&this, &cast_result);
            }).is_err_and(|_| {
                godot_warn!("Received terminated signal from a FluoriteCast instance, but the factory that instantiated it is already freed");
                true
            });
        });
        let self_id = self.object_to_owned().instance_id();
        new_instance.signals().penetrated().connect(move |this, cast_result| {
            let maybe_self = Gd::<Self>::try_from_instance_id(self_id);
            let _  = maybe_self.map(|actually_self| {
                actually_self.signals().penetrated().emit(&this, &cast_result);
            }).is_err_and(|_| {
                godot_warn!("Received penetrated signal from a FluoriteCast instance, but the factory that instantiated it is already freed");
                true
            });
        });
        
        new_instance.bind_mut().fire(from, towards);
        new_instance
    }
    #[func]
    /// Check if this factory is tracking the given cast.
    pub fn is_tracking_this_cast(&self, &this: Gd<FluoriteCast>) -> bool {
        self.tracked_instances.contains(&this)
    }
    #[func]
    /// Get the list of casts the factory is currently tracking. Returns in Godot `Array` type.
    pub fn get_tracked_casts(&self) -> Array<Option<Gd<FluoriteCast>>> {
        let mut gdarray = Array::new();
        let mut tracked_iter = self.tracked_instances.iter();
        gdarray.resize(tracked_iter.len(), None::<&Gd<FluoriteCast>>); // forced to type annotate a None??
        // we know the exact size of our array, so we resize the array then set for each index. this should not crash.
        // unfortunately we have to stupidly use c-like array iteration to do this, and i cannot think of a better way
        for i in 0..tracked_iter.len() {
            gdarray.set(i, tracked_iter.next());
        }
        gdarray
    }
    /// Get the list of casts the factory is currently tracking. Returns in `HashSet` type.
    pub fn get_tracked_casts_rs(&self) -> &HashSet<Gd<FluoriteCast>> {
        &self.tracked_instances
    }
    #[func]
    /// Call `evaluate` on all casts that this factory is tracking.
    pub fn evaluate_tracked_casts(&mut self, delta: f64) -> () {
        let tracked = self.get_tracked_casts_rs();
        let mut to_free = Vec::new();
        for cast in tracked.iter() {
            let mut cast_owned = cast.to_godot_owned();
            cast_owned.bind_mut().evaluate(delta, false);
            if cast_owned.bind().is_scheduled_free() {
                to_free.push(cast.clone());
            }
        }
        for cast in to_free.iter() {
            self.on_cast_freeing(cast.clone());
        }
    }

    fn on_cast_freeing(&mut self, &this: Gd<FluoriteCast>) -> () {
        let taken = self.tracked_instances.remove(&this);
        if !taken {
            godot_warn!("Failed to remove node in tracked_instances: {}", this.to_string());
        }
    }
}

#[godot_api]
impl INode3D for FluoriteCastFactory {
    fn process(&mut self, delta: f64) -> () {
        if let FluoriteFactoryOrchestrationMode::EveryProcess = self.orchestrates_evaluation_as {
            self.evaluate_tracked_casts(delta);
        }
    }
    fn physics_process(&mut self, delta: f64) -> () {
        if let FluoriteFactoryOrchestrationMode::EveryPhysicsProcess = self.orchestrates_evaluation_as {
            self.evaluate_tracked_casts(delta);
        }
    }
}