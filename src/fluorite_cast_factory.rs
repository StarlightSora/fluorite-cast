use godot::prelude::*;

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
    #[func]
    pub fn new_factory(
        parent_to: Gd<Node3D>,
        payload_scene: Option<Gd<PackedScene>>,
        projectile_config: Gd<FluoriteCastConfig>,
        global_fluid: Gd<FluoriteFluidConfig>,
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
    pub fn fire_cast(&mut self, from: Transform3D, towards: Vector3, custom_data: VarDictionary, config_override: Option<Gd<FluoriteCastConfig>>) -> Gd<FluoriteCast> {
        let mut new_instance = FluoriteCast::new_cast(
            self.parent_to.as_ref().expect("parent_to should exist").clone(),
            self.payload_scene.as_ref().map(|packed_scene| {
                packed_scene.try_instantiate_as().expect("payload_scene should extend always Node3D")
            }),
            config_override.unwrap_or_else(|| {
                self.projectile_config.as_ref().expect("projectile_config should always exist").clone()
            }),
            self.global_fluid.as_ref().expect("global_fluid should always exist").clone(),
            custom_data,
        );
        self.tracked_instances.replace(new_instance.clone());
        new_instance.bind_mut().fire(from, towards);
        // TODO: connect signals to proper functions and do cleanup from that list

        new_instance
    }
}