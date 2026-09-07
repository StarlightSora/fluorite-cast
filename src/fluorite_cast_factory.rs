use godot::{meta::conv::ObjectToOwned, prelude::*};

use hashbrown::HashSet;
use crate::prelude::*;

#[derive(GodotClass)]
#[class(init, base=Node3D)]
pub struct FluoriteCastFactory {
    base: Base<Node3D>,
    tracked_instances: HashSet<Gd<FluoriteCast>>,
    #[export]
    pub parent_to: Option<Gd<Node3D>>,
    #[export]
    pub payload_scene: Option<Gd<PackedScene>>,
    #[export]
    pub projectile_config: Option<Gd<FluoriteCastConfig>>,
    #[export]
    pub global_fluid: Option<Gd<FluoriteFluidConfig>>,
}

#[godot_api]
impl FluoriteCastFactory {
    //#[signal]
    //pub fn freeing(this: Gd<FluoriteCast>);
    #[signal]
    pub fn expired(this: Gd<FluoriteCast>);
    #[signal]
    pub fn terminated(this: Gd<FluoriteCast>, cast_result: Gd<FluoriteSpaceCastResult>);
    #[signal]
    pub fn penetrated(this: Gd<FluoriteCast>, cast_result: Gd<FluoriteSpaceCastResult>);

    #[func]
    pub fn new_factory(
        parent_to: Gd<Node3D>,
        payload_scene: Option<Gd<PackedScene>>,
        projectile_config: Gd<FluoriteCastConfig>,
        global_fluid: Gd<FluoriteFluidConfig>,
        // TODO: Instance pooling struct maybe?
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                tracked_instances: HashSet::new(),
                parent_to: Some(parent_to),
                payload_scene,
                projectile_config: Some(projectile_config),
                global_fluid: Some(global_fluid),
            }
        })
    }
    #[func]
    pub fn fire_cast(
        &mut self,
        from: Transform3D,
        towards: Vector3,
        custom_data: VarDictionary,
        config_override: Option<Gd<FluoriteCastConfig>>,
        // if you are injecting a payload_override, it must be pre-instantiated, this is a conscious decision for allowing better control on the caller
        payload_override: Option<Gd<Node3D>>,
    ) -> Gd<FluoriteCast> {
        let mut new_instance = FluoriteCast::new_cast(
            self.parent_to.as_ref().expect("parent_to should exist").clone(),
            payload_override.map_or_else( // concise, but looks kind of ugly
                || self.payload_scene.as_ref().map(|packed_scene| {
                    packed_scene.try_instantiate_as().expect("payload_scene should always extend Node3D, if provided")
                }
            ), |payload| Some(payload)),
            config_override.unwrap_or_else(|| {
                self.projectile_config.as_ref().expect("projectile_config should always exist").clone()
            }),
            self.global_fluid.as_ref().expect("global_fluid should always exist").clone(),
            custom_data,
        );
        let prev = self.tracked_instances.replace(new_instance.clone());
        prev.inspect(|x| {
            godot_warn!("tracked_instances was already occupied with node: {}", x.to_string())
        });

        // no, we are not making a decl macro to avoid breaking DRY
        let self_id = self.object_to_owned().instance_id();
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

    fn on_cast_freeing(&mut self, this: Gd<FluoriteCast>) -> () {
        let taken = self.tracked_instances.remove(&this);
        if !taken {
            godot_warn!("Failed to remove node in tracked_instances: {}", this.to_string());
        }
    }
}