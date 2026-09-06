use godot::prelude::*;
use hashbrown::HashMap;
use core::any::Any;

use super::super::fluorite_cast_config::CollisionDetectionMode;
use super::{FluoriteCast, FluoriteSpaceCastResult};

pub struct FluoriteBuiltinState {
    pub current_penetrated_count: i64
}
impl FluoriteBuiltinState {
    pub fn new(
        current_penetrated_count: i64
    ) -> Self {
        Self {
            current_penetrated_count,
        }
    }
}
impl Default for FluoriteBuiltinState {
    fn default() -> Self {
        Self {
            current_penetrated_count: 0,
        }
    }
}

#[derive(GodotClass)]
#[class(init, base=Resource)]
pub struct FluoriteBuiltinConfig {
    base: Base<Resource>,
    #[export]
    #[init(val = "fluorite_builtin_penetratable".to_string_name())]
    pub penetratable_node_group_name: StringName,
    #[export]
    #[init(val = 4)]
    pub upwards_search_recursion_limit: i64,
    #[export]
    #[init(val = 2)]
    pub max_penetration_count: i64,
}   

impl FluoriteCast {
    pub fn parse_builtin_config(mut this: Gd<Self>) {
        let mut new_hmap = HashMap::new();
        {
            let this_binding = this.bind();
            let custom_config = &this_binding.config.bind().custom_config;
            if let Some(builtin_cfg_gd_as_resource) = custom_config.get("__builtin").flatten().as_ref() {
                match builtin_cfg_gd_as_resource.to_godot_owned().try_cast::<FluoriteBuiltinConfig>() {
                    Ok(_builtin_cfg_gd) => {
                        //let builtin_cfg = _builtin_cfg_gd.bind();
                        let builtin_state = FluoriteBuiltinState::default();

                        new_hmap.insert("__builtin".to_string(), Box::new(builtin_state) as Box<dyn Any>);
                    },
                    Err(_) => {
                        panic!("Downcasting to FluoriteBuiltinConfig failed in parse_builtin_config! Is a different kind of resource assigned?")
                    },
                }
            } else {
                panic!("cast_methods_cfg.builtin_flags was Some, but no key \"__builtin\" found in custom_config!
                It must be assigned with the value as a FluoriteBuiltinConfig instance!")
            }
        }
        this.bind_mut().custom_data_rs.replace(new_hmap);
    }
    pub fn _builtin_try_penetrate(&mut self, cast_result: Gd<FluoriteSpaceCastResult>) -> bool {
        if let Some(collider_gd) = &cast_result.bind().collider {
            //godot_warn!("1");
            let this_binding = self; //this.bind_mut(); // this crashes due to double mutable borrow
            //godot_warn!("2");
            let collision_detection_mode = this_binding.config.bind().cast_hit_detection_cfg.as_ref().expect("cast_hit_detection_cfg should always exist").bind().collision_detection_mode;
            //godot_warn!("3");
            let mut ignore_list = match collision_detection_mode {
                CollisionDetectionMode::Ignore => {
                    panic!("collision_detection_mode was Ignore when _builtin_try_penetrate was called! This should never happen!")
                },
                CollisionDetectionMode::ByRaycast => {
                    //godot_warn!("4a");
                    this_binding.query_params_cache_ray.as_mut().expect("query_params_cache_ray should exist").get_exclude() // this will be a deep copy, apparently? According to godot docs
                },
                CollisionDetectionMode::ByShapecast => {
                    //godot_warn!("4b");
                    this_binding.query_params_cache_shape.as_mut().expect("query_params_cache_ray should exist").get_exclude()
                },
            };
            //godot_warn!("5");
            let this_config_binding = this_binding.config.bind();
            let custom_config = &this_config_binding.custom_config;
            // This is kind of unergonomic... but oh well.
            let builtin_cfg_binding = custom_config
                .get("__builtin")
                .expect("__builtin should exist")
                .expect("__builtin should be a valid instance")
                .try_cast::<FluoriteBuiltinConfig>()
                .expect("__builtin should map to a FluoriteBuiltinConfig");
            let builtin_cfg = builtin_cfg_binding.bind();
            let target_group_name = &builtin_cfg.penetratable_node_group_name;
            let mut maybe_valid: Option<Gd<Node>> = Some(collider_gd.to_godot_owned().upcast());
            let recur_limit = builtin_cfg.upwards_search_recursion_limit;
            drop(this_config_binding);
            //godot_warn!("6");
            for _ in 0..=recur_limit {
                if let Some(actually_valid) = maybe_valid {
                    if actually_valid.is_in_group(target_group_name) {
                        Self::add_ignore_rid(actually_valid, &mut ignore_list, true);
                        match collision_detection_mode {
                            CollisionDetectionMode::Ignore => {
                                panic!("Unreachable code reached in _builtin_try_penetrate!")
                            },
                            CollisionDetectionMode::ByRaycast => {
                                this_binding.query_params_cache_ray.as_mut().expect("query_params_cache_ray should exist").set_exclude(&ignore_list)
                            },
                            CollisionDetectionMode::ByShapecast => {
                                this_binding.query_params_cache_shape.as_mut().expect("query_params_cache_ray should exist").set_exclude(&ignore_list)
                            },
                        };
                        return true
                    } else {
                        maybe_valid = actually_valid.get_parent();
                    }
                } else {
                    return false
                }
            }
            false
        } else {
            godot_warn!("collider was None while running _builtin_try_penetrate! Returning true!");
            true
        }
    }
}