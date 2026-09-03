use godot::{global::ceilf, prelude::*};
use core::cmp::max;

use super::fluorite_cast_config::{FluoriteCastConfig, EvaluateMode, SuperSamplingMode};

#[derive(GodotClass)]
#[class(init, base=Node3D)]
pub struct FluoriteCast {
    base: Base<Node3D>,
    payload_node: Option<Gd<Node3D>>,
    config: Gd<FluoriteCastConfig>,
    #[var]
    current_velocity: Vector3,
    #[var]
    distance_covered: f32,
    #[var]
    alive_for: f64,
    #[var]
    custom_data: VarDictionary,
    // TODO: implement current_acceleration which would work in conjunction with gravity
}

#[godot_api]
impl FluoriteCast {
    #[func]
    pub fn new_cast(payload: Option<Gd<Node3D>>, config: Gd<FluoriteCastConfig>, custom_data: VarDictionary) -> Gd<Self> {
        let mut new_node = Gd::from_init_fn(|base| {
            Self {
                base,
                payload_node: None,
                config: config.clone(), // Grab a reference to it via reference counted smart pointer
                current_velocity: Vector3::ZERO,
                distance_covered: 0.0f32,
                alive_for: 0.0f64,
                custom_data,
            }
        });
        new_node.bind_mut().assign_payload(payload);
        let maybe_parent: Option<Gd<Node3D>> = {
            new_node.bind_mut().config.bind().parent_to.clone()
        };
        if let Some(mut to_parent) = maybe_parent {
            to_parent.add_child(&new_node);
        }
        new_node
    }
    #[func]
    pub fn assign_payload(&mut self, payload: Option<Gd<Node3D>>) -> () {
        if let Some(mut node) = self.payload_node.take() {
            node.queue_free();
        }
        self.payload_node = payload;
    }
    #[func]
    pub fn fire(&mut self, global_origin: Transform3D, direction: Vector3) -> () {
        self.base_mut().set_global_transform(global_origin);
        self.add_velocity(direction);
    }
    #[func] // TODO: might drop this
    pub fn fire_as_local_space(&mut self, local_origin: Transform3D, direction: Vector3) -> () {
        self.base_mut().set_transform(local_origin);
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
}

#[godot_api]
impl INode3D for FluoriteCast {
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