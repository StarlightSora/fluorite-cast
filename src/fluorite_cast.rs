use godot::{
    classes::{
        CollisionShape3D,
        IStaticBody3D,
        ProjectSettings,
        StaticBody3D,
    },
    global::ceilf,
    prelude::*,
};
use core::cmp::max;

use super::fluorite_cast_config::{
    FluoriteCastConfig,
    EvaluateMode,
    SuperSamplingMode,
    GravityBehavior,
};

#[derive(GodotClass)]
#[class(init, base=StaticBody3D)]
pub struct FluoriteCast {
    base: Base<StaticBody3D>,
    payload_node: Option<Gd<Node3D>>,
    gravity_cache: Option<Vector3>,
    config: Gd<FluoriteCastConfig>,
    #[var]
    current_velocity: Vector3,
    #[var]
    current_acceleration: Vector3,
    #[var]
    distance_covered: f32,
    #[var]
    alive_for: f64,
    #[var]
    custom_data: VarDictionary,
}

#[godot_api]
impl FluoriteCast {
    #[func]
    pub fn new_cast(&mut parent_to: Gd<Node3D>, payload: Option<Gd<Node3D>>, config: Gd<FluoriteCastConfig>, custom_data: VarDictionary) -> Gd<Self> {
        let mut new_node = Gd::from_init_fn(|base| {
            Self {
                base,
                payload_node: None,
                gravity_cache: None,
                config: config.clone(), // We clone because Gd<T> is basically a Rc<RefCell<T>>
                current_velocity: Vector3::ZERO,
                current_acceleration: Vector3::ZERO,
                distance_covered: 0.0f32,
                alive_for: 0.0f64,
                custom_data,
            }
        });
        // The base needs to be `StaticBody3D`, and a `CollisionShape3D` is needed so we can use `get_gravity` for `UseCurrentGravityRealTime` mode
        let mut gd_colshape3d = None;

        let mut new_node_bind = new_node.bind_mut();
        let collision_mask_data = new_node_bind.config.bind().collision_mask;
        let gravity_behavior = new_node_bind.config.bind().gravity_behavior;
        match gravity_behavior {
            GravityBehavior::Ignore => {
                new_node_bind.gravity_cache.replace(Vector3::ZERO);
            },
            GravityBehavior::UseGlobalGravityCached => {
                let res = ProjectSettings::singleton().get_setting("physics/3d/default_gravity_vector").to::<Vector3>()
                    * (ProjectSettings::singleton().get_setting("physics/3d/default_gravity").to::<f64>() as f32);
                new_node_bind.gravity_cache.replace(res);
            },
            GravityBehavior::UseGlobalGravityRealTime => {}, // No-op
            GravityBehavior::UseCurrentGravityRealTime => {
                // `CollisionShape3D` is only needed for real-time local gravity polling
                gd_colshape3d.replace(CollisionShape3D::new_alloc());
                let some_cs3d = gd_colshape3d.as_mut().expect("Added right above");
                some_cs3d.set_name("_FluoriteCastGravityPollingCS3D");
                some_cs3d.set_shape(&new_node_bind.config.bind().shape.clone().expect("Shape should always be assigned"));
            },
        }
        new_node_bind.assign_payload(payload);
        drop(new_node_bind);

        new_node.set_collision_mask(collision_mask_data);
        new_node.set_collision_layer(0b0); // we only need to get affected by `Area3D`s for real-time local gravity polling, so disable collision layer entirely
        if let Some(some_gd_cs3) = gd_colshape3d {
            new_node.add_child(&some_gd_cs3);
        }

        parent_to.add_child(&new_node);

        new_node
    }
    #[func]
    pub fn assign_payload(&mut self, payload: Option<Gd<Node3D>>) -> () {
        if let Some(mut node) = self.payload_node.take() {
            node.queue_free();
        }
        self.payload_node = payload;
        if let Some(payload_rc) = self.payload_node.clone() {
            self.base_mut().add_child(&payload_rc);
        }
    }
    #[func]
    pub fn fire(&mut self, global_origin: Transform3D, direction: Vector3) -> () {
        self.base_mut().set_global_transform(global_origin);
        self.add_velocity(direction);
    }
    #[func]
    pub fn add_velocity(&mut self, by: Vector3) -> () {
        self.current_velocity += by;
    }
    #[func]
    pub fn evaluate(&mut self, delta: f64) -> () {
        let vel = self.current_velocity;
        let estimated_dist = (vel*(delta as f32)).length() as f64;
        let slice_count: i64;
        let cfg_bind = self.config.bind();
        match cfg_bind.super_sampling_mode {
            SuperSamplingMode::Never => {
                slice_count = 1;
            },
            SuperSamplingMode::IfAboveTargetDelta => {
                let ratio_t = delta / cfg_bind.target_delta;
                if ratio_t > 1.0 {
                    slice_count = max(ceilf(ratio_t) as i64, cfg_bind.max_supersampling);
                } else {
                    slice_count = 1;
                }
            },
            SuperSamplingMode::IfTooLong => {
                let ratio_l = estimated_dist / cfg_bind.target_length;
                if ratio_l > 1.0 {
                    slice_count = max(ceilf(ratio_l) as i64, cfg_bind.max_supersampling);
                } else {
                    slice_count = 1;
                }
            },
            SuperSamplingMode::IfAboveTargetDeltaOrTooLong => {
                let mut tmp: i64 = 1;
                let ratio_t = delta / cfg_bind.target_delta;
                if ratio_t > 1.0 {
                    tmp = max(tmp, ceilf(ratio_t) as i64);
                }
                let ratio_l = estimated_dist / cfg_bind.target_length;
                if ratio_l > 1.0 {
                    tmp = max(tmp, ceilf(ratio_l) as i64);
                }
                slice_count = max(tmp, cfg_bind.max_supersampling);
            },
        }
        drop(cfg_bind);
        let sliced_delta = delta / (slice_count as f64);
        for _ in 0..slice_count {
            self.evaluate_raw(sliced_delta);
        }
        self.maybe_free();
    }
    #[func]
    pub fn evaluate_raw(&mut self, delta: f64) -> () {
        // TODO: Modify velocity later when we add acceleration
        match self.gravity_cache {
            None => {
                let gravity_behavior = self.config.bind().gravity_behavior;
                let gravity_multiplier = self.config.bind().gravity_multiplier;
                match gravity_behavior {
                    GravityBehavior::UseGlobalGravityRealTime | GravityBehavior::UseCurrentGravityRealTime => {
                        // `get_gravity` returns global gravity if `CollisionShape3D` is missing
                        self.current_velocity += ((self.base().get_gravity()*(gravity_multiplier as f32)) + self.current_acceleration)*(delta as f32);
                    },
                    _ => { panic!("gravity_cache should exist for cached modes") }
                }
            },
            Some(g) => {
                let gravity_multiplier = self.config.bind().gravity_multiplier;
                self.current_velocity += (g*(gravity_multiplier as f32) + self.current_acceleration)*(delta as f32);
            },
        }
        let vel = self.current_velocity;
        let mut base_mut = self.base_mut();
        let starting_pos = base_mut.get_position();
        let dist = vel*(delta as f32);
        base_mut.set_global_position(starting_pos + dist);
        drop(base_mut);
        self.alive_for += delta;
        self.distance_covered += dist.length();
    }
    #[func]
    pub fn maybe_free(&mut self) -> () {
        let alive_for = self.alive_for;
        let distance_covered = self.distance_covered;
        let cfg_bind = self.config.bind();
        let should_free: bool;
        if alive_for > cfg_bind.max_alive_time {
            should_free = true
        } else if distance_covered > (cfg_bind.max_total_length as f32) {
            should_free = true
        } else { should_free = false }
        drop(cfg_bind);
        if should_free {
            self.base_mut().queue_free();
        }
    }
    #[func]
    pub fn get_config(&self) -> Gd<FluoriteCastConfig> {
        self.config.clone()
    }
}

#[godot_api]
impl IStaticBody3D for FluoriteCast {
    fn process(&mut self, delta: f64) {
        let mut can_do = false; // The stupid crap borrowck forces me to do
        if let EvaluateMode::Process = self.config.bind().evaluate_mode {
            can_do = true;
        }
        if can_do {
            self.evaluate(delta);
        }
    }
    fn physics_process(&mut self, delta: f64) {
        let mut can_do = false;
        if let EvaluateMode::PhysicsProcess = self.config.bind().evaluate_mode {
            can_do = true;
        }
        if can_do {
            self.evaluate(delta);
        }
    }
}